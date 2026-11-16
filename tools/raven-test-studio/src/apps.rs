//! The apps a scenario opens apart, in a child process with no window, over the test channel.
//!
//! Each app is found the way a person has it: a path given in an environment variable, then the
//! copy the installer put on this computer, then, inside the RavenRS checkout, its code with
//! `cargo run`. An installed copy from before the test channel is never opened with
//! `--e2e-serve`: it would read the option as the path of a database and open its window.

use std::path::{Path, PathBuf};
use std::process::Command as Process;
use std::time::Duration;

use ravenrs_e2e_wire::{Client, SERVE_FLAG};

use crate::runner::Session;

/// The name an Open block gives to open Raven Finanças.
pub const FINANCE_APP: &str = "Raven Finanças";

/// An app the Studio opens apart.
struct Apart {
    /// The name on the Open block.
    name: &'static str,
    /// The variable that points at its binary, for a copy anywhere else.
    variable: &'static str,
    /// Its package in the RavenRS workspace, for `cargo run`.
    package: &'static str,
    /// The binary of the package that is the app.
    binary: &'static str,
    /// Its folder in the checkout.
    folder: &'static str,
    /// Where its installer puts it on macOS, Linux and Windows, from the home folder.
    installed: [&'static str; 3],
    /// The first version that speaks the test channel.
    since: &'static str,
}

const FINANCE: Apart = Apart {
    name: FINANCE_APP,
    variable: "RAVEN_FINANCAS",
    package: "family-finance",
    binary: "family-finance",
    folder: "examples/family-finance",
    installed: [
        "Applications/Raven Finanças.app/Contents/MacOS/family-finance",
        ".local/bin/family-finance",
        "AppData/Local/Programs/RavenFinancas/family-finance.exe",
    ],
    since: "0.1.39",
};

/// How long an installed app has to say hello, and one built first with `cargo run`.
const READY_TIME: Duration = Duration::from_secs(20);
const BUILD_TIME: Duration = Duration::from_secs(900);

/// Opens the app named `name` apart, if it is one the Studio knows.
pub fn open(name: &str) -> Option<Result<Session, String>> {
    let app = [FINANCE].into_iter().find(|app| app.name == name)?;
    Some(start(&app).map(Session::Apart))
}

fn start(app: &Apart) -> Result<Client, String> {
    let (mut process, time) = find(app)?;
    process.arg(SERVE_FLAG);
    Client::spawn(process, time).map_err(|f| {
        let tail = if f.screen.is_empty() { String::new() } else { format!("\n{}", f.screen) };
        format!("[{}] Não deu para abrir o {}: {}{tail}", f.code, app.name, f.message)
    })
}

/// The command that runs the app, and how long it may take to answer.
fn find(app: &Apart) -> Result<(Process, Duration), String> {
    if let Some(path) = std::env::var_os(app.variable).map(PathBuf::from) {
        return Ok((Process::new(path), READY_TIME));
    }
    let found = installed(app).into_iter().map(|path| {
        let version = version_of(&path);
        (path, version)
    });
    let too_old = match pick_installed(found, app.since) {
        Ok(path) => return Ok((Process::new(path), READY_TIME)),
        Err(too_old) => too_old,
    };
    if let Some(root) = checkout(app) {
        let mut cargo = Process::new("cargo");
        cargo.current_dir(root).args(["run", "-q", "-p", app.package, "--bin", app.binary, "--"]);
        return Ok((cargo, BUILD_TIME));
    }
    Err(match too_old {
        Some(v) => format!(
            "O {} instalado é a versão {v}, e o teste pelo Studio começa na {}: atualize o {} (Ajustes › Atualizar) e rode de novo",
            app.name, app.since, app.name
        ),
        None => format!(
            "O {} não está instalado neste computador. Instale-o, ou rode o Studio dentro do repositório do RavenRS, ou diga onde ele está em {}",
            app.name, app.variable
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

/// Opens Raven Finanças from `binary`, as `RAVEN_FINANCAS` does.
pub fn open_finance_from(binary: &Path) -> Result<Session, String> {
    let mut process = Process::new(binary);
    process.arg(SERVE_FLAG);
    Client::spawn(process, READY_TIME).map(Session::Apart).map_err(|f| format!("[{}] {}", f.code, f.message))
}

/// Where the installer puts the app, for this person.
fn installed(app: &Apart) -> Vec<PathBuf> {
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(PathBuf::from);
    let mut places: Vec<PathBuf> = Vec::new();
    if cfg!(target_os = "macos") {
        places.push(Path::new("/").join(app.installed[0]));
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

/// The RavenRS checkout this process runs in, when it has the app's code.
fn checkout(app: &Apart) -> Option<PathBuf> {
    let here = std::env::current_dir().ok()?;
    here.ancestors().find(|dir| dir.join(app.folder).join("Cargo.toml").is_file()).map(Path::to_path_buf)
}
