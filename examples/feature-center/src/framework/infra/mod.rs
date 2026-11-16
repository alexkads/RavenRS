//! Storage and the server, each a screen of its own in the group "Dados e servidor": records in
//! memory and kept somewhere, backups of SQLite, entities on tables that already exist, PostgreSQL
//! and PostgREST, and the web API with its webhooks and the events it sends out.
//!
//! What needs a driver of the desktop (SQLite, PostgreSQL, the server) says so on the web.

pub mod copias;
pub mod integracoes;
pub mod memoria;
pub mod postgres;
pub mod sql;

use ravenrs::prelude::*;
use ravenrs_app::DashboardInfo;
use ravenrs_app_ui::{App, Session};

/// The group of the navigation of these screens.
pub const GROUP: &str = "Dados e servidor";

/// A screen: its id, its caption, its icon and what makes it.
type Screen = (&'static str, &'static str, &'static str, fn() -> AnyView);

/// The screens, in the order of the navigation.
const SCREENS: [Screen; 5] = [
    (memoria::ID, memoria::PAGE, "database", || Box::new(memoria::MemoryPage)),
    (copias::ID, copias::PAGE, "hard-drive", copias::page),
    (sql::ID, sql::PAGE, "rows-3", sql::page),
    (postgres::ID, postgres::PAGE, "server", postgres::page),
    (integracoes::ID, integracoes::PAGE, "plug", integracoes::page),
];

/// The dashboards of this part.
pub fn dashboards() -> Vec<DashboardInfo> {
    SCREENS.iter().map(|(id, caption, icon, _)| super::screen(id, caption, GROUP, icon)).collect()
}

/// The controls of [`dashboards`].
pub fn register(session: &Session) {
    for (id, _, _, make) in SCREENS {
        session.register_control(id, make);
    }
}

/// What this part adds to the application.
pub fn extend(app: App) -> App {
    app
}

/// What this part adds to the web API of the center: an endpoint of its own, run as whoever calls.
#[cfg(not(target_arch = "wasm32"))]
pub fn serve(service: ravenrs_app_server::ApiService) -> ravenrs_app_server::ApiService {
    integracoes::center_endpoint(service)
}

/// A screen that only the desktop build has, saying so on the web.
#[cfg(target_arch = "wasm32")]
pub(crate) fn desktop_only(what: &str) -> AnyView {
    Box::new(
        VStack::new((
            Text::new("Só na versão para desktop").style(TextStyle::Headline),
            Text::new(what.to_owned()).foreground(Semantic::InkMuted),
        ))
        .spacing(8.0)
        .padding(16.0)
        .align(Align::Start),
    )
}

/// The parts every screen here has: a title, what it shows, the buttons, what is there now and a
/// line with the last thing that happened.
pub(crate) fn layout(
    cx: &BuildCx<'_>,
    intro: &str,
    buttons: Vec<AnyView>,
    shown: Vec<String>,
    said: String,
) -> AnyView {
    let narrow = cx.viewport().width < 720.0;
    let lines: Vec<AnyView> = shown.into_iter().map(|line| Box::new(Text::new(line)) as AnyView).collect();
    Box::new(
        VStack::new((
            Text::new(intro.to_owned()).foreground(Semantic::InkMuted),
            HStack::new(buttons).spacing(8.0).wrap(),
            VStack::new(lines).spacing(4.0).align(Align::Start),
            Text::new(said),
        ))
        .spacing(12.0)
        .padding(if narrow { 0.0 } else { 16.0 })
        .align(Align::Stretch),
    )
}
