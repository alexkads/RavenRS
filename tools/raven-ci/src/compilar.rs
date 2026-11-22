//! `raven-ci plugins compilar PASTA`: builds a plugin written in Rust with the standard traits and
//! installs it as an external plugin, with no new `raven-ci`.
//!
//! The crate in `PASTA` depends on `raven-ci-plugin-api` with the feature `serve`, and its `main` is
//! `raven_ci_plugin_api::serve(plugin)`. The command:
//!
//! 1. refuses, before building, a crate that asks for a `raven-ci-plugin-api` this server does not
//!    accept, saying the version asked and the one there is;
//! 2. runs `cargo build --release` in the crate (`cargo build`, the profile dev, with `--debug`),
//!    with a `CARGO_TARGET_DIR` of its own, removed at the end whatever happened; with
//!    `RAVEN_CI_CARGO_TARGET` set, that folder instead, kept for the next build (what the tests do,
//!    so the example compiles once). The environment goes to cargo whole, `CARGO_BUILD_JOBS`
//!    included, so the machine decides how many jobs a build takes;
//! 3. asks the binary for its `plugin.toml` (`--manifesto`) and checks it as the core reads it;
//! 4. installs the binary and the `plugin.toml` in `plugins/<nome>/` of the folder of data, where the
//!    core finds it the next time it starts or lists plugins.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use raven_ci_core::PLUGINS_DIR;
use raven_ci_core::external::{ExternalPlugin, MANIFEST_FILE};
use raven_ci_plugin_api::CiError;
use raven_ci_plugin_api::protocol::{API_VERSION, PluginToml, api_accepts};
use ravenrs_process::{End, Event, Spec};
use serde_json::Value;

/// The package every plugin compiled by the system depends on.
const API_PACKAGE: &str = "raven-ci-plugin-api";

/// The folder of the build kept between builds, when set; otherwise each build has its own.
const TARGET_VARIABLE: &str = "RAVEN_CI_CARGO_TARGET";

/// The longest a build of a plugin may take.
const BUILD_LIMIT: Duration = Duration::from_secs(60 * 60);

/// Builds the crate in `folder` (in release, or in the profile dev when `debug`) and installs it in
/// the folder of data `data`, writing what it does to `out`; 0 when installed, 1 when the build
/// failed, 2 when it was asked wrong.
pub fn run(folder: &Path, data: &Path, debug: bool, out: &mut dyn Write) -> Result<i32, CiError> {
    let say = |out: &mut dyn Write, text: &str| {
        out.write_all(text.as_bytes()).map_err(|e| CiError::failed(e.to_string()))
    };
    // cargo runs in the folder: every path it gets is whole.
    let folder = &folder.canonicalize().unwrap_or_else(|_| folder.to_path_buf());
    let manifest_path = folder.join("Cargo.toml");
    let Ok(cargo_toml) = std::fs::read_to_string(&manifest_path) else {
        say(
            out,
            &format!("não há Cargo.toml em {}: um plugin compilado é um crate de Rust\n", folder.display()),
        )?;
        return Ok(2);
    };
    if let Err(why) = check_api(&cargo_toml) {
        say(out, &format!("{why}\n"))?;
        return Ok(2);
    }
    if ravenrs_process::resolve("cargo").is_none() {
        say(out, "não há cargo nesta máquina: instale o Rust (rustup.rs) para compilar plugins\n")?;
        return Ok(1);
    }
    if let Some(kept) = std::env::var_os(TARGET_VARIABLE).filter(|dir| !dir.is_empty()) {
        return build_and_install(folder, &manifest_path, Path::new(&kept), debug, data, out);
    }
    let target = data.join(".compilando").join(format!("{}-{}", std::process::id(), next()));
    let result = build_and_install(folder, &manifest_path, &target, debug, data, out);
    let _ = std::fs::remove_dir_all(&target);
    if let Some(parent) = target.parent() {
        let _ = std::fs::remove_dir(parent);
    }
    result
}

fn next() -> u64 {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    NEXT.fetch_add(1, Ordering::SeqCst)
}

/// Refuses the crate whose `Cargo.toml` asks for a `raven-ci-plugin-api` this server does not
/// accept; one that does not depend on it at all is refused too, since it could not be served.
fn check_api(cargo_toml: &str) -> Result<(), String> {
    let manifest: toml::Table = toml::from_str(cargo_toml).map_err(|e| format!("o Cargo.toml: {e}"))?;
    let dependency =
        manifest.get("dependencies").and_then(|deps| deps.get(API_PACKAGE)).ok_or_else(|| {
            format!("o crate não depende de {API_PACKAGE}, com a feature serve: não é um plugin")
        })?;
    let wanted = match dependency {
        toml::Value::String(version) => Some(version.clone()),
        toml::Value::Table(table) => table.get("version").and_then(toml::Value::as_str).map(str::to_owned),
        _ => None,
    };
    match wanted {
        Some(wanted) => api_accepts(&wanted),
        // Only a path: what is at the path decides, and the binary says it in its plugin.toml.
        None => Ok(()),
    }
}

