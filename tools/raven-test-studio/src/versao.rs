//! Who the Studio is to the installer and the updater of `ravenrs-app-update`, the same that
//! install and update Raven Finanças and Raven Flows: its name, folders and addresses.

use ravenrs_app_update::{InstalledApp, Updater};

/// Raven Test Studio, as installed by compiling the main branch.
pub static APP: InstalledApp = InstalledApp {
    name: "Raven Test Studio",
    package: "raven-test-studio",
    home_dir: ".raven-test-studio",
    bundle_id: "dev.ravenrs.raven-test-studio",
    icons: "tools/raven-test-studio/assets",
    mac_category: "public.app-category.developer-tools",
    linux_categories: "Development;",
    source: "https://github.com/alexkads/RavenRS/archive/refs/heads/master.tar.gz",
    env_prefix: "RTS",
    installed_env: "RAVEN_TEST_STUDIO_INSTALLED",
    news_urls: &[
        "https://raw.githubusercontent.com/alexkads/RavenRS/master/tools/raven-test-studio/novidades.json",
    ],
    history_urls: &[
        "https://raw.githubusercontent.com/alexkads/RavenRS/master/tools/raven-test-studio/historico-de-novidades.json",
    ],
    installer_url: "https://raw.githubusercontent.com/alexkads/RavenRS/master/tools/raven-test-studio/install.cmd",
    issues_url: "https://github.com/alexkads/RavenRS/issues",
    version: env!("CARGO_PKG_VERSION"),
    // The installer sets it when it compiles.
    installed: option_env!("RAVEN_TEST_STUDIO_INSTALLED").is_some(),
    history: include_str!("../historico-de-novidades.json"),
};

/// The Studio's version, the one its news speak of.
pub const VERSION: &str = APP.version;

/// An updater that does nothing until asked: the window calls `watch` on it once it opens.
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
