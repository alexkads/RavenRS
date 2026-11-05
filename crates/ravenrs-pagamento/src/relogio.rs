//! Time as something passed in, so that tests decide when it runs.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

/// Where blocking parts of this crate get the time, and how they wait.
pub trait Relogio {
    /// Milliseconds from any fixed moment.
    fn agora_ms(&self) -> u64;

    /// Waits this long. A test clock may do other work here instead of sleeping.
    fn esperar_ms(&mut self, ms: u64);
}

/// A clock that only moves when told. Its clones share the same time.
#[derive(Clone, Debug, Default)]
pub struct RelogioManual(Arc<AtomicU64>);

impl RelogioManual {
    /// A clock at zero.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Moves it forward.
    pub fn avancar(&self, ms: u64) {
        self.0.fetch_add(ms, Ordering::Relaxed);
    }
}

impl Relogio for RelogioManual {
    fn agora_ms(&self) -> u64 {
        self.0.load(Ordering::Relaxed)
    }

    fn esperar_ms(&mut self, ms: u64) {
        self.avancar(ms);
    }
}

/// The clock of the machine.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Copy, Debug)]
pub struct RelogioSistema(std::time::Instant);

#[cfg(not(target_arch = "wasm32"))]
impl Default for RelogioSistema {
    fn default() -> Self {
        Self(std::time::Instant::now())
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl Relogio for RelogioSistema {
    fn agora_ms(&self) -> u64 {
        u64::try_from(self.0.elapsed().as_millis()).unwrap_or(u64::MAX)
    }

    fn esperar_ms(&mut self, ms: u64) {
        std::thread::sleep(std::time::Duration::from_millis(ms));
    }
}
