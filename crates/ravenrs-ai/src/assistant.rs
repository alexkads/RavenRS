//! A model in use: answers away from the UI thread, and lets go of its memory when
//! nobody has asked anything for a while.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use ravenrs_widgets::UiWaker;

use crate::tools::json_closed;
use crate::{LocalEngine, LocalModel, ModelFolder};

/// How long a model stays in memory after answering.
const STAYS_LOADED: Duration = Duration::from_secs(120);
const MAX_TOKENS: usize = 160;
/// How much a choice can take: a tool and its arguments are short.
const CHOICE_TOKENS: usize = 80;
/// How long someone waits, counting the loading: for a whole choice, or for the first
/// word of an answer. A slow machine should not leave them looking at it forever.
const WAIT_LIMIT: Duration = Duration::from_secs(60);

struct Job {
    system: String,
    user: String,
    text: Arc<Mutex<String>>,
    waker: UiWaker,
    if_it_fails: String,
    done: Option<Arc<AtomicBool>>,
    /// Set when the wait ended first: nobody wants this answer any more.
    given_up: Arc<AtomicBool>,
    /// Letting go of it ends the wait.
    waiting: Option<Sender<()>>,
}

impl Job {
    /// Takes in a piece of the answer, and says whether to go on.
    fn take(&mut self, piece: &str) -> bool {
        let Ok(mut text) = self.text.lock() else { return false };
        if self.given_up.load(Ordering::Acquire) {
            return false;
        }
        text.push_str(piece);
        let go_on = match self.done {
            // A choice ends with its JSON, whatever a model goes on to say.
            Some(_) => !json_closed(&text),
            None => {
                self.waiting = None;
                true
            }
        };
        drop(text);
        self.waker.wake();
        go_on
    }

    /// Says it is over, with `if_it_fails` when nothing came.
    fn finish(&self, answered: bool) {
        if !answered && let Ok(mut text) = self.text.lock() {
            text.clone_from(&self.if_it_fails);
        }
        if let Some(done) = &self.done {
            done.store(true, Ordering::Release);
        }
        self.waker.wake();
    }
}

/// A model that answers on its own thread. Dropping it ends the thread and frees
/// the memory.
#[derive(Clone, Debug)]
pub struct LocalAssistant {
    jobs: Sender<Job>,
    too_slow: Arc<AtomicBool>,
    wait_limit: Duration,
}

impl Job {
    /// What every job starts with.
    fn blank() -> Self {
        Self {
            system: String::new(),
            user: String::new(),
            text: Arc::default(),
            waker: UiWaker::default(),
            if_it_fails: String::new(),
            done: None,
            given_up: Arc::default(),
            waiting: None,
        }
    }
}

impl std::fmt::Debug for Job {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Job").finish_non_exhaustive()
    }
}

impl LocalAssistant {
    /// An assistant over `model`. Nothing is loaded until the first question.
    pub fn new(folder: ModelFolder, model: LocalModel) -> Self {
        let (jobs, inbox) = mpsc::channel::<Job>();
        std::thread::spawn(move || {
            let mut engine: Option<LocalEngine> = None;
            loop {
                let mut job = if engine.is_some() {
                    match inbox.recv_timeout(STAYS_LOADED) {
                        Ok(job) => job,
                        Err(RecvTimeoutError::Timeout) => {
                            engine = None;
                            continue;
                        }
                        Err(RecvTimeoutError::Disconnected) => return,
                    }
                } else {
                    match inbox.recv() {
                        Ok(job) => job,
                        Err(_) => return,
                    }
                };
                if job.given_up.load(Ordering::Acquire) {
                    continue;
                }
                if engine.is_none() {
                    engine = LocalEngine::load(&folder, &model).ok();
                }
                let limit = if job.done.is_some() { CHOICE_TOKENS } else { MAX_TOKENS };
                let (system, user) = (job.system.clone(), job.user.clone());
                let answered = engine.as_mut().is_some_and(|engine| {
                    let wrote = engine.generate(&system, &user, limit, &mut |piece| job.take(piece));
                    wrote.is_ok_and(|generated| generated.tokens > 0)
                });
                if !job.given_up.load(Ordering::Acquire) {
                    job.finish(answered);
                }
            }
        });
        Self { jobs, too_slow: Arc::new(AtomicBool::new(false)), wait_limit: WAIT_LIMIT }
    }

    /// Asks it to answer `user`, following `system`, writing to `text` as it goes.
    /// `if_it_fails` is what `text` becomes if the model cannot answer.
    pub fn write(
        &self,
        system: String,
        user: String,
        text: Arc<Mutex<String>>,
        waker: UiWaker,
        if_it_fails: String,
    ) {
        self.send(Job { system, user, text, waker, if_it_fails, done: None, ..Job::blank() });
    }

