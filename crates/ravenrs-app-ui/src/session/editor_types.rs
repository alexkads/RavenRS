//! What a field editor is asked to show, and the registry of editors by name, which the session
//! keeps.

use std::collections::BTreeMap;
use std::rc::Rc;

use ravenrs_app::{FieldInfo, Value};
use ravenrs_reactive::State;
use ravenrs_widgets::AnyView;

use crate::session::Session;

/// What a control is being asked for.
#[derive(Clone, Debug)]
#[non_exhaustive]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct EditorRequest {
    /// The field it edits.
    pub field: FieldInfo,
    /// The value, one state per field, so typing does not touch the whole record.
    pub value: State<Value>,
    /// What people read as its name.
    pub caption: String,
    /// It shows but does not change.
    pub read_only: bool,
    /// The application it lives in.
    pub session: Session,
}

pub(crate) type EditorFactory = Rc<dyn Fn(&EditorRequest) -> AnyView>;

/// Which control edits which kind of field, by name. The model's `Editor` setting
/// picks one, so a field can change its control without touching code.
#[derive(Clone, Default)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct EditorRegistry(pub(crate) BTreeMap<String, EditorFactory>);

impl std::fmt::Debug for EditorRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self.0.keys()).finish()
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose(part = "part1"))]
impl EditorRegistry {
    /// Registers a control by name, replacing one of the same name. The name of a
    /// kind of field, such as `Text`, replaces the built-in for it.
    pub fn register(
        &mut self,
        name: impl Into<String>,
        factory: impl Fn(&EditorRequest) -> AnyView + 'static,
    ) {
        self.0.insert(name.into(), Rc::new(factory));
    }
}
