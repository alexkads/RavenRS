//! Running `.raventest` files straight from their blocks, with no diagram: what
//! `cargo ravenrs e2e run` does, and anything else that runs scenarios without a window.

use std::path::{Path, PathBuf};
use std::time::Instant;

use ravenrs_e2e_wire::{EXTENSION, Scenario};

use crate::driver::{Driver, FilmMode, Inspection};
use crate::report::Report;
use crate::session::{Opener, Outcome, Step};
use crate::window::{Control, Window};

/// The `.raventest` files at `path`: the file itself, or every one in the folder, in name order.
pub fn files_at(path: &Path) -> Result<Vec<PathBuf>, String> {
    let mut files: Vec<PathBuf> = if path.is_dir() {
        std::fs::read_dir(path)
            .map_err(|e| format!("{}: {e}", path.display()))?
            .flatten()
            .map(|entry| entry.path())
            .filter(|p| p.extension().is_some_and(|ext| ext == EXTENSION))
            .collect()
    } else if path.is_file() {
        vec![path.to_path_buf()]
    } else {
        return Err(format!("{} não existe", path.display()));
    };
    files.sort();
    if files.is_empty() {
        return Err(format!("{} não tem arquivos .{EXTENSION}", path.display()));
    }
    Ok(files)
}

/// The scenarios in `files`, read; the first that cannot be read says its file and line.
pub fn read(files: &[PathBuf]) -> Result<Vec<Scenario>, String> {
    files
        .iter()
        .map(|file| {
            let text = std::fs::read_to_string(file).map_err(|e| format!("{}: {e}", file.display()))?;
            Scenario::parse(&text).map_err(|e| format!("{}: {e}", file.display()))
        })
        .collect()
}

/// How a run goes beyond its blocks: whether it films, and where it keeps what it makes.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RunOptions {
    /// When to keep the film of the apps it opens.
    pub film: FilmMode,
    /// Where each run is kept, a folder per scenario; films go there too.
    pub out: Option<PathBuf>,
    /// With or without the app's window, at what pace, and whether it stays at the end.
    pub window: Window,
    /// What the person watching asks for (pause, one more block, stop), when they can ask.
    pub control: Option<Control>,
}

impl RunOptions {
    /// The folder of the run called `title`, where its films go.
    pub fn folder_of(&self, title: &str) -> Option<PathBuf> {
        self.out.as_ref().map(|out| out.join(crate::report::folder_name(title)))
    }
}

/// The title of a run of `scenarios` read from `path`: the scenario's name, or how many there
/// are and where, said from where it was called.
pub fn title_of(path: &Path, scenarios: &[Scenario]) -> String {
    match scenarios {
        [one] => one.name.clone(),
        many => {
            let here = std::env::current_dir().ok();
            let shown = here.as_deref().and_then(|h| path.strip_prefix(h).ok()).unwrap_or(path);
            format!("{} cenários de {}", many.len(), shown.display())
        }
    }
}

/// Runs the scenario files at `path` (one file, or every one in a folder), opening apps with
/// `open`, and reports them all as one run under a title that names them.
pub fn run_files(path: &Path, open: Opener) -> Result<Report, String> {
    run_files_with(path, open, &RunOptions::default())
}

/// [`run_files`] as `options` say: filming, and where to keep the films.
pub fn run_files_with(path: &Path, open: Opener, options: &RunOptions) -> Result<Report, String> {
    let scenarios = read(&files_at(path)?)?;
    let title = title_of(path, &scenarios);
    Ok(run_with(&title, &scenarios, open, options))
}

/// Runs `scenarios` one after the other, each from its first block, and stops at the first
/// block that fails: the blocks after it are reported as not run. Each scenario's app ends
/// with it.
pub fn run(title: &str, scenarios: &[Scenario], open: Opener) -> Report {
    run_with(title, scenarios, open, &RunOptions::default())
}

/// [`run`] as `options` say.
pub fn run_with(title: &str, scenarios: &[Scenario], open: Opener, options: &RunOptions) -> Report {
    run_kept(title, scenarios, open, options).0
}

/// [`run_files_with`], also giving the window left open at the end, when `options` keep it
/// ([`crate::window::KeepOpen`]): print the report, then tell the person and wait on it.
pub fn run_files_kept(
    path: &Path,
    open: Opener,
    options: &RunOptions,
) -> Result<(Report, Option<Inspection>), String> {
    let scenarios = read(&files_at(path)?)?;
    let title = title_of(path, &scenarios);
    Ok(run_kept(&title, &scenarios, open, options))
}

/// [`run_with`], also giving the window left open at the end, when `options` keep it.
pub fn run_kept(
    title: &str,
    scenarios: &[Scenario],
    open: Opener,
    options: &RunOptions,
) -> (Report, Option<Inspection>) {
    let started = Instant::now();
    let blocks: Vec<_> = scenarios.iter().flat_map(|s| s.blocks.iter()).collect();
    let mut driver = Driver::new(open)
        .filming(options.film, options.folder_of(title))
        .presenting(options.window, options.control.clone());
    let mut outcomes: Vec<Outcome> = Vec::new();
    for block in &blocks {
        if !driver.turn() {
            break;
        }
        let id = format!("b{}", outcomes.len() + 1);
        let outcome = driver.step(Step::of(block), &id, block.kind.word(), &block.title, &block.line());
        let failed = outcome.error.is_some();
        outcomes.push(outcome);
        if failed {
            break;
        }
    }
    let failed = outcomes.iter().any(|o| o.error.is_some());
    driver.end(failed);
    let not_run = blocks.iter().skip(outcomes.len()).map(|b| b.title.clone()).collect();
    let report = Report {
        scenario: title.to_owned(),
        outcomes,
        total: blocks.len(),
        cached_at: None,
        duration_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
        apart: true,
        not_run,
        films: driver.films(),
        film_note: driver.film_note(),
        window: options.window.mode,
        slow_mo_ms: u64::try_from(options.window.slow_mo.as_millis()).unwrap_or(u64::MAX),
        stopped: driver.stopped(),
    };
    (report, driver.inspection())
}
