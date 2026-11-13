//! The screens of a business application, generated from the model in `ravenrs-app`.
//!
//! ```
//! use ravenrs_app::BusinessObject;
//! use ravenrs_app_ui::App;
//!
//! #[derive(BusinessObject)]
//! #[object(navigation = "Records")]
//! struct Account {
//!     #[field(key)]
//!     id: i64,
//!     name: String,
//! }
//!
//! let window = App::new("Accounts").entity::<Account>().build()?;
//! let mut ui = ravenrs_widgets::Ui::new(window, ravenrs_style::Theme::raven_dark());
//! ui.update();
//! # Ok::<(), ravenrs_app::ModelError>(())
//! ```

mod app;
mod container;
mod dashboard;
mod detail_view;
mod editors;
mod form;
mod hotkeys;
pub mod ids;
mod jobs;
mod list_editors;
mod list_view;
#[cfg(feature = "live")]
mod live;
#[cfg(feature = "live")]
mod live_list;
mod model_editor;
mod nested;
mod pane;
mod password;
mod reminders;
mod security;
mod session;
mod shell;
mod system;
mod values;
mod words;
// What the generated registrations name views as.
#[cfg(feature = "live")]
#[allow(unused_imports)]
use ravenrs_widgets::AnyView;

pub use app::App;
pub use container::{ActionButton, ActionContainer};
pub use dashboard::DashboardView;
pub use detail_view::DetailView;
pub use form::{EntityForm, FormText};
pub use list_editors::{
    CALENDAR_EDITOR, CHART_EDITOR, SCHEDULER_EDITOR, TREE_EDITOR, chart_view, summary_text,
};
pub use list_view::{CellText, ColumnShape, GridShape, ListView};
pub use model_editor::ModelEditor;
pub use pane::{EntityPane, PaneWords};
pub use ravenrs_app_i18n::{Catalog, Language, Texts};
pub use security::{SecuredApp, SignInAction};
pub use session::{CollectionSource, FormCaption, Session};
pub use session::{EditorRegistry, EditorRequest};
pub use session::{JobInfo, JobProgress};
pub use session::{ListEditorRequest, ListSource, OpenHandler};
pub use shell::{AppShell, CounterTemplate, FrameTemplate, MainTemplate, TemplateParts};
pub use system::{CloneModule, SystemModule};
pub use words::Words;

/// Registers this crate's types with a playground's interpreter (feature `live`).
#[cfg(feature = "live")]
pub fn live_expose(registry: &mut ravenrs_live::Registry) {
    live_list::expose_all(registry);
    live::expose(registry);
}
