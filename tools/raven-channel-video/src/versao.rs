//! Who Raven Channel Video is to the installer and the updater of `ravenrs-app-update`, the same that
//! install and update Raven Finanças and the other tools: its name, folders and addresses.

use ravenrs_app_update::InstalledApp;

/// Raven Channel Video, as installed by compiling the main branch.
pub static APP: InstalledApp = InstalledApp {
    name: "Raven Channel Video",
    package: "raven-channel-video",
    home_dir: ".raven-channel-video",
    bundle_id: "dev.ravenrs.raven-channel-video",
    icons: "tools/raven-channel-video/assets",
    mac_category: "public.app-category.developer-tools",
    linux_categories: "Development;",
    source: "https://github.com/alexkads/RavenRS/archive/refs/heads/master.tar.gz",
    env_prefix: "RCV",
    installed_env: "RAVEN_CHANNEL_VIDEO_INSTALLED",
    news_urls: &[
        "https://raw.githubusercontent.com/alexkads/RavenRS/master/tools/raven-channel-video/novidades.json",
    ],
    history_urls: &[
        "https://raw.githubusercontent.com/alexkads/RavenRS/master/tools/raven-channel-video/historico-de-novidades.json",
    ],
    installer_url: "https://raw.githubusercontent.com/alexkads/RavenRS/master/tools/raven-channel-video/install.cmd",
    issues_url: "https://github.com/alexkads/RavenRS/issues",
    version: env!("CARGO_PKG_VERSION"),
    // The installer sets it when it compiles.
    installed: option_env!("RAVEN_CHANNEL_VIDEO_INSTALLED").is_some(),
    history: include_str!("../historico-de-novidades.json"),
};

/// The version, the one its news speak of.
pub const VERSION: &str = APP.version;

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
