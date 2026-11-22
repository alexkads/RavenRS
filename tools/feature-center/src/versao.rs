//! Who the center is to the installer and the updater of `ravenrs-app-update`, the same that
//! install and update Raven Finanças and the other tools: its name, folders and addresses.

use ravenrs_app_update::{InstalledApp, Updater};

/// Raven Centro de Recursos, as installed by compiling the main branch.
pub static APP: InstalledApp = InstalledApp {
    name: "Raven Centro de Recursos",
    package: "feature-center",
    home_dir: ".raven-feature-center",
    bundle_id: "dev.ravenrs.feature-center",
    icons: "tools/feature-center/assets",
    mac_category: "public.app-category.developer-tools",
    linux_categories: "Development;",
    source: "https://github.com/alexkads/RavenRS/archive/refs/heads/master.tar.gz",
    env_prefix: "RFC",
    installed_env: "RAVEN_FEATURE_CENTER_INSTALLED",
    news_urls: &[
        "https://raw.githubusercontent.com/alexkads/RavenRS/master/tools/feature-center/novidades.json",
    ],
    history_urls: &[
        "https://raw.githubusercontent.com/alexkads/RavenRS/master/tools/feature-center/historico-de-novidades.json",
    ],
    installer_url: "https://raw.githubusercontent.com/alexkads/RavenRS/master/tools/feature-center/install.cmd",
    issues_url: "https://github.com/alexkads/RavenRS/issues",
    version: env!("CARGO_PKG_VERSION"),
    // The installer sets it when it compiles.
    installed: option_env!("RAVEN_FEATURE_CENTER_INSTALLED").is_some(),
    history: include_str!("../historico-de-novidades.json"),
};

/// The center's version, the one its news speak of.
pub const VERSION: &str = APP.version;

/// An updater that does nothing until asked.
pub fn idle() -> Updater {
    Updater::idle(&APP)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::APP;

    #[test]
    fn the_three_release_files_agree() {
        ravenrs_app_update::assert_release_files_agree(Path::new(env!("CARGO_MANIFEST_DIR")));
    }

    #[test]
    fn the_installer_is_the_one_the_template_makes() {
        let file = Path::new(env!("CARGO_MANIFEST_DIR")).join("install.cmd");
        ravenrs_app_update::assert_installer_is_current(&APP, &file);
    }
}
