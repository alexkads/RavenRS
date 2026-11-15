//! The demonstrations of the application framework that are screens of their own rather than an
//! entity's list: what is written in code against the model, the parts of the screens, storage and
//! the server, and the ready-made modules. Each one is a dashboard of the center.

pub mod infra;
pub mod modelo;
pub mod modulos;
pub mod seguranca;
pub mod telas;

use ravenrs_app::{ApplicationBuilder, DashboardInfo};
use ravenrs_app_ui::{App, Session};

/// The dashboards of these screens, for the navigation.
pub fn dashboards() -> Vec<DashboardInfo> {
    [
        modelo::dashboards(),
        seguranca::dashboards(),
        telas::dashboards(),
        infra::dashboards(),
        modulos::dashboards(),
    ]
    .concat()
}

/// The controls those dashboards show.
pub fn register(session: &Session) {
    modelo::register(session);
    seguranca::register(session);
    telas::register(session);
    infra::register(session);
    modulos::register(session);
}

/// What the ready-made modules and the screens add to the application each person gets.
pub fn extend(app: App) -> App {
    telas::extend(modulos::extend(app))
}

/// A dashboard of one control, the way every screen here is put in the navigation.
pub fn screen(id: &str, caption: &str, group: &str, icon: &str) -> DashboardInfo {
    DashboardInfo::new(id, caption).navigation(group).icon(icon).control(caption, id)
}

/// What the modules add to the model storage has to know: their entities come with them.
pub fn storage(builder: ApplicationBuilder) -> ApplicationBuilder {
    modulos::storage(builder)
}

/// What the web API of the center adds to the one the center builds.
#[cfg(not(target_arch = "wasm32"))]
pub fn serve(service: ravenrs_app_server::ApiService) -> ravenrs_app_server::ApiService {
    infra::serve(service)
}
