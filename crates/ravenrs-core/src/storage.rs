//! Somewhere to keep named pieces of data between runs: a file on the desktop, the browser's
//! IndexedDB on the web, memory in tests. Reading is synchronous, so a store that is slow to
//! read, like the browser's, loads everything before the app opens.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fmt;

/// Named pieces of data kept between runs.
///
/// ```
/// use ravenrs_core::{MemoryStorage, Storage};
///
/// let storage = MemoryStorage::default();
/// storage.write("records", b"{}")?;
/// assert_eq!(storage.read("records")?, Some(b"{}".to_vec()));
/// storage.remove("records")?;
/// assert_eq!(storage.read("records")?, None);
/// # Ok::<(), String>(())
/// ```
pub trait Storage {
    /// What is kept under `name`, or `None` if nothing is.
    fn read(&self, name: &str) -> Result<Option<Vec<u8>>, String>;
    /// Keeps `bytes` under `name`, in place of what was there.
    fn write(&self, name: &str, bytes: &[u8]) -> Result<(), String>;
    /// Forgets what is kept under `name`.
    fn remove(&self, name: &str) -> Result<(), String>;
}

/// Pieces of data kept in memory, gone when the program ends: for tests and previews.
#[derive(Default)]
pub struct MemoryStorage(RefCell<BTreeMap<String, Vec<u8>>>);

impl fmt::Debug for MemoryStorage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MemoryStorage").field("names", &self.0.borrow().keys().collect::<Vec<_>>()).finish()
    }
}

impl Storage for MemoryStorage {
    fn read(&self, name: &str) -> Result<Option<Vec<u8>>, String> {
        Ok(self.0.borrow().get(name).cloned())
    }

    fn write(&self, name: &str, bytes: &[u8]) -> Result<(), String> {
        self.0.borrow_mut().insert(name.to_owned(), bytes.to_vec());
        Ok(())
    }

    fn remove(&self, name: &str) -> Result<(), String> {
        self.0.borrow_mut().remove(name);
        Ok(())
    }
}

impl<S: Storage + ?Sized> Storage for std::rc::Rc<S> {
    fn read(&self, name: &str) -> Result<Option<Vec<u8>>, String> {
        (**self).read(name)
    }

    fn write(&self, name: &str, bytes: &[u8]) -> Result<(), String> {
        (**self).write(name, bytes)
    }

    fn remove(&self, name: &str) -> Result<(), String> {
        (**self).remove(name)
    }
}

/// Pieces of data kept as files in a folder on the desktop, one file per name. A write goes
/// to a draft first and then takes the file's place, so a crash never leaves half a file.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Debug)]
pub struct FileStorage {
    folder: std::path::PathBuf,
}

#[cfg(not(target_arch = "wasm32"))]
impl FileStorage {
    /// Files in `folder`, which is made when first written to.
    pub fn new(folder: impl Into<std::path::PathBuf>) -> Self {
        Self { folder: folder.into() }
    }

    fn path(&self, name: &str) -> Result<std::path::PathBuf, String> {
        let plain = !name.is_empty()
            && name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
            && !name.starts_with('.');
        if plain {
            Ok(self.folder.join(name))
        } else {
            Err(format!("{name:?} is not a plain name: use letters, digits, '-', '_' and '.'"))
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl Storage for FileStorage {
    fn read(&self, name: &str) -> Result<Option<Vec<u8>>, String> {
        let path = self.path(name)?;
        match std::fs::read(&path) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(format!("{}: {e}", path.display())),
        }
    }

    fn write(&self, name: &str, bytes: &[u8]) -> Result<(), String> {
        let path = self.path(name)?;
        let failed = |e: std::io::Error| format!("{}: {e}", path.display());
        std::fs::create_dir_all(&self.folder).map_err(failed)?;
        let draft = path.with_extension("draft");
        std::fs::write(&draft, bytes).map_err(failed)?;
        std::fs::rename(&draft, &path).map_err(failed)
    }

    fn remove(&self, name: &str) -> Result<(), String> {
        let path = self.path(name)?;
        match std::fs::remove_file(&path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(format!("{}: {e}", path.display())),
            _ => Ok(()),
        }
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;

    #[test]
    fn files_keep_what_was_written_and_refuse_paths() {
        let folder = std::env::temp_dir().join(format!("ravenrs-storage-{}", std::process::id()));
        let storage = FileStorage::new(&folder);
        assert_eq!(storage.read("records"), Ok(None));
        assert_eq!(storage.write("records", b"one"), Ok(()));
        assert_eq!(storage.write("records", b"two"), Ok(()));
        assert_eq!(storage.read("records"), Ok(Some(b"two".to_vec())));
        assert!(storage.write("../outside", b"x").is_err());
        assert!(storage.read(".hidden").is_err());
        assert_eq!(storage.remove("records"), Ok(()));
        assert_eq!(storage.remove("records"), Ok(()), "nothing to forget is fine");
        assert_eq!(storage.read("records"), Ok(None));
        let _ = std::fs::remove_dir_all(folder);
    }
}
