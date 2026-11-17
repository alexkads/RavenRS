//! RavenRS dependency versions as Cargo resolves them, without changing any manifest.
//!
//! A project may declare `ravenrs = "=27.1.28"`, rename it (`ui = { package = "ravenrs" }`),
//! put it under `[target.'cfg(..)'.dependencies]`, or inherit it with `ravenrs.workspace = true`.
//! For inheritance, the version lives in `[workspace.dependencies]` of the workspace that owns
//! the manifest: the one named by `package.workspace`, or else the nearest ancestor with a
//! `[workspace]` table. A virtual workspace root has no `[package]`; there every member is read.
use crate::read;
use std::path::{Path, PathBuf};
use toml_edit::{DocumentMut, Item, TableLike};

/// One RavenRS dependency with the version Cargo would request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Declared {
    /// Where it was declared, for messages: `<manifest>: <table>.<key>`.
    pub origin: String,
    /// Real package name (after `package = ".."` renames).
    pub package: String,
    /// Requirement as written, such as `=27.1.28` or `^27.1`.
    pub version: String,
}

const KINDS: [&str; 3] = ["dependencies", "dev-dependencies", "build-dependencies"];

/// Refuses a framework version different from the CLI before modifying or diagnosing a project.
pub(crate) fn ensure_current(root: &Path) -> Result<(), String> {
    let versions = ravenrs_versions(root)?;
    if versions.is_empty() {
        return Err("Nenhuma dependência RavenRS com versão no projeto nem no workspace.".into());
    }
    for declared in versions {
        if declared.version.trim_start_matches(['=', '^', '~']) != crate::VERSION {
            return Err(format!(
                "{} ({}) usa {}, mas cargo-ravenrs usa {}. Instale a tag correspondente.",
                declared.package,
                declared.origin,
                declared.version,
                crate::VERSION
            ));
        }
    }
    Ok(())
}

fn manifest(path: &Path) -> Result<DocumentMut, String> {
    read(path)?.parse().map_err(|e| format!("{}: {e}", path.display()))
}

/// Every RavenRS dependency with an explicit or inherited version, for the project at `root`.
pub fn ravenrs_versions(root: &Path) -> Result<Vec<Declared>, String> {
    let path = root.join("Cargo.toml");
    let doc = manifest(&path)?;
    let mut found = Vec::new();
    if doc.get("package").is_some() {
        let workspace = owning_workspace(root, &doc)?;
        package_versions(&path, &doc, workspace.as_ref(), &mut found)?;
    }
    if doc.get("workspace").is_some() {
        let own = Some((path.clone(), doc.clone()));
        workspace_table_versions(&path, &doc, &mut found);
        for member in members(root, &doc)? {
            let member_path = member.join("Cargo.toml");
            if member_path == path {
                continue;
            }
            let member_doc = manifest(&member_path)?;
            package_versions(&member_path, &member_doc, own.as_ref(), &mut found)?;
        }
    }
    Ok(found)
}

/// The workspace manifest Cargo would use for a package at `root`.
fn owning_workspace(root: &Path, doc: &DocumentMut) -> Result<Option<(PathBuf, DocumentMut)>, String> {
    if doc.get("workspace").is_some() {
        return Ok(Some((root.join("Cargo.toml"), doc.clone())));
    }
    if let Some(explicit) = doc["package"].get("workspace").and_then(Item::as_str) {
        let path = root.join(explicit).join("Cargo.toml");
        return Ok(Some((path.clone(), manifest(&path)?)));
    }
    let mut dir = root.canonicalize().map_err(|e| format!("{}: {e}", root.display()))?;
    while dir.pop() {
        let path = dir.join("Cargo.toml");
        if path.is_file() {
            let candidate = manifest(&path)?;
            if candidate.get("workspace").is_some() {
                return Ok(Some((path, candidate)));
            }
        }
    }
    Ok(None)
}

fn package_versions(
    path: &Path,
    doc: &DocumentMut,
    workspace: Option<&(PathBuf, DocumentMut)>,
    found: &mut Vec<Declared>,
) -> Result<(), String> {
    let mut tables: Vec<(String, &dyn TableLike)> = Vec::new();
    for kind in KINDS {
        if let Some(table) = doc.get(kind).and_then(Item::as_table_like) {
            tables.push((kind.into(), table));
        }
    }
    if let Some(targets) = doc.get("target").and_then(Item::as_table_like) {
        for (cfg, target) in targets.iter() {
            for kind in KINDS {
                if let Some(table) = target.get(kind).and_then(Item::as_table_like) {
                    tables.push((format!("target.'{cfg}'.{kind}"), table));
                }
            }
        }
    }
    for (name, table) in tables {
        for (key, item) in table.iter() {
            let origin = format!("{}: {name}.{key}", path.display());
            if item.get("workspace").and_then(Item::as_bool) == Some(true) {
                let Some((ws_path, ws)) = workspace else {
                    return Err(format!(
                        "{origin} herda do workspace, mas nenhum [workspace] foi encontrado."
                    ));
                };
                let Some(inherited) =
                    ws.get("workspace").and_then(|w| w.get("dependencies")).and_then(|d| d.get(key))
                else {
                    return Err(format!(
                        "{origin} herda do workspace, mas {} não declara {key}.",
                        ws_path.display()
                    ));
                };
                // The member may not rename an inherited dependency; the workspace entry decides.
                push(found, format!("{origin} → {}", ws_path.display()), key, inherited);
            } else {
                push(found, origin, key, item);
            }
        }
    }
    Ok(())
}

fn workspace_table_versions(path: &Path, doc: &DocumentMut, found: &mut Vec<Declared>) {
    if let Some(table) =
        doc.get("workspace").and_then(|w| w.get("dependencies")).and_then(Item::as_table_like)
    {
        for (key, item) in table.iter() {
            push(found, format!("{}: workspace.dependencies.{key}", path.display()), key, item);
        }
    }
}

fn push(found: &mut Vec<Declared>, origin: String, key: &str, item: &Item) {
    let package = item.get("package").and_then(Item::as_str).unwrap_or(key);
    if !package.starts_with("ravenrs") {
        return;
    }
    let version = item.as_str().or_else(|| item.get("version").and_then(Item::as_str));
    if let Some(version) = version {
        let declared = Declared { origin, package: package.into(), version: version.into() };
        if !found.iter().any(|d| d.package == declared.package && d.version == declared.version) {
            found.push(declared);
        }
    }
}

/// Members of a workspace, expanding `dir/*` globs and honoring `exclude`.
fn members(root: &Path, doc: &DocumentMut) -> Result<Vec<PathBuf>, String> {
    let list = |key: &str| -> Vec<String> {
        doc["workspace"]
            .get(key)
            .and_then(Item::as_array)
            .map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_owned)).collect())
            .unwrap_or_default()
    };
    let exclude: Vec<PathBuf> = list("exclude").iter().map(|e| root.join(e)).collect();
    let mut out = Vec::new();
    for pattern in list("members") {
        let candidates = match pattern.strip_suffix("/*") {
            Some(parent) => {
                let mut dirs: Vec<PathBuf> = std::fs::read_dir(root.join(parent))
                    .map_err(|e| format!("workspace.members {pattern}: {e}"))?
                    .filter_map(Result::ok)
                    .map(|e| e.path())
                    .collect();
                dirs.sort();
                dirs
            }
            None => vec![root.join(&pattern)],
        };
        for dir in candidates {
            if dir.join("Cargo.toml").is_file() && !exclude.iter().any(|e| dir.starts_with(e)) {
                out.push(dir);
            }
        }
    }
    Ok(out)
}
