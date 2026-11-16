//! The apps a scenario opens apart, in a child process with no window, over the test channel.
//!
//! The Studio keeps no list of apps of its own. An app declares itself in its `Cargo.toml`, in
//! `[package.metadata.ravenrs.e2e]`: the name its scenarios open (`app`), the binary, the first
//! version that speaks the channel (`since`), a variable that points at another copy, and where
//! its installer puts it. The Studio learns an app from that declaration when a project is opened
//! with `--project` ([`crate::project`]), and knows Raven Finanças by the declaration in its
//! own `Cargo.toml`, read when the Studio is built, so the installed Studio can open the installed
//! app with no checkout.
//!
//! An app is found the way a person has it: a path in its variable; for an app of the project
//! the Studio was pointed at, its code with `cargo run` in that project; then the copy the
//! installer put on this computer; then, inside the RavenRS checkout, its code. An installed copy
//! from before the test channel is never opened with `--e2e-serve`: it would read the option as
//! the path of a database and open its window.

use std::path::{Path, PathBuf};
use std::process::Command as Process;
use std::sync::Mutex;
use std::time::Duration;

use ravenrs_e2e_wire::{Client, SERVE_FLAG};
use serde_json::Value;

use crate::runner::Session;

/// The name Raven Finanças declares, which `--run financas` opens.
pub const FINANCE_APP: &str = "Raven Finanças";

/// Raven Finanças' own declaration, read when the Studio is built.
const FINANCE_MANIFEST: &str = include_str!("../../../examples/family-finance/Cargo.toml");

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
    /// The project to run its code in with `cargo run`, and whether that comes before an
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
            source: None,
            source_first: false,
            binary_path: None,
        })
    }

    /// The same declaration read from the text of a `Cargo.toml`, for the app the Studio knows
    /// when it is built. Only what the declaration uses is read: `key = "text"` and lists of texts.
    pub fn declared_in(manifest: &str) -> Option<Self> {
        let mut package = serde_json::Map::new();
        let mut e2e = serde_json::Map::new();
        let mut section = String::new();
        let mut list: Option<(String, Vec<Value>)> = None;
        for raw in manifest.lines() {
            let line = raw.split(" #").next().unwrap_or_default().trim();
            if let Some((key, items)) = list.as_mut() {
                if line.starts_with(']') {
                    e2e.insert(key.clone(), Value::Array(std::mem::take(items)));
                    list = None;
                } else if let Some(item) = quoted(line.trim_end_matches(',')) {
                    items.push(Value::String(item));
                }
                continue;
            }
            if line.starts_with('[') {
                section = line.trim_matches(|c| c == '[' || c == ']').to_owned();
                continue;
            }
            let Some((key, value)) = line.split_once('=') else { continue };
            let (key, value) = (key.trim(), value.trim());
            match section.as_str() {
                "package" if key == "name" => {
                    package.insert("name".into(), Value::String(quoted(value)?));
                }
                "package.metadata.ravenrs.e2e" if value == "[" => list = Some((key.to_owned(), Vec::new())),
                "package.metadata.ravenrs.e2e" => {
                    if let Some(text) = quoted(value) {
                        e2e.insert(key.to_owned(), Value::String(text));
                    }
                }
                _ => {}
            }
        }
        if e2e.is_empty() {
            return None;
        }
        package.insert("metadata".into(), serde_json::json!({ "ravenrs": { "e2e": Value::Object(e2e) } }));
        Self::declared(&Value::Object(package))
    }
}

fn quoted(text: &str) -> Option<String> {
    text.trim().strip_prefix('"')?.strip_suffix('"').map(str::to_owned)
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

/// The app named `name`: one learned from a project, or Raven Finanças.
pub fn known(name: &str) -> Option<App> {
    let learned = LEARNED.lock().ok().and_then(|apps| apps.iter().find(|app| app.name == name).cloned());
    learned.or_else(|| {
        App::declared_in(FINANCE_MANIFEST)
            .filter(|app| app.name == name)
            .map(|app| App { source: checkout_with(&app.package), ..app })
    })
}

/// How long an installed app has to say hello, and one built first with `cargo run`.
const READY_TIME: Duration = Duration::from_secs(20);
const BUILD_TIME: Duration = Duration::from_secs(900);

/// The scenarios the app named `name` brings, read from its hello: the `.raventest` files of its
/// `tests/e2e/`, embedded in the app. `None` when the Studio does not know the app.
pub fn scenarios(name: &str) -> Option<Result<Vec<ravenrs_e2e_wire::Scenario>, String>> {
    let app = known(name)?;
    Some(start(&app).and_then(|client| {
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
pub fn open(name: &str) -> Option<Result<Session, String>> {
    let app = known(name)?;
    Some(start(&app).map(Session::Apart))
}

fn start(app: &App) -> Result<Client, String> {
    let (mut process, time) = find(app)?;
    process.arg(SERVE_FLAG);
    Client::spawn(process, time).map_err(|f| {
        let tail = if f.screen.is_empty() { String::new() } else { format!("\n{}", f.screen) };
        format!("[{}] Não deu para abrir o {}: {}{tail}", f.code, app.name, f.message)
    })
}

/// `cargo run` of the app's binary in `root`.
fn cargo_run(app: &App, root: &Path) -> Process {
    let mut cargo = Process::new("cargo");
    cargo.current_dir(root).args(["run", "-q", "-p", &app.package, "--bin", &app.binary, "--"]);
    cargo
}

/// The command that runs the app, and how long it may take to answer.
fn find(app: &App) -> Result<(Process, Duration), String> {
    if let Some(path) = &app.binary_path {
        return Ok((Process::new(path), READY_TIME));
    }
    if let Some(path) =
        Some(&app.variable).filter(|v| !v.is_empty()).and_then(std::env::var_os).map(PathBuf::from)
    {
        return Ok((Process::new(path), READY_TIME));
    }
    if app.source_first
        && let Some(root) = &app.source
    {
        return Ok((cargo_run(app, root), BUILD_TIME));
    }
    let found = installed(app).into_iter().map(|path| {
        let version = version_of(&path);
        (path, version)
    });
    let too_old = match pick_installed(found, &app.since) {
        Ok(path) => return Ok((Process::new(path), READY_TIME)),
        Err(too_old) => too_old,
    };
    if let Some(root) = &app.source {
        return Ok((cargo_run(app, root), BUILD_TIME));
    }
    Err(match too_old {
        Some(v) => format!(
            "O {} instalado é a versão {v}, e o teste pelo Studio começa na {}: atualize o {} e rode de novo",
            app.name, app.since, app.name
        ),
        None => format!(
            "O {} não está instalado neste computador. Instale-o, aponte o Studio para o projeto dele com --project, ou diga onde ele está em {}",
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

/// Opens Raven Finanças from `binary`, as its variable does.
pub fn open_finance_from(binary: &Path) -> Result<Session, String> {
    let mut process = Process::new(binary);
    process.arg(SERVE_FLAG);
    Client::spawn(process, READY_TIME).map(Session::Apart).map_err(|f| format!("[{}] {}", f.code, f.message))
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
fn checkout_with(package: &str) -> Option<PathBuf> {
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