/// Builds with `target` as the folder of the build, asks the binary for its `plugin.toml` and
/// installs both.
fn build_and_install(
    folder: &Path,
    manifest_path: &Path,
    target: &Path,
    debug: bool,
    data: &Path,
    out: &mut dyn Write,
) -> Result<i32, CiError> {
    let say = |out: &mut dyn Write, text: &str| {
        out.write_all(text.as_bytes()).map_err(|e| CiError::failed(e.to_string()))
    };
    let profile: &[&str] = if debug { &[] } else { &["--release"] };
    let how = if debug { "cargo build, perfil dev" } else { "cargo build --release" };
    say(out, &format!("compilando {} ({how})…\n", folder.display()))?;
    let running = Spec::new("cargo")
        .arg("build")
        .args(profile)
        .args(["--message-format=json-render-diagnostics", "--manifest-path"])
        .arg(manifest_path)
        .dir(folder)
        .inherit_env()
        .env("CARGO_TARGET_DIR", target.display().to_string())
        .timeout(BUILD_LIMIT)
        .spawn()
        .map_err(|e| CiError::failed(format!("o cargo não subiu: {e}")))?;
    let wanted = manifest_path.canonicalize().unwrap_or_else(|_| manifest_path.to_path_buf());
    let mut executable: Option<PathBuf> = None;
    let mut errors: Vec<String> = Vec::new();
    let mut end = End::Code(-1);
    for event in running.events() {
        match event {
            Event::Out(line) => {
                let Ok(message) = serde_json::from_str::<Value>(&line) else { continue };
                if message.get("reason").and_then(Value::as_str) != Some("compiler-artifact") {
                    continue;
                }
                let from = message.get("manifest_path").and_then(Value::as_str).map(PathBuf::from);
                let ours = from.is_some_and(|from| from.canonicalize().unwrap_or(from) == wanted);
                if let Some(path) = message.get("executable").and_then(Value::as_str).filter(|_| ours) {
                    executable = Some(PathBuf::from(path));
                }
            }
            Event::Err(line) => errors.push(line),
            Event::Ended(how) => end = how,
        }
    }
    if !end.success() {
        let tail = errors.len().saturating_sub(40);
        say(out, &format!("a compilação falhou ({end:?}):\n{}\n", errors[tail..].join("\n")))?;
        return Ok(1);
    }
    let Some(executable) = executable else {
        say(
            out,
            "a compilação passou, mas o crate não tem um binário: o main de um plugin é serve(plugin)\n",
        )?;
        return Ok(1);
    };
    let said = Spec::new(&executable)
        .arg("--manifesto")
        .timeout(Duration::from_secs(60))
        .output()
        .map_err(|e| CiError::failed(format!("{}: {e}", executable.display())))?;
    if !said.end.success() {
        say(out, &format!("o plugin não escreveu o plugin.toml: {}\n", said.stderr.join("\n")))?;
        return Ok(1);
    }
    let text = said.stdout.join("\n") + "\n";
    let file: PluginToml = match toml::from_str(&text) {
        Ok(file) => file,
        Err(e) => {
            say(out, &format!("o plugin.toml que o plugin escreveu não se lê: {e}\n"))?;
            return Ok(1);
        }
    };
    let problems = file.check();
    if !problems.is_empty() {
        say(out, &format!("o plugin.toml de {}: {}\n", file.name, problems.join("; ")))?;
        return Ok(2);
    }
    let home = data.join(PLUGINS_DIR).join(&file.name);
    install(&executable, &text, &home).map_err(CiError::failed)?;
    let plugin = ExternalPlugin::open(&home).map_err(CiError::failed)?;
    let points: Vec<&str> = plugin.declared_points().iter().map(|p| p.name()).collect();
    say(
        out,
        &format!(
            "{} {}: compilado com a raven-ci-plugin-api {} e instalado em {}\n  pontos: {}\n",
            file.name,
            file.version,
            file.api.as_deref().unwrap_or(API_VERSION),
            home.display(),
            points.join(", ")
        ),
    )?;
    Ok(0)
}

/// Puts the binary and its `plugin.toml` in `home`, each written beside and renamed over the old
/// one, so a server that lists plugins in the meantime never reads half a file.
fn install(executable: &Path, plugin_toml: &str, home: &Path) -> Result<(), String> {
    std::fs::create_dir_all(home).map_err(|e| format!("{}: {e}", home.display()))?;
    let name = executable.file_name().ok_or("o binário não tem nome")?;
    let binary = home.join(name);
    let fresh = home.join(format!(".{}.novo", name.to_string_lossy()));
    std::fs::copy(executable, &fresh).map_err(|e| format!("{}: {e}", fresh.display()))?;
    std::fs::rename(&fresh, &binary).map_err(|e| format!("{}: {e}", binary.display()))?;
    let manifest = home.join(MANIFEST_FILE);
    let fresh = home.join(format!(".{MANIFEST_FILE}.novo"));
    std::fs::write(&fresh, plugin_toml).map_err(|e| format!("{}: {e}", fresh.display()))?;
    std::fs::rename(&fresh, &manifest).map_err(|e| format!("{}: {e}", manifest.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_crate_that_asks_for_another_api_is_refused_with_both_versions() {
        let two = "[dependencies]\nraven-ci-plugin-api = { path = \"x\", version = \"2\", features = [\"serve\"] }\n";
        let said = check_api(two).expect_err("refused");
        assert!(said.contains("pede a raven-ci-plugin-api 2"), "{said}");
        assert!(said.contains(&format!("este Raven CI tem a {API_VERSION}")), "{said}");
        assert!(check_api("[dependencies]\nraven-ci-plugin-api = \"0.1\"\n").is_ok());
        assert!(check_api("[dependencies]\nraven-ci-plugin-api = { path = \"x\" }\n").is_ok());
        assert!(
            check_api("[dependencies]\nserde = \"1\"\n").expect_err("no api").contains("não é um plugin")
        );
    }
}
