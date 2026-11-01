//! Where the app keeps things, and handing files to the system.

use std::path::{Path, PathBuf};

/// `~/.raven-financas`: the database, the exports and what the installer leaves.
pub fn home() -> PathBuf {
    let base = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" });
    base.map_or_else(|| PathBuf::from("."), PathBuf::from).join(".raven-financas")
}

/// The database file.
pub fn database() -> PathBuf {
    home().join("financas.db")
}

/// Where exported files and backups go.
pub fn exports() -> PathBuf {
    home().join("Exportados")
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

/// Saves `bytes` under the exports folder as `name` and returns the path.
pub fn save_export(name: &str, bytes: &[u8]) -> std::io::Result<PathBuf> {
    let folder = exports();
    std::fs::create_dir_all(&folder)?;
    let path = folder.join(name);
    std::fs::write(&path, bytes)?;
    Ok(path)
}

/// Opens a file with the program the system uses for it.
pub fn open_with_system(path: &Path) {
    let opener = if cfg!(target_os = "macos") {
        std::process::Command::new("open").arg(path).spawn()
    } else if cfg!(target_os = "windows") {
        std::process::Command::new("cmd").args(["/C", "start", ""]).arg(path).spawn()
    } else {
        std::process::Command::new("xdg-open").arg(path).spawn()
    };
    if let Err(e) = opener {
        tracing::warn!(error = %e, "could not open the file");
    }
}
