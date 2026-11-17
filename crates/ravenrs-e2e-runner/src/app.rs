//! The apps a scenario opens apart, in a child process with no window, over the test channel.
//!
//! Shared by Raven Test Studio and `cargo ravenrs e2e`. The runner keeps no list of apps of its own. An app declares itself in its `Cargo.toml`, in
//! `[package.metadata.ravenrs.e2e]`: the name its scenarios open (`app`), the binary, the first
//! version that speaks the channel (`since`), a variable that points at another copy, and where
//! its installer puts it. The Studio learns an app from that declaration when a project is opened
//! with `--project` ([`crate::project`]), and knows Raven Finanças by the declaration in its
//! own `Cargo.toml`, read when the Studio is built, so the installed Studio can open the installed
//! app with no checkout.
//!
//! An app is found the way a person has it: a path in its variable; for an app of the project
//! the Studio was pointed at, its code built with Cargo in that project; then the copy the
//! installer put on this computer; then, inside the RavenRS checkout, its code. An installed copy
//! from before the test channel is never opened with `--e2e-serve`: it would read the option as
//! the path of a database and open its window.

use std::path::{Path, PathBuf};
use std::process::Command as Process;
use std::sync::Mutex;
use std::time::Duration;

use ravenrs_e2e_wire::{Client, SERVE_FLAG};
use serde_json::Value;

use crate::session::{Failure, Session};

/// An app the Studio opens apart, as its `Cargo.toml` declares it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct App {
    /// The name on a scenario's Open block.
    pub name: String,
    /// Its Cargo package.
    pub package: String,
    /// The binary of the package that is the app.
    pub binary: String,
    /// The first version that speaks the test channel; empty when any does.
    pub since: String,
    /// A variable that points at another copy of its binary.
    pub variable: String,
    /// Where its installer puts it, from the home folder.
    pub installed: Vec<String>,
    /// Cargo features enabled only when this tool builds the app for a test.
    pub features: Vec<String>,
    /// The project to build its code in with Cargo, and whether that comes before an
    /// installed copy (it does for a project the Studio was pointed at).
    pub source: Option<PathBuf>,
    /// Whether its code comes before the installed copy.
    pub source_first: bool,
    /// A binary to open, before anything else: a test opens the Studio it was built with.
    pub binary_path: Option<PathBuf>,
}

impl App {
    /// The app a package declares in `[package.metadata.ravenrs.e2e]`, from the JSON of
    /// `cargo metadata`; `None` when it declares none.
    pub fn declared(package: &Value) -> Option<Self> {
        let e2e = package.pointer("/metadata/ravenrs/e2e")?;
        let text = |key: &str| e2e.get(key).and_then(Value::as_str).unwrap_or_default().to_owned();
        let name = package.get("name").and_then(Value::as_str).unwrap_or_default().to_owned();
        let installed = e2e
            .get("installed")
            .and_then(Value::as_array)
            .map(|list| list.iter().filter_map(Value::as_str).map(str::to_owned).collect())
            .unwrap_or_default();
        Some(Self {
            name: Some(text("app")).filter(|n| !n.is_empty()).unwrap_or_else(|| name.clone()),
            binary: Some(text("binary")).filter(|b| !b.is_empty()).unwrap_or_else(|| name.clone()),
            package: name,
            since: text("since"),
            variable: text("variable"),
            installed,
            features: features_of(e2e).ok()?,
            source: None,
            source_first: false,
            binary_path: None,
        })
    }

    /// The same declaration read from the text of a `Cargo.toml`, for the app the Studio knows
    /// when it is built. TOML syntax is read by the same parser used for suite configuration;
    /// its declaration is then interpreted exactly as Cargo's JSON metadata.
    pub fn declared_in(manifest: &str) -> Option<Self> {
        let document: toml_edit::DocumentMut = manifest.parse().ok()?;
        let package = document.get("package")?;
        let name = package.get("name")?.as_str()?;
        let declaration = package.get("metadata")?.get("ravenrs")?.get("e2e")?.as_table_like()?;
        let e2e: serde_json::Map<String, Value> = declaration
            .iter()
            .map(|(key, item)| {
                let value = if let Some(text) = item.as_str() {
                    Value::String(text.to_owned())
                } else if let Some(array) = item.as_array() {
                    Value::Array(
                        array
                            .iter()
                            .map(|item| {
                                item.as_str().map_or(Value::Null, |text| Value::String(text.to_owned()))
                            })
                            .collect(),
                    )
                } else {
                    Value::Null
                };
                (key.to_owned(), value)
            })
            .collect();
        Self::declared(&serde_json::json!({ "name": name, "metadata": { "ravenrs": { "e2e": e2e } } }))
    }

