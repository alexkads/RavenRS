//! Reports of the lists of a business application: an action that lays out a list and shows it in
//! the document viewer, from where it prints and exports.
//!
//! ```
//! use ravenrs_app::{Application, EntityInfo, FieldInfo, FieldKind};
//! use ravenrs_app_report::ReportModule;
//! use ravenrs_app_ui::SystemModule;
//!
//! let account = EntityInfo::new("Account")
//!     .navigation("Records")
//!     .field(FieldInfo::new("id", FieldKind::Integer).key())
//!     .field(FieldInfo::new("name", FieldKind::Text));
//! let application = Application::builder("Accounts")
//!     .module(SystemModule::new())
//!     .module(ReportModule::new("Report"))
//!     .entity(account)
//!     .build()?;
//! # let _ = application;
//! # Ok::<(), ravenrs_app::ModelError>(())
//! ```

use std::rc::Rc;

mod analytics;
mod print;
mod stored;

pub use analytics::{ANALYTICS_DASHBOARD, ANALYTICS_TILE, AnalyticsModule, analytics_entities};
#[cfg(not(target_arch = "wasm32"))]
pub use print::send_to_printer;
pub use print::{design_stored_report, label_bytes, render_stored_report, stored_reports_for};
pub use stored::{PT_BR, REPORT_DEFINITION, ReportWords, StoredReportsModule, report_entity};

use ravenrs_app::{Action, ActionCategory, Module, TargetScope, ViewController, ViewKind};
use ravenrs_app_ui::Session;
use ravenrs_grid_export::document_of;
use ravenrs_report_viewer::{DocumentViewer, ViewerState};
use ravenrs_widgets::AnyView;

/// The id of the action that shows the report of a list.
pub const REPORT: &str = "ListReport";

/// How tall the viewer is, inside its window.
const VIEWER_HEIGHT: f32 = 640.0;

struct ReportController {
    caption: String,
}

impl ViewController for ReportController {
    fn id(&self) -> &str {
        "ListReport"
    }

    fn scope(&self) -> TargetScope {
        TargetScope::any().view_kind(ViewKind::List)
    }

    fn actions(&self) -> Vec<Action> {
        vec![
            Action::simple(REPORT, self.caption.clone())
                .category(ActionCategory::Export)
                .icon("table")
                .on_execute(|cx| {
                    let session =
                        Session::of(cx).ok_or_else(|| "this action needs the screens".to_owned())?;
                    let view = cx.view.as_ref().ok_or_else(|| "this action needs a list".to_owned())?;
                    let table = session.list_table(view).ok_or_else(|| view.id.clone())?;
                    let title = table.title.clone();
                    let document = Rc::new(document_of(&table));
                    let state = ViewerState::new();
                    session.present(title, move || {
                        let viewer = DocumentViewer::new(document.clone(), &state);
                        Box::new(ravenrs_widgets::ViewExt::height(viewer, VIEWER_HEIGHT)) as AnyView
                    });
                    Ok(())
                }),
        ]
    }
}

/// Gives every list an action that shows it as a report.
#[derive(Clone, Debug)]
pub struct ReportModule {
    caption: String,
}

impl ReportModule {
    /// With this word on the action.
    pub fn new(caption: impl Into<String>) -> Self {
        Self { caption: caption.into() }
    }
}

impl Module for ReportModule {
    fn name(&self) -> &str {
        "Reports"
    }

    fn view_controllers(&self) -> Vec<Box<dyn ViewController>> {
        vec![Box::new(ReportController { caption: self.caption.clone() })]
    }
}
