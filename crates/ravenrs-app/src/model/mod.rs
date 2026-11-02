//! The application model: a tree of settings in layers, generated from the entities and
//! changed by differences in JSON.

mod generate;
mod layers;
mod node;
mod store;

pub use generate::{check_model, generate_model};
pub use layers::{Diff, MergedModel, ModelLayer, ModelLayers, UnusableNode};
pub use node::ModelNode;
#[cfg(not(target_arch = "wasm32"))]
pub use store::FileStore;
pub use store::{EmbeddedStore, MemoryModelStore, ModelStore};
