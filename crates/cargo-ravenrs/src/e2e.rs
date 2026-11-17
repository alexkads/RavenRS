//! `cargo ravenrs e2e`: the end-to-end scenarios of a RavenRS project, run with no window and
//! no Raven Test Studio, by `ravenrs-e2e-runner`, the engine the Studio uses too.

use std::path::{Path, PathBuf};

use ravenrs_e2e_runner::config::Config;
use ravenrs_e2e_runner::report::{self, FORMATS, Report};
use ravenrs_e2e_runner::{app, project, run};

/// What `cargo ravenrs e2e run` was asked.
#[derive(Clone, Debug, Default)]
pub struct RunAsk {
    /// The project's folder; the current one by default.
    pub project: Option<PathBuf>,
    /// A `.raventest` file or a folder of them; every app's `tests/e2e` by default.
    pub path: Option<PathBuf>,
    /// `text`, `json`, `jsonl`, `junit` or `html`; empty for the `e2e.toml`'s, else text.
    pub format: String,
    /// Where to keep each run; `e2e/` in the folder cargo builds the project into by default.
    pub out: Option<PathBuf>,
}

/// The exit code a run ends with: 0 when every scenario passed, 1 when one failed, 2 when
/// nothing could run (no project, no scenario, a format it does not know, an `e2e.toml` that
/// does not read). The project's `e2e.toml` gives the format and the folder to keep runs in
/// when the command line does not.
pub fn run(ask: &RunAsk) -> i32 {
    let project = match read(ask.project.as_deref()) {
        Ok(project) => project,
        Err(why) => {
            eprintln!("{why}");
            return 2;
        }
    };
    let config = match Config::read(&project.root) {
        Ok(config) => config,
        Err(why) => {
            eprintln!("{why}");
            return 2;
        }
    };
    if let Some(note) = config.planned_note() {
        eprintln!("{note}");
    }
    let broken: Vec<&str> = project.found.iter().filter_map(|f| f.config_error.as_deref()).collect();
    if !broken.is_empty() {
        eprintln!("{}", broken.join("\n"));
        return 2;
    }
    let format = if ask.format.is_empty() {
        config.format.clone().unwrap_or_else(|| "text".to_owned())
    } else {
        ask.format.clone()
    };
    if !FORMATS.contains(&format.as_str()) {
        eprintln!("--format não aceita {format:?}; os formatos: {}", FORMATS.join(", "));
        return 2;
    }
    let out = ask
        .out
        .clone()
        .or_else(|| config.out.as_ref().map(|o| project.root.join(o)))
        .unwrap_or_else(|| project.e2e_folder());
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
    report::deliver(&reports, ask.path.is_none(), &format, Some(&out));
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

/// `cargo ravenrs e2e schema report|scenario`: the JSON Schema of the report or of a scenario
/// as JSON. 0, or 2 for another name.
pub fn schema(which: &str) -> i32 {
    let schema = match which {
        "report" | "relatorio" => report::schema(),
        "scenario" | "cenario" => ravenrs_e2e_wire::scenario_schema(),
        other => {
            eprintln!("não há esquema {other:?}; os esquemas: report, scenario");
            return 2;
        }
    };
    println!("{}", serde_json::to_string_pretty(&schema).unwrap_or_default());
    0
}

/// `cargo ravenrs e2e rust <file>`: the scenario as a Rust integration test of the app it
/// opens, found in the project (`binary` says which binary when the app has more than one).
/// 0, or 2 when the file does not read or names no app of the project.
pub fn rust(file: &Path, folder: Option<&Path>, binary: Option<&str>) -> i32 {
    let made = (|| -> Result<String, String> {
        let scenarios = run::read(&[file.to_path_buf()])?;
        let scenario = scenarios.first().ok_or("o arquivo não tem cenário")?;
        let binary = match binary {
            Some(binary) => binary.to_owned(),
            None => {
                let project = read(folder)?;
                let opened = scenario
                    .blocks
                    .iter()
                    .find(|b| b.kind == ravenrs_e2e_wire::Kind::Open)
                    .map(|b| b.target.clone())
                    .ok_or("o cenário não abre nenhum app")?;
                let app = project.found.iter().find(|f| f.name == opened && f.is_app()).ok_or_else(|| {
                    format!("nenhum app do projeto se chama {opened:?}; diga o binário com --bin")
                })?;
                app.declared
                    .as_ref()
                    .map(|d| d.binary.clone())
                    .filter(|b| !b.is_empty())
                    .or_else(|| app.binaries.first().cloned())
                    .ok_or_else(|| format!("{opened} não tem binário"))?
            }
        };
        ravenrs_e2e_runner::rust::test(scenario, &binary, &file.display().to_string())
    })();
    match made {
        Ok(code) => {
            print!("{code}");
            0
        }
        Err(why) => {
            eprintln!("{why}");
            2
        }
    }
}