    pub(crate) fn declaration_error(package: &Value) -> Option<String> {
        let e2e = package.pointer("/metadata/ravenrs/e2e")?;
        features_of(e2e).err()
    }
}

fn features_of(e2e: &Value) -> Result<Vec<String>, String> {
    let Some(value) = e2e.get("features") else { return Ok(Vec::new()) };
    let why =
        || "package.metadata.ravenrs.e2e.features precisa ser uma lista de textos não vazios".to_owned();
    value
        .as_array()
        .ok_or_else(why)?
        .iter()
        .map(|feature| {
            feature.as_str().filter(|text| !text.trim().is_empty()).map(str::to_owned).ok_or_else(why)
        })
        .collect()
}

/// The apps learned from projects opened in this run, before the ones the Studio knows.
static LEARNED: Mutex<Vec<App>> = Mutex::new(Vec::new());

/// Makes `app` known by its name, as an app of a project the Studio was pointed at: its code
/// there comes before any installed copy.
pub fn learn(app: App) {
    if let Ok(mut learned) = LEARNED.lock() {
        learned.retain(|known| known.name != app.name);
        learned.push(app);
    }
}

/// Whether `name` is an app learned from a project the Studio was pointed at.
pub fn learned(name: &str) -> bool {
    LEARNED.lock().is_ok_and(|apps| apps.iter().any(|app| app.name == name))
}

/// Apps a tool knows by itself, after the learned ones: the Studio knows Raven Finanças.
static KNOWN: Mutex<Vec<App>> = Mutex::new(Vec::new());

/// Makes `app` known by its name, after any app learned from a project: an app a tool brings
/// the declaration of, such as the Studio with Raven Finanças.
pub fn know(app: App) {
    if let Ok(mut known) = KNOWN.lock()
        && !known.iter().any(|k| k.name == app.name)
    {
        known.push(app);
    }
}

/// The app named `name`: one learned from a project, else one a tool made known.
pub fn known(name: &str) -> Option<App> {
    let find = |apps: &Mutex<Vec<App>>| {
        apps.lock().ok().and_then(|apps| apps.iter().find(|app| app.name == name).cloned())
    };
    find(&LEARNED).or_else(|| find(&KNOWN))
}

/// How long an installed app has to say hello.
pub const READY_TIME: Duration = Duration::from_secs(20);
/// How long Cargo may build an app before its process group is stopped.
pub const BUILD_TIME: Duration = Duration::from_secs(900);

/// The scenarios the app named `name` brings, read from its hello: the `.raventest` files of its
/// `tests/e2e/`, embedded in the app. `None` when the Studio does not know the app.
pub fn scenarios(name: &str) -> Option<Result<Vec<ravenrs_e2e_wire::Scenario>, String>> {
    let app = known(name)?;
    Some(start(&app).map_err(|error| error.to_string()).and_then(|client| {
        if client.scenarios().is_empty() {
            return Err(format!(
                "O {} aberto não traz cenários na pasta tests/e2e; atualize-o, ou rode um arquivo .raventest com --run",
                app.name
            ));
        }
        client
            .scenarios()
            .iter()
            .map(|text| ravenrs_e2e_wire::Scenario::parse(text).map_err(|e| format!("cenário do {}: {e}", app.name)))
            .collect()
    }))
}

/// Opens the app named `name` apart, if it is one the Studio knows.
pub fn open(name: &str) -> Option<Result<Session, Box<Failure>>> {
    let app = known(name)?;
    Some(start(&app).map(Session::Apart))
}

fn start(app: &App) -> Result<Client, Box<Failure>> {
    let mut process = match find(app).map_err(start_failure)? {
        Launch::Binary(path) => Process::new(path),
        Launch::Source(root) => {
            let binary = crate::build::compile_with_features(
                &root,
                &app.package,
                &app.binary,
                &app.features,
                BUILD_TIME,
            )?;
            let mut process = Process::new(binary);
            process.current_dir(root);
            process
        }
    };
    process.arg(SERVE_FLAG);
    Client::spawn(process, READY_TIME).map_err(|failure| Box::new(failure.into()))
}

enum Launch {
    Binary(PathBuf),
    Source(PathBuf),
}

