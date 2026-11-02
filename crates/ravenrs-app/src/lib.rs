//! The model of a business application, with no GUI: entities, a layered
//! application model and object spaces.
//!
//! ```
//! use ravenrs_app::{EntityInfo, FieldInfo, FieldKind, MemoryDatabase, ModelLayers, ModelLayer, ObjectSpace, generate_model};
//!
//! let account = EntityInfo::new("Account")
//!     .caption("Conta")
//!     .navigation("Cadastros")
//!     .field(FieldInfo::new("id", FieldKind::Integer).key())
//!     .field(FieldInfo::new("name", FieldKind::Text).caption("Nome").default_property());
//!
//! // The model: a list, a form and a place in the navigation, changed by a layer.
//! let mut layers = ModelLayers::new(generate_model(std::slice::from_ref(&account))?);
//! layers.set(&ModelLayer::User, &["NavigationItems", "Cadastros"], "Caption", "Registros".into());
//! let model = layers.merged();
//! assert_eq!(model.root.at(&["NavigationItems", "Cadastros"]).and_then(|n| n.text("Caption")), Some("Registros"));
//!
//! // The records: nothing is kept until the commit.
//! let mut space = MemoryDatabase::new(vec![account])?.space();
//! let mut object = space.new_object("Account")?;
//! object.set("name", "Corrente");
//! space.save(object)?;
//! assert!(space.is_modified());
//! space.commit()?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

mod entity;
mod model;
mod space;

pub use entity::{EntityInfo, EnumOption, FieldInfo, FieldKind, ModelError, Object};
#[cfg(not(target_arch = "wasm32"))]
pub use model::FileStore;
pub use model::{
    Diff, EmbeddedStore, MemoryModelStore, MergedModel, ModelLayer, ModelLayers, ModelNode, ModelStore,
    UnusableNode, check_model, generate_model,
};
pub use ravenrs_criteria::{Criteria, Value};
pub use space::{
    Change, MemoryDatabase, MemoryObjectSpace, ObjectSpace, Query, SpaceError, SpaceEvent, SpaceWatcher,
};
