//! Where the app keeps things.

use std::path::PathBuf;

use crate::update::APP;

/// The app's name to the system: on Linux, its `.desktop` file and the window that belongs to it.
/// The installer names them after the package, so this is the package.
pub const APP_ID: &str = APP.package;

/// `~/.raven-financas`: the database, the exports and what the installer leaves.
pub fn home() -> PathBuf {
    APP.home()
}

/// The database file.
pub fn database() -> PathBuf {
    home().join("financas.db")
}
