//! Where the content of attached files is kept, apart from the records that point to them.

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;

/// Keeps the content of files, and gives a key for each. A record holds only that key, so reading
/// a list does not read every file.
pub trait AttachmentStore {
    /// Keeps a file, and says the key to find it by.
    fn put(&self, name: &str, bytes: &[u8]) -> Result<String, String>;
    /// The content kept under this key; nothing if it is gone.
    fn get(&self, key: &str) -> Result<Option<Vec<u8>>, String>;
    /// Forgets what is kept under this key.
    fn remove(&self, key: &str) -> Result<(), String>;
}

/// Files that last as long as the program, as on the web with no server.
#[derive(Debug, Default)]
pub struct MemoryAttachmentStore {
    files: RefCell<BTreeMap<String, Vec<u8>>>,
    next: Cell<u64>,
}

impl AttachmentStore for MemoryAttachmentStore {
    fn put(&self, _name: &str, bytes: &[u8]) -> Result<String, String> {
        let key = self.next.get() + 1;
        self.next.set(key);
        self.files.borrow_mut().insert(key.to_string(), bytes.to_vec());
        Ok(key.to_string())
    }

    fn get(&self, key: &str) -> Result<Option<Vec<u8>>, String> {
        Ok(self.files.borrow().get(key).cloned())
    }

    fn remove(&self, key: &str) -> Result<(), String> {
        self.files.borrow_mut().remove(key);
        Ok(())
    }
}
