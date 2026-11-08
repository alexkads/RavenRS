//! Where a layer's difference is kept.

use std::cell::RefCell;

use super::layers::Diff;
use crate::ModelError;

/// Keeps one layer's difference.
pub trait ModelStore {
    /// What it holds; `None` when nothing was kept yet.
    fn load(&self) -> Result<Option<Diff>, ModelError>;
    /// Keeps this.
    fn save(&self, difference: &Diff) -> Result<(), ModelError>;
}

/// A difference that came inside the program, as a module's does. It cannot change.
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct EmbeddedStore(pub &'static str);

impl ModelStore for EmbeddedStore {
    fn load(&self) -> Result<Option<Diff>, ModelError> {
        Diff::parse(self.0).map(Some)
    }

    fn save(&self, _difference: &Diff) -> Result<(), ModelError> {
        Err(ModelError("a difference inside the program cannot be saved".into()))
    }
}

/// A difference that lasts as long as the program, where nothing else is at hand.
#[derive(Debug, Default)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct MemoryModelStore(RefCell<Option<Diff>>);

impl ModelStore for MemoryModelStore {
    fn load(&self) -> Result<Option<Diff>, ModelError> {
        Ok(self.0.borrow().clone())
    }

    fn save(&self, difference: &Diff) -> Result<(), ModelError> {
        *self.0.borrow_mut() = Some(difference.clone());
        Ok(())
    }
}

/// The person's own settings kept in a [`Storage`](ravenrs_core::Storage) under a name: a file on
/// the desktop, the browser's IndexedDB on the web, beside the records.
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct StoredModel {
    storage: std::rc::Rc<dyn ravenrs_core::Storage>,
    name: String,
}

impl std::fmt::Debug for StoredModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StoredModel").field("name", &self.name).finish_non_exhaustive()
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl StoredModel {
    /// Settings kept in `storage` under `name`.
    pub fn new(storage: impl ravenrs_core::Storage + 'static, name: impl Into<String>) -> Self {
        Self { storage: std::rc::Rc::new(storage), name: name.into() }
    }
}

impl ModelStore for StoredModel {
    fn load(&self) -> Result<Option<Diff>, ModelError> {
        let failed = |e: String| ModelError(format!("{}: {e}", self.name));
        let Some(bytes) = self.storage.read(&self.name).map_err(failed)? else { return Ok(None) };
        let text = String::from_utf8(bytes).map_err(|e| failed(e.to_string()))?;
        Diff::parse(&text).map(Some).map_err(|e| failed(e.to_string()))
    }

    fn save(&self, difference: &Diff) -> Result<(), ModelError> {
        self.storage
            .write(&self.name, difference.to_json().as_bytes())
            .map_err(|e| ModelError(format!("{}: {e}", self.name)))
    }
}

/// A difference in a file. A failure while saving leaves the old file whole.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Debug)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct FileStore(pub std::path::PathBuf);

#[cfg(not(target_arch = "wasm32"))]
impl ModelStore for FileStore {
    fn load(&self) -> Result<Option<Diff>, ModelError> {
        match std::fs::read_to_string(&self.0) {
            Ok(text) => {
                Diff::parse(&text).map(Some).map_err(|e| ModelError(format!("{}: {e}", self.0.display())))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(ModelError(format!("{}: {e}", self.0.display()))),
        }
    }

    fn save(&self, difference: &Diff) -> Result<(), ModelError> {
        let failed = |e: std::io::Error| ModelError(format!("{}: {e}", self.0.display()));
        if let Some(folder) = self.0.parent() {
            std::fs::create_dir_all(folder).map_err(failed)?;
        }
        let draft = self.0.with_extension("tmp");
        std::fs::write(&draft, difference.to_json()).map_err(failed)?;
        std::fs::rename(&draft, &self.0).map_err(failed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ModelLayer, ModelLayers, ModelNode};

    fn node() -> ModelNode {
        ModelNode::new("Application").child(ModelNode::new("Views"))
    }

    #[test]
    fn embedded_differences_load_and_refuse_saving() {
        let store = EmbeddedStore(r#"{ "Views": { "Caption": "x" } }"#);
        let mut layers = ModelLayers::new(node());
        let module = ModelLayer::Module("m".into());
        layers.load(&module, &store).expect("loads");
        assert_eq!(layers.merged().root.at(&["Views"]).and_then(|n| n.text("Caption")), Some("x"));
        assert!(layers.save(&module, &store).is_err());
    }

    #[test]
    fn a_memory_store_gives_back_what_it_was_given() {
        let store = MemoryModelStore::default();
        let mut layers = ModelLayers::new(node());
        layers.load(&ModelLayer::User, &store).expect("nothing is fine");
        layers.set(&ModelLayer::User, &["Views"], "Caption", "x".into());
        layers.save(&ModelLayer::User, &store).expect("saves");
        let mut again = ModelLayers::new(node());
        again.load(&ModelLayer::User, &store).expect("loads");
        assert_eq!(again, layers);
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn a_file_store_keeps_it_on_disk() {
        let folder = std::env::temp_dir().join(format!("ravenrs-app-{}", std::process::id()));
        let store = FileStore(folder.join("deep").join("user.json"));
        assert_eq!(store.load(), Ok(None));
        let mut layers = ModelLayers::new(node());
        layers.set(&ModelLayer::User, &["Views"], "Caption", "x".into());
        layers.save(&ModelLayer::User, &store).expect("saves");
        assert_eq!(store.load().expect("loads").as_ref(), layers.difference(&ModelLayer::User));
        std::fs::write(&store.0, "{ broken").expect("writes");
        assert!(store.load().is_err_and(|e| e.0.contains("user.json")), "says which file");
        std::fs::remove_dir_all(folder).expect("cleans up");
    }
}
