//! Work that takes long, out of the way of the screens: it runs on its own thread, and what it
//! brings comes back on the thread of the screens. In the browser, which has no threads to spare,
//! it runs right away.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};

use ravenrs_reactive::State;
use ravenrs_widgets::UiWaker;

use crate::Session;

/// How a piece of work in the background is going.
#[derive(Clone, Debug, PartialEq)]
pub struct JobInfo {
    /// What it is known by, to cancel it.
    pub id: u64,
    /// What it is doing, for people.
    pub title: String,
    /// How much of it is done, from 0 to 1, if it says.
    pub fraction: Option<f32>,
    /// The step it is on, if it says.
    pub note: Option<String>,
}

enum JobEvent {
    Progress(Option<f32>, Option<String>),
    Done,
}

/// What a piece of work has, on its thread, to say how it is going and to hear it was canceled.
#[derive(Clone, Debug)]
pub struct JobProgress {
    sender: Option<Sender<JobEvent>>,
    waker: Option<UiWaker>,
    canceled: Arc<AtomicBool>,
}

impl std::fmt::Debug for JobEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("JobEvent")
    }
}

impl JobProgress {
    /// Says how much is done, from 0 to 1, and what it is doing.
    pub fn report(&self, fraction: f32, note: impl Into<String>) {
        if let Some(sender) = &self.sender {
            let _ = sender.send(JobEvent::Progress(Some(fraction.clamp(0.0, 1.0)), Some(note.into())));
        }
        if let Some(waker) = &self.waker {
            waker.wake();
        }
    }

    /// Whether someone asked it to stop. It stops when it next looks.
    pub fn is_canceled(&self) -> bool {
        self.canceled.load(Ordering::Acquire)
    }
}

type Finish = Box<dyn FnOnce(&Session)>;

struct Running {
    id: u64,
    canceled: Arc<AtomicBool>,
    events: Receiver<JobEvent>,
    /// What it brought, once it did.
    result: Box<dyn FnMut() -> Option<Finish>>,
}

/// What runs for a session.
pub(crate) struct Jobs {
    running: RefCell<Vec<Running>>,
    next: std::cell::Cell<u64>,
    infos: State<Vec<JobInfo>>,
    waker: RefCell<Option<UiWaker>>,
}

impl Default for Jobs {
    fn default() -> Self {
        Self {
            running: RefCell::new(Vec::new()),
            next: std::cell::Cell::new(1),
            infos: State::new(Vec::new()),
            waker: RefCell::new(None),
        }
    }
}

impl std::fmt::Debug for Jobs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Jobs").finish_non_exhaustive()
    }
}

impl Jobs {
    pub(crate) fn connect(&self, waker: UiWaker) {
        *self.waker.borrow_mut() = Some(waker);
    }
}

impl Session {
    /// Runs `work` out of the way of the screens, then `done` with what it brought, on the thread of
    /// the screens. Gives what it is known by, to cancel it.
    pub fn run_background<R: Send + 'static>(
        &self,
        title: impl Into<String>,
        work: impl FnOnce(&JobProgress) -> R + Send + 'static,
        done: impl FnOnce(&Session, R) + 'static,
    ) -> u64 {
        let jobs = self.jobs();
        let id = jobs.next.get();
        jobs.next.set(id + 1);
        #[cfg(target_arch = "wasm32")]
        {
            let _ = title;
            let progress =
                JobProgress { sender: None, waker: None, canceled: Arc::new(AtomicBool::new(false)) };
            done(self, work(&progress));
            return id;
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let (events, heard) = channel();
            let (result_out, result_in) = channel::<R>();
            let canceled = Arc::new(AtomicBool::new(false));
            let waker = jobs.waker.borrow().clone();
            let progress = JobProgress {
                sender: Some(events.clone()),
                waker: waker.clone(),
                canceled: canceled.clone(),
            };
            std::thread::spawn(move || {
                let brought = work(&progress);
                let _ = result_out.send(brought);
                let _ = events.send(JobEvent::Done);
                if let Some(waker) = waker {
                    waker.wake();
                }
            });
            let mut done = Some(done);
            let result: Box<dyn FnMut() -> Option<Finish>> = Box::new(move || {
                let brought = result_in.try_recv().ok()?;
                let done = done.take()?;
                Some(Box::new(move |session: &Session| done(session, brought)) as Finish)
            });
            jobs.running.borrow_mut().push(Running { id, canceled, events: heard, result });
            jobs.infos.update(|infos| {
                infos.push(JobInfo { id, title: title.into(), fraction: None, note: None });
            });
            id
        }
    }

    /// Asks a piece of work to stop. What it brings when it does still comes.
    pub fn cancel_background(&self, id: u64) {
        if let Some(job) = self.jobs().running.borrow().iter().find(|job| job.id == id) {
            job.canceled.store(true, Ordering::Release);
        }
    }

    /// What is running now.
    pub fn background_jobs(&self) -> Vec<JobInfo> {
        self.jobs().infos.get()
    }

    /// Takes in what the work running has to say, and finishes what ended. The window does this
    /// by itself; a test without one calls it.
    pub fn poll_background(&self) {
        let jobs = self.jobs();
        let mut finished: Vec<Finish> = Vec::new();
        let mut changes: Vec<(u64, Option<f32>, Option<String>)> = Vec::new();
        let mut gone: Vec<u64> = Vec::new();
        jobs.running.borrow_mut().retain_mut(|job| {
            let mut over = false;
            loop {
                match job.events.try_recv() {
                    Ok(JobEvent::Progress(fraction, note)) => changes.push((job.id, fraction, note)),
                    Ok(JobEvent::Done) | Err(TryRecvError::Disconnected) => {
                        over = true;
                        break;
                    }
                    Err(TryRecvError::Empty) => break,
                }
            }
            if over {
                if let Some(finish) = (job.result)() {
                    finished.push(finish);
                }
                gone.push(job.id);
            }
            !over
        });
        if !changes.is_empty() || !gone.is_empty() {
            jobs.infos.update(|infos| {
                for (id, fraction, note) in changes {
                    if let Some(info) = infos.iter_mut().find(|info| info.id == id) {
                        info.fraction = fraction;
                        info.note = note;
                    }
                }
                infos.retain(|info| !gone.contains(&info.id));
            });
        }
        for finish in finished {
            finish(self);
        }
    }

    /// Waits for everything running and finishes it, for a test.
    #[doc(hidden)]
    pub fn finish_background(&self) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
        while !self.background_jobs().is_empty() && std::time::Instant::now() < deadline {
            self.poll_background();
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
    }
}

/// Shared with the frames of a session.
pub(crate) type SharedJobs = Rc<Jobs>;
