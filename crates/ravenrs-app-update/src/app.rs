//! Who the app is: its name, its folders, its addresses, and what its build knows.

use std::path::PathBuf;

use crate::news::{News, parse_history};

/// An app that installs and updates by compiling on the user's machine.
///
/// Everything about one app is here, and nothing about it is anywhere else in
/// this crate: the installer, the folders and the addresses all come from these
/// fields. An app keeps one in a `static`:
///
/// ```
/// use ravenrs_app_update::InstalledApp;
///
/// pub static APP: InstalledApp = InstalledApp {
///     name: "Raven Exemplo",
///     package: "raven-exemplo",
///     home_dir: ".raven-exemplo",
///     bundle_id: "dev.ravenrs.raven-exemplo",
///     icons: "examples/raven-exemplo/assets",
///     mac_category: "public.app-category.productivity",
///     linux_categories: "Office;",
///     source: "https://github.com/alexkads/RavenRS/archive/refs/heads/master.tar.gz",
///     env_prefix: "RE",
///     installed_env: "RAVEN_EXEMPLO_INSTALLED",
///     news_urls: &["https://raw.githubusercontent.com/alexkads/RavenRS/master/examples/raven-exemplo/novidades.json"],
///     history_urls: &[
///         "https://raw.githubusercontent.com/alexkads/RavenRS/master/examples/raven-exemplo/historico-de-novidades.json",
///     ],
///     installer_url: "https://raw.githubusercontent.com/alexkads/RavenRS/master/examples/raven-exemplo/install.cmd",
///     issues_url: "https://github.com/alexkads/RavenRS/issues",
///     version: env!("CARGO_PKG_VERSION"),
///     installed: option_env!("RAVEN_EXEMPLO_INSTALLED").is_some(),
///     history: "[]",
/// };
///
/// assert!(APP.home().ends_with(".raven-exemplo"));
/// assert!(APP.render_installer().is_ok());
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InstalledApp {
    /// The name people see: in the menus, the shortcut and the macOS bundle.
    /// Accents are fine; the Windows parts use [`Self::ascii_name`].
    pub name: &'static str,
    /// The Cargo package, which is also the binary's name and, on Linux, the
    /// `.desktop` file, the icon and the window's id: the window has to be opened with this id.
    pub package: &'static str,
    /// The folder in the user's home where the installer keeps the code, the
    /// build and its notes, and where the app may keep its own files. It starts with a dot.
    pub home_dir: &'static str,
    /// The macOS bundle identifier.
    pub bundle_id: &'static str,
    /// The folder, inside the repository, with `icon.ico`, `icon.icns` and `icon.png`.
    pub icons: &'static str,
    /// The macOS category (`LSApplicationCategoryType`).
    pub mac_category: &'static str,
    /// The Linux menu categories, each one ending in `;`.
    pub linux_categories: &'static str,
    /// The `.tar.gz` of the code the installer compiles.
    pub source: &'static str,
    /// What the installer's options start with: `RF` gives `RF_CASA`, `RF_DESTINO` and the rest.
    pub env_prefix: &'static str,
    /// The variable the installer sets while it compiles, which [`Self::installed`] reads.
    pub installed_env: &'static str,
    /// Where `novidades.json` is read, in order. An installed copy only knows the
    /// addresses it was compiled with, so addresses are only ever added.
    pub news_urls: &'static [&'static str],
    /// Where `historico-de-novidades.json` is read, in order, for skipping more than one version.
    pub history_urls: &'static [&'static str],
    /// The installer the app downloads and runs, the same one its README shows.
    pub installer_url: &'static str,
    /// Where the issues are read, without the number.
    pub issues_url: &'static str,
    /// The running version: `env!("CARGO_PKG_VERSION")`.
    pub version: &'static str,
    /// The installer compiled this copy: `option_env!(<installed_env>).is_some()`.
    pub installed: bool,
    /// The history compiled into the app: `include_str!("../historico-de-novidades.json")`.
    pub history: &'static str,
}

/// How this copy of the app was installed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InstallKind {
    /// By the installer, which is the only thing that can update it.
    Installed,
    /// `cargo run`: whoever develops updates with `git pull`.
    Development,
}

impl InstalledApp {
    /// The name without accents, for the Windows shortcut and the `cmd` window,
    /// where the console's code page would garble them.
    pub fn ascii_name(&self) -> String {
        self.name.chars().filter_map(ascii_letter).collect()
    }

    /// The folder under `%LOCALAPPDATA%\Programs` where Windows keeps the app: the ASCII name, without spaces.
    pub fn windows_dir(&self) -> String {
        self.ascii_name().split_whitespace().collect()
    }

    /// The app's folder in the repository, where its `Cargo.toml` is: the one that holds its
    /// icons (`examples/family-finance` for `examples/family-finance/assets`). The installer builds
    /// through that manifest, as the app may be a workspace of its own (#1084).
    pub(crate) fn source_folder(&self) -> &str {
        self.icons
            .strip_suffix("/assets")
            .or_else(|| self.icons.rsplit_once('/').map(|(folder, _)| folder))
            .unwrap_or(".")
    }

