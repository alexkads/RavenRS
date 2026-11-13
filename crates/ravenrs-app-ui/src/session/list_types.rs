//! What a list hands around: its source of records, its rows as a table, and the request a list
//! editor gets. The session keeps them, so they live below the views.

use std::rc::Rc;

use ravenrs_app::{EntityInfo, FieldInfo, Object, Value, ViewInfo};
use ravenrs_widgets::AnyView;
use ravenrs_widgets::grid::GridTable;

use crate::session::Session;

/// What opening a row of a list does, with its key.
pub type OpenHandler = Rc<dyn Fn(&Value)>;

/// What gives a list its records when no storage keeps them.
pub type ListSource = Rc<dyn Fn() -> Vec<Object>>;

/// What a list on screen shows now, as a table, for a report to take the same rows.
pub(crate) type ShownList = Rc<dyn Fn() -> Option<GridTable>>;

/// What a list editor is asked to show: the records as the list reads them, and how to act on them.
#[non_exhaustive]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct ListEditorRequest {
    /// The application it lives in.
    pub session: Session,
    /// The view it shows.
    pub view: ViewInfo,
    /// The entity of the records.
    pub entity: EntityInfo,
    /// The fields the list shows, with what their columns say.
    pub columns: Vec<(FieldInfo, String)>,
    /// The records, in the list's order.
    pub objects: Rc<Vec<Object>>,
    /// Opens a record, by its key.
    pub on_open: OpenHandler,
    /// Says which records are selected, which turns actions on and off.
    pub on_select: Rc<dyn Fn(Vec<Object>)>,
}

impl std::fmt::Debug for ListEditorRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ListEditorRequest").field("view", &self.view).finish_non_exhaustive()
    }
}

/// Shows a list some other way than in a grid.
pub(crate) type ListEditorFactory = Rc<dyn Fn(&ListEditorRequest) -> AnyView>;
