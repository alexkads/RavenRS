//! Updating by compiling, the way VintageLightbox does it: the rules are in
//! `ravenrs-app-update`, and this is who Raven Finanças is to them.

pub use ravenrs_app_update::{
    Decision, InstallKind, InstalledApp, Memory, News, Notice, NoticeAction, UpdateStatus, Updater, is_newer,
    join_history, notice, parse_history, parse_news,
};

/// Raven Finanças to the installer and the updater. Installed copies only know
/// the addresses they were compiled with, so addresses are only ever added.
pub static APP: InstalledApp = InstalledApp {
    name: "Raven Finanças",
    package: "family-finance",
    home_dir: ".raven-financas",
    bundle_id: "dev.ravenrs.family-finance",
    icons: "examples/family-finance/assets",
    mac_category: "public.app-category.finance",
    linux_categories: "Office;Finance;",
    source: "https://github.com/alexkads/RavenRS/archive/refs/heads/master.tar.gz",
    env_prefix: "FF",
    installed_env: "FAMILY_FINANCE_INSTALLED",
    // `raw` follows the branch at once, and Pages is the copy.
    news_urls: &[
        "https://raw.githubusercontent.com/alexkads/RavenRS/master/examples/family-finance/novidades.json",
        "https://alexkads.github.io/RavenRS/family-finance/novidades.json",
    ],
    history_urls: &[
        "https://raw.githubusercontent.com/alexkads/RavenRS/master/examples/family-finance/historico-de-novidades.json",
        "https://alexkads.github.io/RavenRS/family-finance/historico-de-novidades.json",
    ],
    installer_url: "https://raw.githubusercontent.com/alexkads/RavenRS/master/examples/family-finance/install.cmd",
    issues_url: "https://github.com/alexkads/RavenRS/issues",
    version: env!("CARGO_PKG_VERSION"),
    // The installer sets it when it compiles.
    installed: option_env!("FAMILY_FINANCE_INSTALLED").is_some(),
    history: include_str!("../historico-de-novidades.json"),
};

/// The running version.
pub const CURRENT: &str = APP.version;

/// An updater that does nothing until asked; tests use this one.
pub fn idle() -> Updater {
    Updater::idle(&APP)
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::*;
    use crate::paths;

    #[test]
    fn the_three_release_files_agree() {
        ravenrs_app_update::assert_release_files_agree(Path::new(env!("CARGO_MANIFEST_DIR")));
    }

    #[test]
    fn the_installer_is_the_one_the_template_makes() {
        ravenrs_app_update::assert_installer_is_current(
            &APP,
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("install.cmd"),
        );
    }

    #[test]
    fn the_installer_in_the_repository_is_whole() {
        assert!(ravenrs_app_update::installer_is_whole(include_str!("../install.cmd")));
    }

    #[test]
    fn the_command_by_hand_is_the_one_the_readme_shows() {
        let readme = include_str!("../README.md");
        for windows in [true, false] {
            let command = APP.install_command_on(windows);
            assert!(readme.contains(&command), "{command}");
        }
    }

    #[test]
    fn gnome_finds_the_icon_through_the_window_id() {
        let installer = include_str!("../install.cmd");
        let id = paths::APP_ID;
        assert_eq!(APP.package, id, "the window opens with the installer's id");
        assert!(installer.contains(&format!("share/applications/{id}.desktop")), "the file's name is the id");
        assert!(installer.contains(&format!("StartupWMClass={id}")));
    }

    /// The folders and addresses an installed 0.1.38 knows, as they were written
    /// before they moved to `ravenrs-app-update`: changing one would strand it.
    #[test]
    fn the_folders_and_addresses_stay_where_installed_copies_look() {
        let user = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(PathBuf::from);
        let user = user.unwrap_or_else(|| PathBuf::from("."));
        assert_eq!(APP.home(), user.join(".raven-financas"));
        assert_eq!(APP.installer_log(), user.join(".raven-financas").join("atualizacao.log"));
        assert_eq!(APP.issue_url(589), "https://github.com/alexkads/RavenRS/issues/589");
        if cfg!(windows) {
            let base = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);
            let binary = base.map(|b| b.join("Programs").join("RavenFinancas").join("family-finance.exe"));
            assert_eq!(APP.installed_binary(), binary);
        } else if cfg!(target_os = "macos") {
            const INSIDE: &str = "Raven Finanças.app/Contents/MacOS/family-finance";
            let system = PathBuf::from("/Applications").join(INSIDE);
            let expected = if system.exists() { system } else { user.join("Applications").join(INSIDE) };
            assert_eq!(APP.installed_binary(), Some(expected));
            assert_eq!(
                APP.previous_version(),
                Some(user.join(".raven-financas/anterior/Raven Finanças.app"))
            );
        } else {
            assert_eq!(APP.installed_binary(), Some(user.join(".local/bin/family-finance")));
            assert_eq!(APP.previous_version(), Some(user.join(".local/bin/family-finance.anterior")));
        }
        let installer = include_str!("../install.cmd");
        for kept in [
            r#"CASA="${FF_CASA:-$HOME/.raven-financas}""#,
            r#"$Casa = if ($env:FF_CASA) { $env:FF_CASA } else { "$env:USERPROFILE\.raven-financas" }"#,
            r#"$Destino = if ($env:FF_DESTINO) { $env:FF_DESTINO } else { "$env:LOCALAPPDATA\Programs\RavenFinancas" }"#,
            r#"if not "%FF_SEM_PAUSA%"=="1" pause"#,
            "FAMILY_FINANCE_INSTALLED=1 cargo build --release --manifest-path examples/family-finance/Cargo.toml -p family-finance",
            "<key>CFBundleIdentifier</key><string>dev.ravenrs.family-finance</string>",
        ] {
            assert!(installer.contains(kept), "{kept}");
        }
        assert_eq!(
            APP.news_urls[0],
            "https://raw.githubusercontent.com/alexkads/RavenRS/master/examples/family-finance/novidades.json"
        );
        assert!(Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/icon.icns").exists());
    }
}
