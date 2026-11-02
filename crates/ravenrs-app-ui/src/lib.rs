//! The screens of a business application, generated from the model in `ravenrs-app`.
//!
//! ```
//! use ravenrs_app::{Application, EntityInfo, FieldInfo, FieldKind, MemoryDatabase};
//! use ravenrs_app_ui::{AppShell, SystemModule};
//!
//! let account = EntityInfo::new("Account")
//!     .navigation("Records")
//!     .field(FieldInfo::new("id", FieldKind::Integer).key())
//!     .field(FieldInfo::new("name", FieldKind::Text));
//! let database = MemoryDatabase::new(vec![account.clone()])?;
//! let application = Application::builder("Accounts").module(SystemModule::new()).entity(account).build()?;
//! let shell = AppShell::new(application, move || database.space());
//! let mut ui = ravenrs_widgets::Ui::new(shell, ravenrs_style::Theme::raven_dark());
//! ui.update();
//! # Ok::<(), ravenrs_app::ModelError>(())
//! ```

mod container;
mod detail_view;
mod editors;
mod list_view;
mod session;
mod shell;
mod system;
mod values;

pub use container::ActionContainer;
pub use detail_view::DetailView;
pub use editors::{EditorRegistry, EditorRequest};
pub use list_view::{ListView, OpenHandler};
pub use session::{Session, Words};
pub use shell::{AppShell, FrameTemplate, MainTemplate, TemplateParts};
pub use system::{SystemModule, ids};
