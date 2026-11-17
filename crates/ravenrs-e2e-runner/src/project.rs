//! A Rust project the Studio is pointed at: which of its packages are made with RavenRS, which
//! of them are apps, whether each speaks the test channel, which scenarios it keeps, and which
//! screens it builds, so a person knows what can be tested there without knowing the project.
//!
//! The project is read, never built: `cargo metadata --no-deps` says the packages, and their
//! sources are searched for the views they build (`impl View for`, `impl Component for`,
//! functions that return `impl View`) and the test ids they declare (`.test_id("…")`).

use std::path::{Path, PathBuf};
use std::process::Command as Process;

use regex_lite::Regex;
use serde_json::{Value, json};

use crate::app::{self as apps, App};

/// A package of the project made with RavenRS.
#[derive(Clone, Debug, PartialEq)]
pub struct Found {
    /// The name its scenarios open: the one it declares for the Studio, or its package name.
    pub name: String,
    /// Its Cargo package.
    pub package: String,
    /// Its folder.
    pub folder: PathBuf,
    /// The binaries it builds; none for a library of components.
    pub binaries: Vec<String>,
    /// Whether it answers the test channel (`--e2e-serve`).
    pub channel: bool,
    /// What it declares for the Studio in `[package.metadata.ravenrs.e2e]`, when it does.
    pub declared: Option<App>,
    /// Its scenario files in `tests/e2e/`, with what is wrong with each that does not read.
    pub scenarios: Vec<(PathBuf, Result<String, String>)>,
    /// The views it builds.
    pub screens: Vec<Screen>,
    /// Its `e2e.toml`, or the defaults; a file that does not read leaves the defaults here and
    /// says why in `config_error`.
    pub config: crate::config::Config,
    /// Why its `e2e.toml` could not be read, when it could not.
    pub config_error: Option<String>,
}

/// A view the package builds, where, and the test ids its file declares.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Screen {
    /// The type or the function.
    pub name: String,
    /// `View`, `Component` or `fn`.
    pub kind: &'static str,
    /// The file, from the project's folder.
    pub file: PathBuf,
    /// The line, counting from 1.
    pub line: usize,
    /// The test ids declared in the same file.
    pub test_ids: Vec<String>,
}

impl Found {
    /// Whether it is an app, something that opens, and not a library of components.
    pub fn is_app(&self) -> bool {
        !self.binaries.is_empty()
    }
}

/// What a project has that is made with RavenRS.
#[derive(Clone, Debug, PartialEq)]
pub struct Project {
    /// The folder pointed at.
    pub root: PathBuf,
    /// Its packages made with RavenRS, in name order.
    pub found: Vec<Found>,
    /// The folder cargo builds it into.
    pub target: PathBuf,
}

/// The crates whose use makes a package one made with RavenRS.
fn uses_ravenrs(dependency: &str) -> bool {
    dependency == "ravenrs"
        || dependency.starts_with("ravenrs-widgets")
        || dependency.starts_with("ravenrs-app")
}

/// Reads the project in `folder` (a package or a workspace, or any folder inside one).
pub fn read(folder: &Path) -> Result<Project, String> {
    let root = folder.canonicalize().map_err(|e| format!("{}: {e}", folder.display()))?;
    let manifest = root.join("Cargo.toml");
    let mut cargo = Process::new("cargo");
    cargo.args(["metadata", "--no-deps", "--format-version", "1", "--offline"]).current_dir(&root);
    if manifest.is_file() {
        cargo.arg("--manifest-path").arg(&manifest);
    }
    let out = cargo.output().map_err(|e| format!("não deu para rodar o cargo: {e}"))?;
    if !out.status.success() {
        let why = String::from_utf8_lossy(&out.stderr);
        return Err(format!("{} não é um projeto Rust que o cargo leia: {}", root.display(), why.trim()));
    }
    let metadata: Value = serde_json::from_slice(&out.stdout).map_err(|e| e.to_string())?;
    Ok(of_metadata(&root, &metadata))
}