/// Installed and explicit binaries run directly; source projects compile first.
fn find(app: &App) -> Result<Launch, String> {
    if let Some(path) = &app.binary_path {
        return Ok(Launch::Binary(path.clone()));
    }
    if let Some(path) =
        Some(&app.variable).filter(|v| !v.is_empty()).and_then(std::env::var_os).map(PathBuf::from)
    {
        return Ok(Launch::Binary(path));
    }
    if app.source_first
        && let Some(root) = &app.source
    {
        return Ok(Launch::Source(root.clone()));
    }
    let found = installed(app).into_iter().map(|path| {
        let version = version_of(&path);
        (path, version)
    });
    let too_old = match pick_installed(found, &app.since) {
        Ok(path) => return Ok(Launch::Binary(path)),
        Err(too_old) => too_old,
    };
    if let Some(root) = &app.source {
        return Ok(Launch::Source(root.clone()));
    }
    Err(match too_old {
        Some(v) => format!(
            "O {} instalado é a versão {v}, e o teste pelo canal começa na {}: atualize o {} e rode de novo",
            app.name, app.since, app.name
        ),
        None => format!(
            "O {} não está instalado neste computador. Instale-o, rode os cenários dentro do projeto dele (cargo ravenrs e2e run, ou --project no Studio), ou diga onde ele está em {}",
            app.name,
            if app.variable.is_empty() { "uma variável de ambiente" } else { &app.variable }
        ),
    })
}

/// The first installed copy that speaks the test channel (version `since` or later), or the
/// version of the one that is too old, to tell the person to update it. A copy that does not
/// say its version is not opened.
pub fn pick_installed(
    found: impl IntoIterator<Item = (PathBuf, Option<String>)>,
    since: &str,
) -> Result<PathBuf, Option<String>> {
    let mut too_old = None;
    for (path, version) in found {
        match version {
            Some(v) if !is_older(&v, since) => return Ok(path),
            Some(v) => too_old = Some(v),
            None => {}
        }
    }
    Err(too_old)
}

/// Opens the app at `binary` apart, as its variable does.
pub fn open_from(binary: &Path) -> Result<Session, Box<Failure>> {
    let mut process = Process::new(binary);
    process.arg(SERVE_FLAG);
    Client::spawn(process, READY_TIME).map(Session::Apart).map_err(|failure| Box::new(failure.into()))
}

fn start_failure(detail: String) -> Box<Failure> {
    Box::new(ravenrs_e2e_wire::Failure::of(ravenrs_e2e_wire::codes::APP_START, detail).into())
}

/// Where the installer puts the app, for this person.
fn installed(app: &App) -> Vec<PathBuf> {
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(PathBuf::from);
    let mut places: Vec<PathBuf> = Vec::new();
    if cfg!(target_os = "macos") {
        places.extend(
            app.installed.iter().filter(|p| p.starts_with("Applications/")).map(|p| Path::new("/").join(p)),
        );
    }
    if let Some(home) = home {
        places.extend(app.installed.iter().map(|tail| home.join(tail)));
    }
    places.into_iter().filter(|p| p.is_file()).collect()
}

/// The version the app says with `--versao`, which every installed RavenRS app answers.
fn version_of(path: &Path) -> Option<String> {
    let out = Process::new(path).arg("--versao").output().ok()?;
    let text = String::from_utf8(out.stdout).ok()?;
    let line = text.lines().next()?.trim().to_owned();
    (out.status.success() && line.split('.').all(|part| part.parse::<u32>().is_ok())).then_some(line)
}

/// Whether version `a` comes before `b`, part by part.
pub fn is_older(a: &str, b: &str) -> bool {
    let parts = |v: &str| v.split('.').map(|p| p.parse::<u32>().unwrap_or(0)).collect::<Vec<_>>();
    parts(a) < parts(b)
}

/// The RavenRS checkout this process runs in, when it has a package called `package` among its
/// examples or tools.
pub fn checkout_with(package: &str) -> Option<PathBuf> {
    let here = std::env::current_dir().ok()?;
    here.ancestors()
        .find(|dir| {
            ["examples", "tools"].iter().any(|group| {
                std::fs::read_dir(dir.join(group)).into_iter().flatten().flatten().any(|entry| {
                    std::fs::read_to_string(entry.path().join("Cargo.toml"))
                        .is_ok_and(|text| text.contains(&format!("name = \"{package}\"")))
                })
            })
        })
        .map(Path::to_path_buf)
}
