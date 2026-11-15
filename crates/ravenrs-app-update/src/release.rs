//! Releasing is one commit with three files that must agree.

use std::path::Path;

use crate::news::{News, parse_history, parse_news};

/// The three release files of the app in `dir` say the same version:
///
/// - `version` under `[package]` in `Cargo.toml`, the app's own (not the workspace's);
/// - `novidades.json`, which names who made it in `contribuidores` and the issues
///   it closes in `issues`;
/// - the first entry of `historico-de-novidades.json`, the same one with its `data`.
///
/// Returns what disagrees, in words that say what to fix.
pub fn check_release_files(dir: &Path) -> Result<(), String> {
    let read = |name: &str| std::fs::read_to_string(dir.join(name)).map_err(|e| format!("{name}: {e}"));
    let version = package_version(&read("Cargo.toml")?)
        .ok_or("Cargo.toml: the app has its own `version = \"…\"` under [package]")?;
    let news = parse_news(&read("novidades.json")?)
        .ok_or("novidades.json: a version made of numbers, a titulo, novidades and por_que_atualizar")?;
    if news.versao != version {
        return Err(format!(
            "novidades.json says {}, Cargo.toml says {version}: raise novidades.json together with Cargo.toml",
            news.versao
        ));
    }
    if news.contribuidores.is_empty() || news.contribuidores.iter().any(|name| name.trim().is_empty()) {
        return Err("novidades.json names who made the version in contribuidores".to_owned());
    }
    if news.issues.is_empty() {
        return Err("novidades.json lists the issues this version closes in issues".to_owned());
    }
    let history = parse_history(&read("historico-de-novidades.json")?);
    let top = history.first().ok_or("historico-de-novidades.json: empty history")?;
    if top.data.is_none() {
        return Err("historico-de-novidades.json: the history records the day, in data".to_owned());
    }
    if (News { data: None, ..top.clone() }) != news {
        return Err(format!(
            "historico-de-novidades.json starts with {}, not with the entry of novidades.json: the history starts with this version's entry",
            top.versao
        ));
    }
    Ok(())
}

/// [`check_release_files`], for an app's tests:
///
/// ```no_run
/// #[test]
/// fn the_three_release_files_agree() {
///     ravenrs_app_update::assert_release_files_agree(std::path::Path::new(env!("CARGO_MANIFEST_DIR")));
/// }
/// ```
///
/// # Panics
///
/// When the three files disagree, or one is missing.
#[track_caller]
pub fn assert_release_files_agree(dir: &Path) {
    if let Err(problem) = check_release_files(dir) {
        panic!("{problem}");
    }
}

/// The `version` under `[package]`, written as a string.
fn package_version(manifest: &str) -> Option<String> {
    let mut in_package = false;
    for line in manifest.lines().map(str::trim) {
        if line.starts_with('[') {
            in_package = line == "[package]";
            continue;
        }
        let Some(value) = line.strip_prefix("version").map(str::trim_start) else { continue };
        if in_package && let Some(value) = value.strip_prefix('=') {
            let value = value.trim();
            return value.strip_prefix('"').and_then(|rest| rest.split('"').next()).map(str::to_owned);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::package_version;

    #[test]
    fn the_version_is_the_package_s() {
        let manifest = "[package]\nname = \"a\"\nversion = \"0.1.38\"\n[dependencies]\nversion = \"9\"\n";
        assert_eq!(package_version(manifest).as_deref(), Some("0.1.38"));
        assert_eq!(package_version("[package]\nversion.workspace = true\n"), None);
        assert_eq!(package_version("[dependencies]\nversion = \"1\"\n"), None);
    }
}
