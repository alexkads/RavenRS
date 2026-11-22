//! What a machine has room for.

use std::path::Path;

use sysinfo::{MemoryRefreshKind, RefreshKind, System};

/// What a machine has, for deciding which models to offer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Machine {
    /// All its memory, in bytes.
    pub memory: u64,
    /// The free space on the disk that would hold the model, in bytes.
    pub free_disk: u64,
}

impl Machine {
    /// This machine, with the disk that holds `folder`, which may not exist yet. A disk
    /// it cannot find counts as full, so nothing new is offered.
    pub fn detect(folder: &Path) -> Self {
        let system = System::new_with_specifics(
            RefreshKind::nothing().with_memory(MemoryRefreshKind::nothing().with_ram()),
        );
        Self { memory: system.total_memory(), free_disk: free_disk(folder) }
    }
}

/// The free space of the disk mounted deepest above `folder`.
fn free_disk(folder: &Path) -> u64 {
    ravenrs_metrics::free_disk_bytes(folder).unwrap_or(0)
}