/// The project as `cargo metadata` describes it, counting only the packages inside `root`.
pub fn of_metadata(root: &Path, metadata: &Value) -> Project {
    let mut found: Vec<Found> = metadata["packages"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|package| {
            let manifest = PathBuf::from(package["manifest_path"].as_str()?);
            let folder = manifest.parent()?.to_path_buf();
            if !folder.starts_with(root) {
                return None;
            }
            let dependencies: Vec<&str> = package["dependencies"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|d| d["name"].as_str())
                .collect();
            if !dependencies.iter().any(|d| uses_ravenrs(d)) {
                return None;
            }
            let binaries: Vec<String> = package["targets"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|t| t["kind"].as_array().is_some_and(|k| k.iter().any(|k| k == "bin")))
                .filter_map(|t| t["name"].as_str().map(str::to_owned))
                .collect();
            let declared = App::declared(package);
            let package_name = package["name"].as_str()?.to_owned();
            let sources = rust_files(&folder.join("src"));
            let channel = dependencies.contains(&"ravenrs-e2e-wire")
                && sources.iter().any(|file| {
                    std::fs::read_to_string(file).is_ok_and(|text| {
                        text.contains("e2e-serve")
                            || text.contains("SERVE_FLAG")
                            || text.contains("ravenrs_e2e_wire::serve")
                    })
                });
            let (config, config_error) = match crate::config::Config::read(&folder) {
                Ok(config) => (config, None),
                Err(why) => (crate::config::Config::default(), Some(why)),
            };
            let config_error = App::declaration_error(package)
                .map(|why| format!("{}: {why}", manifest.display()))
                .or(config_error);
            Some(Found {
                name: declared.as_ref().map_or_else(|| package_name.clone(), |d| d.name.clone()),
                package: package_name,
                scenarios: scenarios_in(&folder.join(&config.folder)),
                screens: screens_in(root, &sources),
                folder,
                binaries,
                channel,
                declared,
                config,
                config_error,
            })
        })
        .collect();
    found.sort_by(|a, b| a.package.cmp(&b.package));
    let target = metadata["target_directory"].as_str().map_or_else(|| root.join("target"), PathBuf::from);
    Project { root: root.to_path_buf(), found, target }
}

fn rust_files(folder: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut pending = vec![folder.to_path_buf()];
    while let Some(dir) = pending.pop() {
        for entry in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|e| e == "rs") {
                files.push(path);
            }
        }
    }
    files.sort();
    files
}

fn scenarios_in(folder: &Path) -> Vec<(PathBuf, Result<String, String>)> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(folder)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == ravenrs_e2e_wire::EXTENSION))
        .collect();
    files.sort();
    files
        .into_iter()
        .map(|file| {
            let read = std::fs::read_to_string(&file).map_err(|e| e.to_string()).and_then(|text| {
                ravenrs_e2e_wire::Scenario::parse(&text).map(|s| s.name).map_err(|e| e.to_string())
            });
            (file, read)
        })
        .collect()
}

