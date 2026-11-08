//! Work an action does away from the screen: it runs on a thread of its own, says how far it got,
//! can be stopped, and hands back the records it changed, which the frame keeps on the screen's
//! thread.

use std::fmt;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use crate::{ActionInput, MessageKind, Object};

/// What a background action does: it reads what it was given, does its work and tells as it goes.
/// An error ends it, in words for people.
pub type JobWork = Arc<dyn Fn(&JobContext) -> Result<(), String> + Send + Sync>;

/// Something that wakes the screen, from any thread.
pub type JobWaker = Arc<dyn Fn() + Send + Sync>;

/// A change a job hands back, kept by the frame on the screen's thread.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "variants"))]
pub enum JobChange {
    /// Keep this record, new or changed.
    Save(Object),
    /// Remove this record.
    Delete(Object),
    /// Tell people this.
    Tell(String, MessageKind),
}

#[derive(Default)]
struct Shared {
    /// How far it got, from 0 to 1, and what it is doing; nothing while it cannot tell.
    progress: Mutex<(Option<f32>, String)>,
    cancel: AtomicBool,
    changes: Mutex<Vec<JobChange>>,
    /// How it ended, once it did.
    end: Mutex<Option<Result<(), String>>>,
    waker: Mutex<Option<JobWaker>>,
}

fn locked<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    // A job that panicked still leaves what it handed over readable.
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl Shared {
    fn wake(&self) {
        let waker = locked(&self.waker).clone();
        if let Some(waker) = waker {
            waker();
        }
    }
}

/// What a job sees while it runs, on its own thread.
///
/// ```
/// use ravenrs_app::{JobContext, Object};
///
/// fn count(job: &JobContext) -> Result<(), String> {
///     for (done, record) in job.selected().iter().enumerate() {
///         if job.cancelled() {
///             return Ok(());
///         }
///         job.progress((done + 1) as f32 / job.selected().len() as f32, record.entity());
///         let mut record = record.clone();
///         record.set("Counted", true);
///         job.save(record);
///     }
///     Ok(())
/// }
/// # let _ = count;
/// ```
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct JobContext {
    selected: Vec<Object>,
    input: ActionInput,
    shared: Arc<Shared>,
}

impl fmt::Debug for JobContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("JobContext").field("selected", &self.selected.len()).finish_non_exhaustive()
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl JobContext {
    /// The records selected when it started, as they were then.
    pub fn selected(&self) -> &[Object] {
        &self.selected
    }

    /// What people gave the action.
    pub fn input(&self) -> &ActionInput {
        &self.input
    }

    /// How far it got, from 0 to 1, and what it is doing now.
    pub fn progress(&self, done: f32, doing: impl Into<String>) {
        *locked(&self.shared.progress) = (Some(done.clamp(0.0, 1.0)), doing.into());
        self.shared.wake();
    }

    /// What it is doing, when it cannot tell how far it got.
    pub fn doing(&self, doing: impl Into<String>) {
        *locked(&self.shared.progress) = (None, doing.into());
        self.shared.wake();
    }

    /// Whether someone asked it to stop. It stops by returning; what it handed over stays.
    pub fn cancelled(&self) -> bool {
        self.shared.cancel.load(Ordering::Relaxed)
    }

    /// Keeps this record, new or changed.
    pub fn save(&self, record: Object) {
        self.hand(JobChange::Save(record));
    }

    /// Removes this record.
    pub fn delete(&self, record: Object) {
        self.hand(JobChange::Delete(record));
    }

    /// Tells people something.
    pub fn tell(&self, text: impl Into<String>, kind: MessageKind) {
        self.hand(JobChange::Tell(text.into(), kind));
    }

    fn hand(&self, change: JobChange) {
        locked(&self.shared.changes).push(change);
        self.shared.wake();
    }
}

static NEXT_JOB: AtomicU64 = AtomicU64::new(1);

/// A job that started, as the screen sees it. A handle: the frame keeps one while it runs.
#[derive(Clone)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct Job {
    id: u64,
    action: String,
    caption: String,
    shared: Arc<Shared>,
}

impl fmt::Debug for Job {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Job").field("id", &self.id).field("action", &self.action).finish_non_exhaustive()
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl Job {
    /// Starts this work on a thread of its own (or right here, where there are no threads) and
    /// wakes the screen with `waker` each time it has something to show.
    pub(crate) fn start(
        action: &str,
        caption: &str,
        work: JobWork,
        selected: Vec<Object>,
        input: ActionInput,
        waker: Option<JobWaker>,
    ) -> Self {
        let shared = Arc::new(Shared { waker: Mutex::new(waker), ..Shared::default() });
        let job = Self {
            id: NEXT_JOB.fetch_add(1, Ordering::Relaxed),
            action: action.to_owned(),
            caption: caption.to_owned(),
            shared: shared.clone(),
        };
        let cx = JobContext { selected, input, shared };
        let run = move || {
            // Ends it even when the work panics, so the screen never waits for nothing.
            struct Ending(Arc<Shared>, Option<Result<(), String>>);
            impl Drop for Ending {
                fn drop(&mut self) {
                    let end =
                        self.1.take().unwrap_or_else(|| Err("the work stopped unexpectedly".to_owned()));
                    *locked(&self.0.end) = Some(end);
                    self.0.wake();
                }
            }
            let mut ending = Ending(cx.shared.clone(), None);
            ending.1 = Some(work(&cx));
        };
        #[cfg(not(target_arch = "wasm32"))]
        {
            let name = format!("job {}", job.action);
            if let Err(error) = std::thread::Builder::new().name(name).spawn(run) {
                *locked(&job.shared.end) = Some(Err(error.to_string()));
            }
        }
        #[cfg(target_arch = "wasm32")]
        run();
        job
    }

    /// What it is known by, while the application runs.
    pub fn id(&self) -> u64 {
        self.id
    }

    /// The id of the action that started it.
    pub fn action(&self) -> &str {
        &self.action
    }

    /// What it says, as the action does.
    pub fn caption(&self) -> &str {
        &self.caption
    }

    /// How far it got, from 0 to 1 (nothing while it cannot tell), and what it is doing.
    pub fn progress(&self) -> (Option<f32>, String) {
        locked(&self.shared.progress).clone()
    }

    /// Asks it to stop.
    pub fn cancel(&self) {
        self.shared.cancel.store(true, Ordering::Relaxed);
    }

    /// Whether someone asked it to stop.
    pub fn is_cancelled(&self) -> bool {
        self.shared.cancel.load(Ordering::Relaxed)
    }

    /// Changes the waker, for the screen that shows it now.
    pub(crate) fn set_waker(&self, waker: Option<JobWaker>) {
        *locked(&self.shared.waker) = waker;
    }

    /// What it handed over since last asked.
    pub(crate) fn take_changes(&self) -> Vec<JobChange> {
        std::mem::take(&mut *locked(&self.shared.changes))
    }

    /// How it ended, once it did and everything it handed over was taken.
    pub(crate) fn take_end(&self) -> Option<Result<(), String>> {
        let end = locked(&self.shared.end).take()?;
        // What it handed over just before ending comes first.
        if !locked(&self.shared.changes).is_empty() {
            *locked(&self.shared.end) = Some(end);
            return None;
        }
        Some(end)
    }
}
