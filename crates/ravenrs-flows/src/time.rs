//! Time and stopping: the clock a run reads and waits on, and the token that cancels it. The
//! bottom of the crate with `item`: the catalog hands both to every node.

use std::fmt;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::{Arc, Mutex};

/// Where a run takes the time from, and how it waits.
pub trait Clock: Send + Sync {
    /// Milliseconds since 1970.
    fn now_ms(&self) -> i64;

    /// Waits `ms` milliseconds. A test clock may move its time instead of waiting.
    fn sleep_ms(&self, ms: u64) {
        #[cfg(not(target_arch = "wasm32"))]
        std::thread::sleep(std::time::Duration::from_millis(ms));
        #[cfg(target_arch = "wasm32")]
        let _ = ms;
    }
}

impl fmt::Debug for dyn Clock {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Clock({})", self.now_ms())
    }
}

/// A clock that always says the same time and does not wait: for repeatable runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FixedClock(pub i64);

impl Clock for FixedClock {
    fn now_ms(&self) -> i64 {
        self.0
    }

    fn sleep_ms(&self, _: u64) {}
}

/// A clock that moves only when something waits on it, by what was waited: tests of waits and
/// retries take no time, and the time they would have taken is there to check.
#[derive(Clone, Debug, Default)]
pub struct ManualClock(Arc<AtomicI64>);

impl ManualClock {
    /// A clock at `ms` milliseconds since 1970.
    pub fn at(ms: i64) -> Self {
        Self(Arc::new(AtomicI64::new(ms)))
    }

    /// Moves it `ms` milliseconds on.
    pub fn advance(&self, ms: i64) {
        self.0.fetch_add(ms, Ordering::SeqCst);
    }
}

impl Clock for ManualClock {
    fn now_ms(&self) -> i64 {
        self.0.load(Ordering::SeqCst)
    }

    fn sleep_ms(&self, ms: u64) {
        self.advance(ms as i64);
    }
}

/// The machine's clock.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SystemClock;

impl Clock for SystemClock {
    #[cfg(not(target_arch = "wasm32"))]
    fn now_ms(&self) -> i64 {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
    }

    /// In a browser, `std` has no clock: the page's is used.
    #[cfg(target_arch = "wasm32")]
    fn now_ms(&self) -> i64 {
        js_sys::Date::now() as i64
    }
}

#[derive(Debug, Default)]
struct Signal {
    cancelled: AtomicBool,
    reason: Mutex<Option<String>>,
}

/// Says that a run, or a part of it, should stop. Clones share it, so one is given to a run and
/// another kept to cancel it from any thread; a child is cancelled with its parent, so a
/// subflow stops when the flow that called it does.
#[derive(Clone, Debug, Default)]
pub struct CancelToken {
    signal: Arc<Signal>,
    parent: Option<Box<CancelToken>>,
}

impl CancelToken {
    /// A token nothing has cancelled.
    pub fn new() -> Self {
        Self::default()
    }

    /// A token cancelled when this one is, and also on its own.
    pub fn child(&self) -> Self {
        Self { signal: Arc::default(), parent: Some(Box::new(self.clone())) }
    }

    /// Cancels it, saying why. The first reason given stays.
    pub fn cancel(&self, reason: impl Into<String>) {
        if let Ok(mut said) = self.signal.reason.lock() {
            said.get_or_insert_with(|| reason.into());
        }
        self.signal.cancelled.store(true, Ordering::SeqCst);
    }

    /// Whether it, or a token it came from, was cancelled.
    pub fn is_cancelled(&self) -> bool {
        self.signal.cancelled.load(Ordering::SeqCst) || self.parent.as_ref().is_some_and(|p| p.is_cancelled())
    }

    /// Why it was cancelled: its own reason, or the nearest parent's.
    pub fn reason(&self) -> Option<String> {
        if self.signal.cancelled.load(Ordering::SeqCst) {
            return self.signal.reason.lock().ok().and_then(|r| r.clone());
        }
        self.parent.as_ref().and_then(|p| p.reason())
    }
}

impl fmt::Display for CancelToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (self.is_cancelled(), self.reason()) {
            (true, Some(why)) => write!(f, "cancelled: {why}"),
            (true, None) => f.write_str("cancelled"),
            _ => f.write_str("going"),
        }
    }
}
