//! What a machine has room for.

use std::path::Path;

use sysinfo::{Disks, MemoryRefreshKind, RefreshKind, System};

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
    let absolute = std::path::absolute(folder).unwrap_or_else(|_| folder.to_path_buf());
    let disks = Disks::new_with_refreshed_list();
    disks
        .list()
        .iter()
        .filter(|disk| absolute.starts_with(disk.mount_point()))
        .max_by_key(|disk| disk.mount_point().as_os_str().len())
        .map_or(0, sysinfo::Disk::available_space)
}