    /// Asks it to answer `user`, following `system`, and says when it is over: `done`
    /// is set after `text` has all of it, or stays empty if the model could not answer.
    pub fn answer(
        &self,
        system: String,
        user: String,
        text: Arc<Mutex<String>>,
        done: Arc<AtomicBool>,
        waker: UiWaker,
    ) {
        let if_it_fails = String::new();
        self.send(Job { system, user, text, waker, if_it_fails, done: Some(done), ..Job::blank() });
    }

    /// Whether a question waited too long on this machine. From then on it answers
    /// nothing, at once, so nobody waits for it again until another assistant is made.
    pub fn too_slow(&self) -> bool {
        self.too_slow.load(Ordering::Acquire)
    }

    fn send(&self, mut job: Job) {
        if self.too_slow() {
            job.finish(false);
            return;
        }
        let (waiting, ended) = mpsc::channel();
        job.waiting = Some(waiting);
        let watched = Job {
            system: String::new(),
            user: String::new(),
            text: job.text.clone(),
            waker: job.waker.clone(),
            if_it_fails: job.if_it_fails.clone(),
            done: job.done.clone(),
            given_up: job.given_up.clone(),
            waiting: None,
        };
        let (too_slow, limit) = (self.too_slow.clone(), self.wait_limit);
        std::thread::spawn(move || {
            if matches!(ended.recv_timeout(limit), Err(RecvTimeoutError::Timeout)) {
                too_slow.store(true, Ordering::Release);
                watched.given_up.store(true, Ordering::Release);
                watched.finish(false);
            }
        });
        // The thread ends only with this handle, so this does not fail.
        let _ = self.jobs.send(job);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An assistant whose model never gets to answer.
    fn stuck() -> (LocalAssistant, mpsc::Receiver<Job>) {
        let (jobs, inbox) = mpsc::channel();
        let wait_limit = Duration::from_millis(20);
        (LocalAssistant { jobs, too_slow: Arc::default(), wait_limit }, inbox)
    }

    fn wait_for(done: &AtomicBool) {
        let started = std::time::Instant::now();
        while !done.load(Ordering::Acquire) {
            assert!(started.elapsed() < Duration::from_secs(5), "it waited forever");
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn a_choice_that_takes_too_long_ends_empty() {
        let (assistant, _inbox) = stuck();
        let (text, done) = (Arc::new(Mutex::new("half".to_owned())), Arc::new(AtomicBool::new(false)));
        assistant.answer("rules".into(), "question".into(), text.clone(), done.clone(), UiWaker::default());
        wait_for(&done);
        assert!(assistant.too_slow());
        assert_eq!(*text.lock().expect("text"), "");
    }

    #[test]
    fn after_that_nobody_waits_again() {
        let (assistant, inbox) = stuck();
        let (text, done) = (Arc::new(Mutex::new(String::new())), Arc::new(AtomicBool::new(false)));
        assistant.answer("rules".into(), "question".into(), text.clone(), done.clone(), UiWaker::default());
        wait_for(&done);
        drop(inbox.try_recv());
        let words = Arc::new(Mutex::new(String::new()));
        assistant.write("rules".into(), "question".into(), words.clone(), UiWaker::default(), "found".into());
        assert_eq!(*words.lock().expect("text"), "found");
        assert!(inbox.try_recv().is_err(), "it did not reach the model");
    }

    #[test]
    fn nothing_arrives_after_the_wait_is_over() {
        let given_up = Arc::new(AtomicBool::new(true));
        let text = Arc::new(Mutex::new(String::new()));
        let mut job = Job { text: text.clone(), given_up, ..Job::blank() };
        assert!(!job.take("late"));
        assert_eq!(*text.lock().expect("text"), "");
    }

    #[test]
    fn a_choice_stops_when_its_json_closes() {
        let done = Some(Arc::new(AtomicBool::new(false)));
        let mut job = Job { done, ..Job::blank() };
        assert!(job.take(r#"{"tool":"a","args":{"#));
        assert!(!job.take(r#"}}"#));
    }

    #[test]
    fn the_first_word_ends_the_wait_for_an_answer() {
        let (waiting, ended) = mpsc::channel::<()>();
        let mut job = Job { waiting: Some(waiting), ..Job::blank() };
        assert!(job.take("A"));
        assert!(matches!(ended.try_recv(), Err(mpsc::TryRecvError::Disconnected)));
    }
}
