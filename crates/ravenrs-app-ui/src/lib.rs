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
mod detail_view;
mod editors;
mod list_view;
mod session;
mod shell;
mod system;
mod values;

pub use app::{App, Language};
pub use container::ActionContainer;
pub use detail_view::DetailView;
pub use editors::{EditorRegistry, EditorRequest};
pub use list_view::{ListView, OpenHandler};
pub use session::{Session, Words};
pub use shell::{AppShell, FrameTemplate, MainTemplate, TemplateParts};
pub use system::{SystemModule, ids};
