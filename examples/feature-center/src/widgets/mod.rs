//! The components of the widget families, one page each, every public feature shown.

pub mod graficos;

use ravenrs::prelude::AnyView;
use ravenrs_app_ui::Session;

/// Registers the control of every component page with the session.
pub fn register(session: &Session) {
    session.register_control(graficos::fluxo::ID, || Box::new(graficos::fluxo::FlowPage) as AnyView);
    session.register_control(graficos::cabos::ID, || Box::new(graficos::cabos::CablesPage) as AnyView);
}
