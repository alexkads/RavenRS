//! A model in use: answers away from the UI thread, and lets go of its memory when
//! nobody has asked anything for a while.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use ravenrs_widgets::UiWaker;

use crate::{LocalEngine, LocalModel, ModelFolder};

/// How long a model stays in memory after answering.
const STAYS_LOADED: Duration = Duration::from_secs(120);
const MAX_TOKENS: usize = 160;

struct Job {
    system: String,
    user: String,
    text: Arc<Mutex<String>>,
    waker: UiWaker,
    if_it_fails: String,
    done: Option<Arc<AtomicBool>>,
}

/// A model that answers on its own thread. Dropping it ends the thread and frees
/// the memory.
#[derive(Clone, Debug)]
pub struct LocalAssistant {
    jobs: Sender<Job>,
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
                let job = if engine.is_some() {
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
                if engine.is_none() {
                    engine = LocalEngine::load(&folder, &model).ok();
                }
                let answered = engine.as_mut().is_some_and(|engine| {
                    let wrote = engine.generate(&job.system, &job.user, MAX_TOKENS, &mut |piece| {
                        if let Ok(mut text) = job.text.lock() {
                            text.push_str(piece);
                        }
                        job.waker.wake();
                        true
                    });
                    wrote.is_ok_and(|generated| generated.tokens > 0)
                });
                if !answered && let Ok(mut text) = job.text.lock() {
                    text.clone_from(&job.if_it_fails);
                }
                if let Some(done) = &job.done {
                    done.store(true, Ordering::Release);
                }
                job.waker.wake();
            }
        });
        Self { jobs }
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
        let job = Job { system, user, text, waker, if_it_fails, done: None };
        // The thread ends only with this handle, so this does not fail.
        let _ = self.jobs.send(job);
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
        let job = Job { system, user, text, waker, if_it_fails: String::new(), done: Some(done) };
        let _ = self.jobs.send(job);
    }
}
