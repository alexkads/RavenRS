//! Keeps one job from running twice at once.

use std::collections::HashSet;
use std::fmt;
use std::sync::{Arc, Mutex};

/// The jobs running now, by key. Clones share the same set.
#[derive(Clone, Default)]
pub struct Running {
    keys: Arc<Mutex<HashSet<String>>>,
}

impl fmt::Debug for Running {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let count = self.keys.lock().map_or(0, |keys| keys.len());
        f.debug_struct("Running").field("count", &count).finish()
    }
}

impl Running {
    /// An empty set.
    pub fn new() -> Self {
        Self::default()
    }

    /// Marks `key` as running, or returns `None` when it already is. The key is free again
    /// when the guard is dropped, also if the run panics.
    pub fn try_start(&self, key: &str) -> Option<RunningGuard> {
        let free = self.keys.lock().is_ok_and(|mut keys| keys.insert(key.to_owned()));
        free.then(|| RunningGuard { keys: self.keys.clone(), key: key.to_owned() })
    }

    /// Whether `key` is running now.
    pub fn is_running(&self, key: &str) -> bool {
        self.keys.lock().is_ok_and(|keys| keys.contains(key))
    }
}

/// Held while a job runs; dropping it frees the key.
#[derive(Debug)]
pub struct RunningGuard {
    keys: Arc<Mutex<HashSet<String>>>,
    key: String,
}

impl RunningGuard {
    /// The key it holds.
    pub fn key(&self) -> &str {
        &self.key
    }
}

impl Drop for RunningGuard {
    fn drop(&mut self) {
        if let Ok(mut keys) = self.keys.lock() {
            keys.remove(&self.key);
        }
    }
}
