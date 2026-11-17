//! Running `.raventest` files straight from their blocks, with no diagram: what
//! `cargo ravenrs e2e run` does, and anything else that runs scenarios without a window.

use std::path::{Path, PathBuf};
use std::time::Instant;

use ravenrs_e2e_wire::{EXTENSION, Scenario};

use crate::report::Report;
use crate::session::{self, Failure, Opener, Outcome, Session, Step};

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

/// Runs the scenario files at `path` (one file, or every one in a folder), opening apps with
/// `open`, and reports them all as one run under a title that names them.
pub fn run_files(path: &Path, open: Opener) -> Result<Report, String> {
    let scenarios = read(&files_at(path)?)?;
    let title = match scenarios.as_slice() {
        [one] => one.name.clone(),
        many => {
            // Said from where it was called, which is shorter and the same on every machine.
            let here = std::env::current_dir().ok();
            let shown = here.as_deref().and_then(|h| path.strip_prefix(h).ok()).unwrap_or(path);
            format!("{} cenários de {}", many.len(), shown.display())
        }
    };
    Ok(run(&title, &scenarios, open))
}

/// Runs `scenarios` one after the other, each from its first block, and stops at the first
/// block that fails: the blocks after it are reported as not run. Each scenario's app ends
/// with it.
pub fn run(title: &str, scenarios: &[Scenario], open: Opener) -> Report {
    let started = Instant::now();
    let blocks: Vec<_> = scenarios.iter().flat_map(|s| s.blocks.iter()).collect();
    let mut outcomes = Vec::new();
    let mut session: Option<Session> = None;
    for block in &blocks {
        let begun = Instant::now();
        let result = Step::of(block)
            .map_err(|m| Box::new(Failure::plain(m)))
            .and_then(|step| session::perform(&mut session, &step, open));
        let ms = u64::try_from(begun.elapsed().as_millis()).unwrap_or(u64::MAX);
        let failed = result.is_err();
        let f = result.err().map(|f| *f).unwrap_or_default();
        let screen = match &f.screen {
            Some(screen) => Some(screen.clone()),
            None if failed => session.as_mut().and_then(Session::screen),
            None => None,
        };
        outcomes.push(Outcome {
            id: format!("b{}", outcomes.len() + 1),
            kind: block.kind.word().to_owned(),
            title: block.title.clone(),
            line: block.line(),
            error: (!f.message.is_empty()).then_some(f.message),
            code: f.code,
            meaning: f.meaning,
            detail: f.detail,
            fix: f.fix,
            close: f.close,
            screen,
            ms,
        });
        if failed {
            break;
        }
    }
    drop(session);
    let not_run = blocks.iter().skip(outcomes.len()).map(|b| b.title.clone()).collect();
    Report {
        scenario: title.to_owned(),
        outcomes,
        total: blocks.len(),
        cached_at: None,
        duration_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
        apart: true,
        not_run,
    }
}
