//! The apps a scenario opens apart, from `ravenrs-e2e-runner`, and Raven Finanças, which the
//! Studio knows by the declaration in its own `Cargo.toml`, read when the Studio is built, so
//! the installed Studio opens the installed app with no checkout.

use std::path::Path;

pub use ravenrs_e2e_runner::app::*;

use ravenrs_e2e_runner::session::{Failure, Session};

/// The name Raven Finanças declares, which `--run financas` opens.
pub const FINANCE_APP: &str = "Raven Finanças";

/// Raven Finanças' own declaration, read when the Studio is built.
const FINANCE_MANIFEST: &str = include_str!("../../../examples/family-finance/Cargo.toml");

/// Makes Raven Finanças known, after any app learned from a project, with its code when the
/// Studio runs inside a RavenRS checkout.
fn know_finance() {
    if let Some(app) = App::declared_in(FINANCE_MANIFEST) {
        know(App { source: checkout_with(&app.package), ..app });
    }
}

/// The app named `name`: one learned from a project, or Raven Finanças.
pub fn known(name: &str) -> Option<App> {
    know_finance();
    ravenrs_e2e_runner::app::known(name)
}

/// The scenarios the app named `name` brings, read from its hello.
pub fn scenarios(name: &str) -> Option<Result<Vec<ravenrs_e2e_wire::Scenario>, String>> {
    know_finance();
    ravenrs_e2e_runner::app::scenarios(name)
}

/// Opens the app named `name` apart, if it is one the Studio knows.
pub fn open(name: &str) -> Option<Result<Session, Box<Failure>>> {
    know_finance();
    ravenrs_e2e_runner::app::open(name)
}

/// Opens Raven Finanças from `binary`, as its variable does.
pub fn open_finance_from(binary: &Path) -> Result<Session, Box<Failure>> {
    open_from(binary)
}
