//! `cargo ravenrs e2e`: the end-to-end scenarios of a RavenRS project, run with no window and
//! no Raven Test Studio, by `ravenrs-e2e-runner`, the engine the Studio uses too.

use std::path::{Path, PathBuf};

use ravenrs_e2e_runner::report::{self, FORMATS, Report};
use ravenrs_e2e_runner::{app, project, run};

/// What `cargo ravenrs e2e run` was asked.
#[derive(Clone, Debug, Default)]
pub struct RunAsk {
    /// The project's folder; the current one by default.
    pub project: Option<PathBuf>,
    /// A `.raventest` file or a folder of them; every app's `tests/e2e` by default.
    pub path: Option<PathBuf>,
    /// `text`, `json`, `jsonl` or `junit`.
    pub format: String,
    /// Where to keep each run; `e2e/` in the folder cargo builds the project into by default.
    pub out: Option<PathBuf>,
}

/// The exit code a run ends with: 0 when every scenario passed, 1 when one failed, 2 when
/// nothing could run (no project, no scenario, a format it does not know).
pub fn run(ask: &RunAsk) -> i32 {
    if !FORMATS.contains(&ask.format.as_str()) {
        eprintln!("--format não aceita {:?}; os formatos: {}", ask.format, FORMATS.join(", "));
        return 2;
    }
    let project = match read(ask.project.as_deref()) {
        Ok(project) => project,
        Err(why) => {
            eprintln!("{why}");
            return 2;
        }
    };
    let out = ask.out.clone().unwrap_or_else(|| project.e2e_folder());
    let folders = match &ask.path {
        Some(path) => vec![path.clone()],
        None => project.scenario_folders(),
    };
    if folders.is_empty() {
        eprintln!("nenhum app do projeto tem o canal de teste e cenários em tests/e2e:\n{}", project.text());
        return 2;
    }
    let mut worst = 0;
    let mut reports: Vec<Report> = Vec::new();
    for folder in &folders {
        match run::run_files(folder, app::open) {
            Ok(report) => {
                worst = worst.max(report.exit_code());
                reports.push(report);
            }
            Err(why) => {
                eprintln!("{why}");
                worst = 2;
            }
        }
    }
    report::deliver(&reports, ask.path.is_none(), &ask.format, Some(&out));
    worst
}

/// `cargo ravenrs e2e list`: the project's RavenRS packages, which are apps, whether each
/// speaks the test channel, its scenarios and the screens it builds. 0, or 2 with no project.
pub fn list(folder: Option<&Path>, json: bool) -> i32 {
    match read(folder) {
        Ok(project) => {
            if json {
                println!("{}", serde_json::to_string_pretty(&project.json()).unwrap_or_default());
            } else {
                println!("{}", project.text());
            }
            0
        }
        Err(why) => {
            eprintln!("{why}");
            2
        }
    }
}

/// The project at `folder` (the current one by default), with its apps made known, so a
/// scenario opens them from this code before any installed copy.
fn read(folder: Option<&Path>) -> Result<project::Project, String> {
    let project = project::read(folder.unwrap_or(Path::new(".")))?;
    if project.found.is_empty() {
        return Err(format!("{}\nnenhum pacote feito com RavenRS aqui", project.text()));
    }
    project.learn_apps();
    Ok(project)
}
