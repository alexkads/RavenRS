//! Who the book is to the installer and the updater of `ravenrs-app-update`, the same that
//! install and update Raven Finanças and Raven Flows: its name, folders and addresses.

use ravenrs_app_update::{InstalledApp, Updater};

/// RavenComponentBook, as installed by compiling the main branch.
pub static APP: InstalledApp = InstalledApp {
    name: "RavenComponentBook",
    package: "raven-component-book",
    home_dir: ".raven-component-book",
    bundle_id: "dev.ravenrs.raven-component-book",
    icons: "tools/raven-component-book/assets",
    mac_category: "public.app-category.developer-tools",
    linux_categories: "Development;",
    source: "https://github.com/alexkads/RavenRS/archive/refs/heads/master.tar.gz",
    env_prefix: "RCB",
    installed_env: "RAVEN_COMPONENT_BOOK_INSTALLED",
    news_urls: &[
        "https://raw.githubusercontent.com/alexkads/RavenRS/master/tools/raven-component-book/novidades.json",
    ],
    history_urls: &[
        "https://raw.githubusercontent.com/alexkads/RavenRS/master/tools/raven-component-book/historico-de-novidades.json",
    ],
    installer_url: "https://raw.githubusercontent.com/alexkads/RavenRS/master/tools/raven-component-book/install.cmd",
    issues_url: "https://github.com/alexkads/RavenRS/issues",
    version: env!("CARGO_PKG_VERSION"),
    // The installer sets it when it compiles.
    installed: option_env!("RAVEN_COMPONENT_BOOK_INSTALLED").is_some(),
    history: include_str!("../historico-de-novidades.json"),
};

/// The book's version, the one its news speak of.
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
