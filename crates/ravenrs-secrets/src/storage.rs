//! Where sealed records go. What is kept here is never the value: only what was sealed, who may do
//! what with it, and whether it was revoked.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};

use serde::{Deserialize, Serialize};

use crate::vault::{Access, SecretError};

/// One secret as it is kept.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Sealed {
    /// What it is, as `github-token`.
    pub kind: String,
    /// Raised by each rotation.
    pub version: u32,
    /// The nonce of this version, in hexadecimal.
    pub nonce: String,
    /// The value sealed with its id, kind and version, in hexadecimal.
    pub cipher: String,
    /// Revoked secrets are kept, so what still refers to one says why it fails.
    pub revoked: bool,
    /// Who may do what.
    pub grants: BTreeMap<String, Vec<Access>>,
}

/// Keeps sealed records by id.
pub trait SecretStorage {
    /// The record of `id`, if there is one.
    fn load(&self, id: &str) -> Result<Option<Sealed>, SecretError>;
    /// Keeps `sealed` as `id`, in place of what was there.
    fn save(&self, id: &str, sealed: &Sealed) -> Result<(), SecretError>;
    /// Forgets `id`.
    fn remove(&self, id: &str) -> Result<(), SecretError>;
    /// Every id kept.
    fn ids(&self) -> Result<Vec<String>, SecretError>;
}

/// Records in memory, for tests and for what does not outlive the program. Clones share them.
#[derive(Clone, Debug, Default)]
pub struct MemoryStorage {
    records: Arc<Mutex<BTreeMap<String, Sealed>>>,
}

impl MemoryStorage {
    /// Nothing kept yet.
    pub fn new() -> Self {
        Self::default()
    }
}

impl SecretStorage for MemoryStorage {
    fn load(&self, id: &str) -> Result<Option<Sealed>, SecretError> {
        Ok(self.records.lock().unwrap_or_else(PoisonError::into_inner).get(id).cloned())
    }

    fn save(&self, id: &str, sealed: &Sealed) -> Result<(), SecretError> {
        self.records.lock().unwrap_or_else(PoisonError::into_inner).insert(id.to_owned(), sealed.clone());
        Ok(())
    }

    fn remove(&self, id: &str) -> Result<(), SecretError> {
        self.records.lock().unwrap_or_else(PoisonError::into_inner).remove(id);
        Ok(())
    }

    fn ids(&self) -> Result<Vec<String>, SecretError> {
        Ok(self.records.lock().unwrap_or_else(PoisonError::into_inner).keys().cloned().collect())
    }
}

/// One JSON file per secret in a folder, readable by its owner only on Linux and macOS.
#[derive(Clone, Debug)]
pub struct FileStorage {
    folder: PathBuf,
}

/// Only ids a vault makes name files, so no id can reach outside the folder.
fn checked(id: &str) -> Result<&str, SecretError> {
    let fine = !id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-');
    if fine { Ok(id) } else { Err(SecretError::NotFound) }
}

fn storage(error: impl std::fmt::Display) -> SecretError {
    SecretError::Storage(error.to_string())
}

impl FileStorage {
    /// Records in `folder`, made if missing.
    pub fn new(folder: impl Into<PathBuf>) -> Result<Self, SecretError> {
        let folder = folder.into();
        std::fs::create_dir_all(&folder).map_err(storage)?;
        Ok(Self { folder })
    }

    fn path(&self, id: &str) -> Result<PathBuf, SecretError> {
        Ok(self.folder.join(format!("{}.json", checked(id)?)))
    }
}

impl SecretStorage for FileStorage {
    fn load(&self, id: &str) -> Result<Option<Sealed>, SecretError> {
        let path = self.path(id)?;
        match std::fs::read_to_string(&path) {
            Ok(text) => serde_json::from_str(&text).map(Some).map_err(|_| SecretError::Tampered),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(storage(error)),
        }
    }

    fn save(&self, id: &str, sealed: &Sealed) -> Result<(), SecretError> {
        let path = self.path(id)?;
        let text = serde_json::to_string_pretty(sealed).map_err(storage)?;
        // Written beside and moved over, so a crash leaves the old record or the new one.
        let partial = path.with_extension("json.part");
        write_mine(&partial, &text).map_err(storage)?;
        std::fs::rename(&partial, &path).map_err(storage)
    }

    fn remove(&self, id: &str) -> Result<(), SecretError> {
        match std::fs::remove_file(self.path(id)?) {
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(storage(error)),
            _ => Ok(()),
        }
    }

    fn ids(&self) -> Result<Vec<String>, SecretError> {
        let mut ids = Vec::new();
        for entry in std::fs::read_dir(&self.folder).map_err(storage)? {
            let name = entry.map_err(storage)?.file_name();
            if let Some(id) = name.to_str().and_then(|name| name.strip_suffix(".json")) {
                ids.push(id.to_owned());
            }
        }
        ids.sort();
        Ok(ids)
    }
}

#[cfg(unix)]
fn write_mine(path: &std::path::Path, text: &str) -> std::io::Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let mut file =
        std::fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(path)?;
    file.write_all(text.as_bytes())?;
    file.sync_all()
}

#[cfg(not(unix))]
fn write_mine(path: &std::path::Path, text: &str) -> std::io::Result<()> {
    std::fs::write(path, text)
}
