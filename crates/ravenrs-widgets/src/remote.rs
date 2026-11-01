//! Asking for a frame from another thread. Everything else in the runtime
//! belongs to one thread, so this is the one handle that may leave it.

use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};

type Notify = Box<dyn Fn() + Send + Sync>;

#[derive(Default)]
struct Shared {
    /// Set by [`UiWaker::wake`] and cleared by the build it causes.
    dirty: AtomicBool,
    /// How the platform gets out of its sleep; absent in tests and on the web.
    notify: OnceLock<Notify>,
}

/// Rebuilds the UI from any thread, when background work has something to show.
///
/// Get one with [`BuildCx::ui_waker`](crate::BuildCx::ui_waker) and move it to
/// the thread. Send the result through a channel and call [`UiWaker::wake`]; the
/// next build runs on the UI thread and reads the channel. Between wakes the app
/// stays asleep, so there is nothing to poll.
///
/// ```
/// use std::sync::mpsc::channel;
/// use ravenrs_widgets::{BuildCx, NodeId, View};
///
/// struct Pump;
///
/// impl View for Pump {
///     fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
///         let waker = cx.ui_waker();
///         let (send, _receive) = channel();
///         std::thread::spawn(move || {
///             let _ = send.send(21 * 2);
///             waker.wake();
///         });
///         cx.empty()
///     }
/// }
/// ```
///
/// The web build has no other threads to wake it from; there, change a
/// [`State`](ravenrs_reactive::State) as usual.
#[derive(Clone, Default)]
pub struct UiWaker(Arc<Shared>);

impl fmt::Debug for UiWaker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UiWaker").field("dirty", &self.is_dirty()).finish()
    }
}

impl UiWaker {
    /// Asks for a rebuild. Calls made before it happens count as one.
    pub fn wake(&self) {
        self.0.dirty.store(true, Ordering::Release);
        if let Some(notify) = self.0.notify.get() {
            notify();
        }
    }

    pub(crate) fn is_dirty(&self) -> bool {
        self.0.dirty.load(Ordering::Acquire)
    }

    pub(crate) fn clear(&self) {
        self.0.dirty.store(false, Ordering::Release);
    }

    pub(crate) fn set_notify(&self, notify: Notify) {
        if self.0.notify.set(notify).is_err() {
            tracing::warn!("the UI waker was already connected to a platform");
        }
    }
}