/// The views the files build, with the test ids each file declares.
fn screens_in(root: &Path, files: &[PathBuf]) -> Vec<Screen> {
    let (Ok(implemented), Ok(function), Ok(test_id)) = (
        Regex::new(r"impl(?:\s*<[^>]*>)?\s+(View|Component)\s+for\s+([A-Za-z_]\w*)"),
        Regex::new(r"pub\s+fn\s+([a-z_]\w*)\s*(?:<[^>]*>)?\s*\([^)]*\)\s*->\s*impl\s+View\b"),
        Regex::new(r#"test_id\(\s*"([^"]+)""#),
    ) else {
        return Vec::new();
    };
    let mut screens = Vec::new();
    for file in files {
        let Ok(text) = std::fs::read_to_string(file) else { continue };
        let ids: Vec<String> = test_id.captures_iter(&text).map(|c| c[1].to_owned()).collect();
        let relative = file.strip_prefix(root).unwrap_or(file).to_path_buf();
        for (index, line) in text.lines().enumerate() {
            let found = implemented
                .captures(line)
                .map(|c| (c[2].to_owned(), if &c[1] == "View" { "View" } else { "Component" }))
                .or_else(|| function.captures(line).map(|c| (c[1].to_owned(), "fn")));
            if let Some((name, kind)) = found {
                screens.push(Screen {
                    name,
                    kind,
                    file: relative.clone(),
                    line: index + 1,
                    test_ids: ids.clone(),
                });
            }
        }
    }
    screens
}

impl Project {
    /// Refuses execution when an app's test configuration is invalid, while discovery can
    /// still show every error. Both command-line tools use this before opening an app.
    pub fn check_configs(&self) -> Result<(), String> {
        let errors: Vec<&str> = self.found.iter().filter_map(|found| found.config_error.as_deref()).collect();
        if errors.is_empty() { Ok(()) } else { Err(errors.join("\n")) }
    }
    /// Where a run of this project keeps what it found: `e2e` inside the folder cargo builds
    /// this project into, as `cargo metadata` says (the workspace's `target`, or
    /// `CARGO_TARGET_DIR` and `build.target-dir` when set), so git never sees it.
    pub fn e2e_folder(&self) -> PathBuf {
        self.target.join("e2e")
    }

    /// Makes the apps of the project known by name, its code before any installed copy, so its
    /// scenarios open them.
    pub fn learn_apps(&self) {
        for found in self.found.iter().filter(|f| f.is_app() && f.channel && f.config_error.is_none()) {
            let declared = found.declared.clone().unwrap_or_default();
            apps::learn(App {
                name: found.name.clone(),
                package: found.package.clone(),
                binary: Some(declared.binary.clone())
                    .filter(|b| !b.is_empty() && found.binaries.contains(b))
                    .unwrap_or_else(|| found.binaries[0].clone()),
                source: Some(self.root.clone()),
                source_first: true,
                binary_path: None,
                ..declared
            });
        }
    }

    /// The scenario folders of the apps that can run them: with the channel and scenarios.
    pub fn scenario_folders(&self) -> Vec<PathBuf> {
        self.found
            .iter()
            .filter(|f| f.is_app() && f.channel && !f.scenarios.is_empty())
            .map(|f| f.folder.join(&f.config.folder))
            .collect()
    }

    /// Everything, for a tool or an AI.
    pub fn json(&self) -> Value {
        let found: Vec<Value> = self
            .found
            .iter()
            .map(|f| {
                json!({
                    "name": f.name,
                    "package": f.package,
                    "kind": if f.is_app() { "app" } else { "library" },
                    "folder": f.folder.strip_prefix(&self.root).unwrap_or(&f.folder),
                    "binaries": f.binaries,
                    "channel": f.channel,
                    "declared": f.declared.is_some(),
                    "features": f.declared.as_ref().map(|app| app.features.as_slice()).unwrap_or_default(),
                    "config_error": f.config_error,
                    "next": next_step(f),
                    "scenarios": f.scenarios.iter().map(|(file, read)| match read {
                        Ok(name) => json!({ "file": file.strip_prefix(&self.root).unwrap_or(file), "name": name }),
                        Err(why) => json!({ "file": file.strip_prefix(&self.root).unwrap_or(file), "error": why }),
                    }).collect::<Vec<_>>(),
                    "screens": f.screens.iter().map(|s| json!({
                        "name": s.name, "kind": s.kind, "file": s.file, "line": s.line, "test_ids": s.test_ids,
                    })).collect::<Vec<_>>(),
                })
            })
            .collect();
        json!({
            "root": self.root,
            "apps": self.found.iter().filter(|f| f.is_app()).count(),
            "libraries": self.found.iter().filter(|f| !f.is_app()).count(),
            "found": found,
        })
    }

    /// A few lines for a person.
    pub fn text(&self) -> String {
        if self.found.is_empty() {
            return format!("{}: nenhum pacote feito com RavenRS", self.root.display());
        }
        let mut out = format!("{}\n", self.root.display());
        for f in &self.found {
            let unmarked = f.screens.iter().filter(|s| s.test_ids.is_empty()).count();
            out.push_str(&format!(
                "\n{} ({}, {}): {} telas, {} sem marcas para teste; canal de teste: {}; cenários: {}\n",
                f.name,
                f.package,
                if f.is_app() { format!("app {}", f.binaries.join(", ")) } else { "biblioteca".to_owned() },
                f.screens.len(),
                unmarked,
                if f.channel { "sim" } else { "não" },
                f.scenarios.len(),
            ));
            if let Some(app) = &f.declared
                && !app.features.is_empty()
            {
                out.push_str(&format!("  features de teste: {}\n", app.features.join(", ")));
            }
            for (file, read) in &f.scenarios {
                let shown = file.strip_prefix(&self.root).unwrap_or(file).display().to_string();
                match read {
                    Ok(name) => out.push_str(&format!("  cenário {shown}: {name}\n")),
                    Err(why) => out.push_str(&format!("  cenário {shown}: não lê ({why})\n")),
                }
            }
            if let Some(why) = &f.config_error {
                out.push_str(&format!("  configuração de teste não lê: {why}\n"));
            }
            if let Some(next) = next_step(f) {
                out.push_str(&format!("  próximo passo: {next}\n"));
            }
        }
        out
    }
}

/// What to do so the package can be tested by the Studio, when something is missing.
fn next_step(f: &Found) -> Option<String> {
    if !f.is_app() {
        return Some("é uma biblioteca: teste as telas dela pelo app que as usa".to_owned());
    }
    if !f.channel {
        return Some(format!(
            "ligue o canal de teste: dependa de ravenrs-e2e-wire e trate {} no main (cargo ravenrs e2e install, F6 da #627, fará isso)",
            ravenrs_e2e_wire::SERVE_FLAG
        ));
    }
    if f.scenarios.is_empty() {
        return Some(format!(
            "escreva cenários .raventest em {}/ do app, ou numa pasta do QA",
            ravenrs_e2e_wire::FOLDER
        ));
    }
    None
}
