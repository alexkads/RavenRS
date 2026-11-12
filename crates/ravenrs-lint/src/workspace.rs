//! Which packages to check and where their sources are, from `cargo metadata`.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

use crate::{Config, Error};

/// One package of the workspace.
#[derive(Clone, Debug)]
pub struct Package {
    /// Its name.
    pub name: String,
    /// Its `Cargo.toml`.
    pub manifest: PathBuf,
    /// The directory of its `Cargo.toml`; diagnostics are relative to it.
    pub root: PathBuf,
    /// The directories of its library and binaries. Tests, examples and benches are left out.
    pub source_dirs: Vec<PathBuf>,
    /// Whether it depends on a RavenRS crate.
    pub uses_ravenrs: bool,
    /// Its configuration, the workspace's merged under its own.
    pub config: Config,
}

/// The packages of the workspace that `manifest` belongs to.
pub fn packages(manifest: &Path) -> Result<Vec<Package>, Error> {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let output = Command::new(cargo)
        .args(["metadata", "--no-deps", "--format-version", "1", "--manifest-path"])
        .arg(manifest)
        .output()
        .map_err(|error| Error::Cargo(error.to_string()))?;
    if !output.status.success() {
        return Err(Error::Cargo(String::from_utf8_lossy(&output.stderr).trim().to_owned()));
    }
    let metadata: Value =
        serde_json::from_slice(&output.stdout).map_err(|error| Error::Cargo(error.to_string()))?;
    let workspace = metadata.pointer("/metadata/ravenrs-lint");
    let Some(packages) = metadata.get("packages").and_then(Value::as_array) else {
        return Err(Error::Cargo("no packages in the answer".into()));
    };
    packages.iter().map(|package| read_package(package, workspace)).collect()
}

fn read_package(package: &Value, workspace: Option<&Value>) -> Result<Package, Error> {
    let text = |key: &str| package.get(key).and_then(Value::as_str).unwrap_or_default().to_owned();
    let name = text("name");
    let manifest = PathBuf::from(text("manifest_path"));
    let root = manifest.parent().map(Path::to_path_buf).unwrap_or_default();
    let config = Config::from_metadata(workspace, package.pointer("/metadata/ravenrs-lint"))
        .map_err(|error| Error::Config(format!("{name}: {error}")))?;
    let uses_ravenrs = package.get("dependencies").and_then(Value::as_array).is_some_and(|deps| {
        deps.iter()
            .filter_map(|d| d.get("name").and_then(Value::as_str))
            .any(|d| (d == "ravenrs" || d.starts_with("ravenrs-")) && d != "ravenrs-lint")
    });
    let mut dirs: Vec<PathBuf> = package
        .get("targets")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|target| {
            target.get("kind").and_then(Value::as_array).is_some_and(|kinds| {
                kinds.iter().filter_map(Value::as_str).any(|kind| {
                    matches!(kind, "lib" | "rlib" | "dylib" | "cdylib" | "staticlib" | "bin" | "proc-macro")
                })
            })
        })
        .filter_map(|target| target.get("src_path").and_then(Value::as_str))
        .filter_map(|path| Path::new(path).parent().map(Path::to_path_buf))
        .collect();
    dirs.sort();
    dirs.dedup();
    // `src/bin` is already inside `src`.
    let source_dirs = dirs
        .iter()
        .filter(|dir| !dirs.iter().any(|other| other != *dir && dir.starts_with(other)))
        .cloned()
        .collect();
    Ok(Package { name, manifest, root, source_dirs, uses_ravenrs, config })
}
