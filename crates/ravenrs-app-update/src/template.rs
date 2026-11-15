//! The installer of every app, made from one template.
//!
//! Each app keeps the `install.cmd` made for it, whole, because that file is
//! what people double-click and what installed copies download: a copy only
//! knows the address it was compiled with, and `cmd` cannot include another
//! file. A test in the app compares it with [`InstalledApp::render_installer`],
//! so it never drifts from the template.

use std::path::Path;

use crate::InstalledApp;

/// The installer with `@NAME@` marks where each app's values go.
pub const INSTALLER_TEMPLATE: &str = include_str!("../install.cmd.in");

/// Set to `1`, [`assert_installer_is_current`] writes the app's installer
/// instead of failing when it differs.
pub const WRITE_INSTALLER_ENV: &str = "RAVENRS_WRITE_INSTALLER";

impl InstalledApp {
    /// This app's installer: the template with its values, or the field that
    /// cannot go into a script as it is.
    pub fn render_installer(&self) -> Result<String, String> {
        self.check()?;
        let icons_windows = self.icons.replace('/', "\\");
        let windows_dir = self.windows_dir();
        let ascii_name = self.ascii_name();
        let values: [(&str, &str); 13] = [
            ("@NOME@", self.name),
            ("@NOME_ASCII@", &ascii_name),
            ("@PACOTE@", self.package),
            ("@CASA@", self.home_dir),
            ("@PASTA_WINDOWS@", &windows_dir),
            ("@BUNDLE_ID@", self.bundle_id),
            ("@ICONES@", self.icons),
            ("@ICONES_WINDOWS@", &icons_windows),
            ("@CATEGORIA_MAC@", self.mac_category),
            ("@CATEGORIAS_LINUX@", self.linux_categories),
            ("@FONTE@", self.source),
            ("@PREFIXO@", self.env_prefix),
            ("@INSTALADO@", self.installed_env),
        ];
        let mut text = INSTALLER_TEMPLATE.to_owned();
        for (mark, value) in values {
            text = text.replace(mark, value);
        }
        Ok(text)
    }

    /// Every field goes into `cmd`, PowerShell, `sh`, a plist and a `.desktop`
    /// file. A quote, a `$` or a `%` would change what the installer does, so
    /// each field takes only what it needs.
    pub fn check(&self) -> Result<(), String> {
        let fields = [
            ("name", self.name, Allowed::Name),
            ("package", self.package, Allowed::Package),
            ("home_dir", self.home_dir, Allowed::Path),
            ("bundle_id", self.bundle_id, Allowed::Path),
            ("icons", self.icons, Allowed::Path),
            ("mac_category", self.mac_category, Allowed::Path),
            ("linux_categories", self.linux_categories, Allowed::Categories),
            ("source", self.source, Allowed::Url),
            ("env_prefix", self.env_prefix, Allowed::Variable),
            ("installed_env", self.installed_env, Allowed::Variable),
            ("installer_url", self.installer_url, Allowed::Url),
        ];
        for (field, value, allowed) in fields {
            if value.is_empty() || !value.chars().all(|c| allowed.takes(c)) {
                return Err(format!("{field} = {value:?}: {}", allowed.says()));
            }
        }
        if !self.home_dir.starts_with('.') || self.home_dir.contains('/') {
            return Err(format!("home_dir = {:?}: one hidden folder, starting with a dot", self.home_dir));
        }
        if !self.linux_categories.ends_with(';') {
            return Err(format!("linux_categories = {:?}: each category ends in ;", self.linux_categories));
        }
        if self.windows_dir().is_empty() {
            return Err(format!("name = {:?}: needs a letter Windows can show", self.name));
        }
        Ok(())
    }
}

#[derive(Clone, Copy)]
enum Allowed {
    Name,
    Package,
    Path,
    Categories,
    Url,
    Variable,
}

impl Allowed {
    fn takes(self, c: char) -> bool {
        match self {
            Self::Name => c.is_alphanumeric() || c == ' ' || c == '-',
            Self::Package => c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-',
            Self::Path => c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | '/'),
            Self::Categories => c.is_ascii_alphanumeric() || c == ';',
            Self::Url => c.is_ascii_alphanumeric() || matches!(c, ':' | '/' | '.' | '-' | '_' | '~'),
            Self::Variable => c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_',
        }
    }

    fn says(self) -> &'static str {
        match self {
            Self::Name => "letters, digits, spaces and hyphens only",
            Self::Package => "lowercase letters, digits and hyphens only, like the Cargo package",
            Self::Path => "ASCII letters, digits, '.', '-', '_' and '/' only",
            Self::Categories => "ASCII letters and ';' only",
            Self::Url => "an https address without spaces, quotes, '%', '?' or '&'",
            Self::Variable => "uppercase letters, digits and '_' only",
        }
    }
}

/// The app's committed installer is the one the template makes for it; if not,
/// says how to write it again. With [`WRITE_INSTALLER_ENV`] set to `1`, writes it.
pub fn check_installer(app: &InstalledApp, file: &Path) -> Result<(), String> {
    let made = app.render_installer()?;
    let committed = std::fs::read_to_string(file).unwrap_or_default();
    if committed == made {
        return Ok(());
    }
    if std::env::var(WRITE_INSTALLER_ENV).is_ok_and(|value| value == "1") {
        return std::fs::write(file, made).map_err(|e| format!("{}: {e}", file.display()));
    }
    Err(format!(
        "{} is not what crates/ravenrs-app-update/install.cmd.in makes for {}: change the template, \
         never the copy, and run the tests again with {WRITE_INSTALLER_ENV}=1 to write it",
        file.display(),
        app.name
    ))
}

/// [`check_installer`], for an app's tests.
///
/// # Panics
///
/// When the installer differs from what the template makes, or a field cannot go into it.
#[track_caller]
pub fn assert_installer_is_current(app: &InstalledApp, file: &Path) {
    if let Err(problem) = check_installer(app, file) {
        panic!("{problem}");
    }
}
