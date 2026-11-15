//! Who the app is to the installer and the updater of `ravenrs-app-update`, the same that
//! install and update Raven Finanças: its name, folders and addresses.

use ravenrs_app_update::{InstalledApp, Updater};

/// Raven Flows, as installed by compiling the main branch.
pub static APP: InstalledApp = InstalledApp {
    name: "Raven Flows",
    package: "raven-flows",
    home_dir: ".raven-flows",
    bundle_id: "dev.ravenrs.raven-flows",
    icons: "examples/raven-flows/assets",
    mac_category: "public.app-category.productivity",
    linux_categories: "Office;Development;",
    source: "https://github.com/alexkads/RavenRS/archive/refs/heads/master.tar.gz",
    env_prefix: "RF",
    installed_env: "RAVEN_FLOWS_INSTALLED",
    news_urls: &[
        "https://raw.githubusercontent.com/alexkads/RavenRS/master/examples/raven-flows/novidades.json",
    ],
    history_urls: &[
        "https://raw.githubusercontent.com/alexkads/RavenRS/master/examples/raven-flows/historico-de-novidades.json",
    ],
    installer_url: "https://raw.githubusercontent.com/alexkads/RavenRS/master/examples/raven-flows/install.cmd",
    issues_url: "https://github.com/alexkads/RavenRS/issues",
    version: env!("CARGO_PKG_VERSION"),
    // The installer sets it when it compiles.
    installed: option_env!("RAVEN_FLOWS_INSTALLED").is_some(),
    history: include_str!("../historico-de-novidades.json"),
};

/// An updater that does nothing until asked: the app calls `watch` on it once it opens.
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
