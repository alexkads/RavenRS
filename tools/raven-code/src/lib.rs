//! Raven Code: a small editor for Rust projects, built on the `DockArea`. It opens a project from
//! disk, edits and saves its files, and builds and runs it with cargo, with the compiler's errors
//! marked in the code and listed under Problems. It is the start of an IDE for RavenRS.
//!
//! `cargo run -p raven-code -- <folder>` opens a project; with no folder it opens a copy of a small
//! sample project in a temporary folder, so the repository is never edited.
//!
//! * [`project`]: the project on disk: its files, making, renaming and deleting them, and
//!   searching them.
//! * [`cargo`]: building, running, testing and checking it, stopping it and typing into it, and
//!   reading what the compiler says.
//! * [`lsp`]: what rust-analyzer says while typing: problems, suggestions, tips, definitions.
//! * [`config`]: what is remembered between runs: the recent projects and how each was left.
//! * [`app`]: the window, wiring them to the components.
//!
//! Keys, with Cmd for Ctrl on a Mac: Ctrl+S saves the file at the front, Ctrl+O opens a folder,
//! Ctrl+B builds, F5 runs and Shift+F5 stops, Ctrl+Shift+T tests, Ctrl+Z undoes, Ctrl+F finds
//! in the file and Ctrl+H replaces, Ctrl+Shift+F searches the project, Ctrl+Space suggests,
//! F12 goes to a definition, and Ctrl+Tab goes back to the panel used before.

/// The scenarios of Raven Test Studio the tool brings, one per file of `tests/e2e/`: the Studio
/// gets them when it opens the tool with `--e2e-serve`, and the tests play them.
pub const SCENARIOS: [&str; 1] = [include_str!("../tests/e2e/abrir.raventest")];

/// The editor a test drives: the sample project in a folder of its own under the system's
/// temporary folder, emptied first, so what a scenario edits and saves is nobody else's.
#[cfg(not(target_arch = "wasm32"))]
pub fn e2e_session(name: &str) -> std::io::Result<ravenrs_e2e::E2e> {
    let folder = std::env::temp_dir().join(format!("raven-code-e2e-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&folder);
    let project = sample_in(&folder)?;
    Ok(ravenrs_e2e::E2e::new(RavenCode::new(project), ravenrs::prelude::Theme::raven_dark())
        .viewport(ravenrs::prelude::Size::new(1440.0, 900.0), 1.0))
}

pub mod app;
pub mod cargo;
pub mod config;
pub mod lsp;
pub mod project;

pub use app::RavenCode;

use std::io;
use std::path::PathBuf;

use project::Project;

pub mod versao;

/// Builds the editor on the sample project with no window, as the installer and an agent ask
/// it to. Returns what it checked.
pub fn self_test() -> Result<String, String> {
    let project = sample().map_err(|e| e.to_string())?;
    let mut ui = ravenrs::prelude::Ui::new(RavenCode::new(project), ravenrs::prelude::Theme::raven_dark());
    ui.set_viewport(ravenrs::prelude::Size::new(1440.0, 900.0), 1.0);
    for _ in 0..4 {
        ui.update();
    }
    let nodes = ui.access_tree().nodes.len();
    if nodes < 3 {
        return Err(format!("o editor abriu com {nodes} nós acessíveis"));
    }
    Ok("Autoteste: o editor abre o projeto de exemplo sem janela".to_owned())
}

/// The sample project, copied into a temporary folder the first time and opened from there.
pub fn sample() -> io::Result<Project> {
    sample_in(std::env::temp_dir().join("raven-code-sample"))
}

/// The sample project, written into `folder` and opened from there: a test gives it a folder of
/// its own, so what it edits and saves is nobody else's.
pub fn sample_in(folder: impl Into<PathBuf>) -> io::Result<Project> {
    let folder = folder.into();
    project::write_sample(&folder)?;
    Project::open(folder)
}

/// The project a command line names, or the sample with none.
pub fn from_args(mut args: impl Iterator<Item = String>) -> io::Result<Project> {
    match args.next() {
        Some(folder) => Project::open(PathBuf::from(folder)),
        None => sample(),
    }
}
