//! The application model: a tree of settings in layers, generated from the entities and
//! changed by differences in JSON.

mod captions;
mod generate;
mod layers;
mod store;

pub(crate) use captions::{captions_of, translation_of};
pub(crate) use generate::{add_custom_fields, add_dashboard, follow_captions};
pub use generate::{check_model, generate_model};
pub use layers::{Diff, MergedModel, ModelLayer, ModelLayers, UnusableNode};
#[cfg(not(target_arch = "wasm32"))]
pub use store::FileStore;
pub use store::{EmbeddedStore, MemoryModelStore, ModelStore, StoredModel};
