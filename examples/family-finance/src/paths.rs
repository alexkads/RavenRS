//! Where the app keeps things.

use std::path::PathBuf;

/// `~/.raven-financas`: the database, the exports and what the installer leaves.
pub fn home() -> PathBuf {
    let base = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" });
    base.map_or_else(|| PathBuf::from("."), PathBuf::from).join(".raven-financas")
}

/// The database file.
pub fn database() -> PathBuf {
    home().join("financas.db")
}

/// Where the installer puts the app; the update checks this one and reopens it.
pub fn installed_binary() -> Option<PathBuf> {
    let user_home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(PathBuf::from);
    if cfg!(windows) {
        std::env::var_os("LOCALAPPDATA")
            .map(|base| PathBuf::from(base).join("Programs").join("RavenFinancas").join("family-finance.exe"))
    } else if cfg!(target_os = "macos") {
        user_home.map(|h| h.join("Applications/Raven Finanças.app/Contents/MacOS/family-finance"))
    } else {
        user_home.map(|h| h.join(".local/bin/family-finance"))
    }
}