    /// How this copy was installed.
    pub fn install_kind(&self) -> InstallKind {
        if self.installed { InstallKind::Installed } else { InstallKind::Development }
    }

    /// `~/<home_dir>`: what the installer leaves, and where the update keeps its notes.
    pub fn home(&self) -> PathBuf {
        user_home().unwrap_or_else(|| PathBuf::from(".")).join(self.home_dir)
    }

    /// Where the installer puts the app; the update checks this one and reopens it.
    pub fn installed_binary(&self) -> Option<PathBuf> {
        if cfg!(windows) {
            std::env::var_os("LOCALAPPDATA").map(|base| {
                PathBuf::from(base)
                    .join("Programs")
                    .join(self.windows_dir())
                    .join(format!("{}.exe", self.package))
            })
        } else if cfg!(target_os = "macos") {
            // The installer uses /Applications when it can write there.
            let inside = format!("{}.app/Contents/MacOS/{}", self.name, self.package);
            let system = PathBuf::from("/Applications").join(&inside);
            if system.exists() {
                Some(system)
            } else {
                user_home().map(|h| h.join("Applications").join(inside))
            }
        } else {
            user_home().map(|h| h.join(".local/bin").join(self.package))
        }
    }

    /// Where the installer keeps the version it replaced: on macOS in
    /// `~/<home_dir>/anterior`, where it does not show as a second app; elsewhere
    /// beside the installed one, as `.anterior`.
    pub fn previous_version(&self) -> Option<PathBuf> {
        if cfg!(target_os = "macos") {
            Some(self.home().join("anterior").join(format!("{}.app", self.name)))
        } else {
            self.installed_binary().map(|binary| {
                let mut name = binary.into_os_string();
                name.push(".anterior");
                PathBuf::from(name)
            })
        }
    }

    /// Running the installer by hand on this system, for when updating from inside the app fails.
    pub fn install_command(&self) -> String {
        self.install_command_on(cfg!(windows))
    }

    /// Running the installer by hand, in PowerShell on Windows or in a terminal elsewhere.
    pub fn install_command_on(&self, windows: bool) -> String {
        let url = self.installer_url;
        if windows {
            format!(r#"irm {url} -OutFile "$env:TEMP\install.cmd"; & "$env:TEMP\install.cmd""#)
        } else {
            format!("curl -fsSL {url} -o /tmp/install.cmd && sh /tmp/install.cmd")
        }
    }

    /// Where an issue is read, by its number.
    pub fn issue_url(&self, number: u32) -> String {
        format!("{}/{number}", self.issues_url.trim_end_matches('/'))
    }

    /// Every version up to this one, newest first, as compiled into the app.
    pub fn history(&self) -> Vec<News> {
        parse_history(self.history)
    }

    /// Where the installer writes what it does.
    pub fn installer_log(&self) -> PathBuf {
        self.home().join(crate::installer::LOG)
    }

    /// The end of the installer's log, for the details of a failure.
    pub fn installer_log_tail(&self, lines: usize) -> Option<String> {
        let text = std::fs::read(self.installer_log())
            .ok()
            .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())?;
        let kept: Vec<&str> = text.lines().rev().take(lines).collect();
        let tail = kept.into_iter().rev().collect::<Vec<_>>().join("\n");
        (!tail.trim().is_empty()).then_some(tail)
    }

    /// The variable that keeps the installer from waiting for a key at the end,
    /// which the app sets when it runs it.
    pub(crate) fn no_pause_env(&self) -> String {
        format!("{}_SEM_PAUSA", self.env_prefix)
    }
}

fn user_home() -> Option<PathBuf> {
    std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(PathBuf::from)
}

/// The letter without its accent; anything else that is not ASCII goes.
fn ascii_letter(c: char) -> Option<char> {
    if c.is_ascii() {
        return Some(c);
    }
    let plain = match c {
        'á' | 'à' | 'â' | 'ã' | 'ä' => 'a',
        'Á' | 'À' | 'Â' | 'Ã' | 'Ä' => 'A',
        'é' | 'è' | 'ê' | 'ë' => 'e',
        'É' | 'È' | 'Ê' | 'Ë' => 'E',
        'í' | 'ì' | 'î' | 'ï' => 'i',
        'Í' | 'Ì' | 'Î' | 'Ï' => 'I',
        'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
        'Ó' | 'Ò' | 'Ô' | 'Õ' | 'Ö' => 'O',
        'ú' | 'ù' | 'û' | 'ü' => 'u',
        'Ú' | 'Ù' | 'Û' | 'Ü' => 'U',
        'ç' => 'c',
        'Ç' => 'C',
        'ñ' => 'n',
        'Ñ' => 'N',
        _ => return None,
    };
    Some(plain)
}
