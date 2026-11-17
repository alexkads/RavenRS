//! [`Driver`]: the one way a run drives an app, block by block, for every tool that runs
//! scenarios: `cargo ravenrs e2e`, which runs files straight from their blocks, and Raven Test
//! Studio, which runs the blocks of its diagram one at a time. It opens the app a block names,
//! runs each step, times it, asks for the screen when it fails, and films the app when asked
//! (6.4 of #627).

use std::path::{Path, PathBuf};
use std::time::Instant;

use ravenrs_e2e_wire::{Command, codes};

use crate::session::{self, Failure, Opener, Outcome, Session, Step};

/// When a run keeps the film of the app it drives.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FilmMode {
    /// Never films.
    #[default]
    Never,
    /// Films each app, and keeps only the film of the one a block failed on.
    OnFailure,
    /// Films each app, and keeps every film.
    Always,
}

impl FilmMode {
    /// The words `--film` and `e2e.toml` take.
    pub const WORDS: [&'static str; 3] = ["never", "on-failure", "always"];

    /// The mode a word says, in English or in Portuguese.
    pub fn read(word: &str) -> Option<Self> {
        Some(match word {
            "never" | "nunca" => Self::Never,
            "on-failure" | "na-falha" => Self::OnFailure,
            "always" | "sempre" => Self::Always,
            _ => return None,
        })
    }
}

/// The film being made of the app open now, and the ones finished.
#[derive(Debug, Default)]
struct Films {
    mode: FilmMode,
    folder: Option<PathBuf>,
    made: usize,
    current: Option<PathBuf>,
    /// Each film finished, and whether a block failed while it was filmed.
    done: Vec<(PathBuf, bool)>,
    note: Option<String>,
}

/// Drives the apps of a run. See the [module](self).
#[derive(Debug)]
pub struct Driver {
    session: Option<Session>,
    open: Opener,
    films: Films,
    ended: bool,
}

impl Driver {
    /// A driver that opens apps with `open`, filming nothing.
    pub fn new(open: Opener) -> Self {
        Self { session: None, open, films: Films::default(), ended: false }
    }

    /// Films each app it opens, keeping the films in `folder` as `filme-1.ravenmovie`,
    /// `filme-2.ravenmovie` and so on, as `mode` says. With no folder, nothing is filmed.
    pub fn filming(mut self, mode: FilmMode, folder: Option<PathBuf>) -> Self {
        self.films.mode = mode;
        self.films.folder = folder;
        self
    }

    /// Runs `step`, or says why it cannot run, and reports it under `id`, `kind`, `title` and
    /// `line`, as the block that stands for it is written.
    pub fn step(
        &mut self,
        step: Result<Step, String>,
        id: &str,
        kind: &str,
        title: &str,
        line: &str,
    ) -> Outcome {
        let started = Instant::now();
        let opening = match &step {
            Ok(Step::Open(name)) => Some(name.clone()),
            _ => None,
        };
        if opening.is_some() {
            // The film of the app a previous Open block started ends before that app does.
            self.stop_film(false);
        }
        let result = step
            .map_err(|m| Box::new(Failure::plain(m)))
            .and_then(|step| session::perform(&mut self.session, &step, self.open));
        if let (Ok(()), Some(name)) = (&result, &opening) {
            self.start_film(name);
        }
        let ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        let failed = result.is_err();
        let f = result.err().map(|f| *f).unwrap_or_default();
        // The failure brings the screen as it was; when it did not, the app is asked for it.
        let screen = match &f.screen {
            Some(screen) => Some(screen.clone()),
            None if failed => self.session.as_mut().and_then(Session::screen),
            None => None,
        };
        Outcome {
            id: id.to_owned(),
            kind: kind.to_owned(),
            title: title.to_owned(),
            line: line.to_owned(),
            error: (!f.message.is_empty()).then_some(f.message),
            code: f.code,
            meaning: f.meaning,
            detail: f.detail,
            fix: f.fix,
            close: f.close,
            screen,
            ms,
        }
    }

    /// Ends the run: keeps the film of the app still open (as the one that failed when
    /// `failed`), lets the app go, and, filming on failure, drops the films of apps where
    /// nothing failed. Ending twice does nothing.
    pub fn end(&mut self, failed: bool) {
        if self.ended {
            return;
        }
        self.ended = true;
        self.stop_film(failed);
        self.session = None;
        if self.films.mode == FilmMode::OnFailure {
            self.films.done.retain(|(path, failed)| {
                if !failed {
                    // A film nobody keeps only takes room.
                    let _ = std::fs::remove_file(path);
                }
                *failed
            });
        }
    }

    /// The films kept, once the run [ended](Self::end).
    pub fn films(&self) -> Vec<String> {
        self.films.done.iter().map(|(path, _)| path.display().to_string()).collect()
    }

    /// Why an app was not filmed, when it was asked to be.
    pub fn film_note(&self) -> Option<String> {
        self.films.note.clone()
    }

    fn start_film(&mut self, app: &str) {
        let (Some(folder), true) = (&self.films.folder, self.films.mode != FilmMode::Never) else {
            return;
        };
        let Some(session) = self.session.as_mut() else {
            return;
        };
        // The app may run from another project. Resolve the caller's path before it crosses
        // the channel; canonicalize would require the output folder to exist already.
        let path = match std::path::absolute(folder.join(format!("filme-{}.ravenmovie", self.films.made + 1)))
        {
            Ok(path) => path,
            Err(error) => {
                self.films.note = Some(format!("o {app} não começou a filmar: {error}"));
                return;
            }
        };
        match session.run(&Command::Film { path: path.display().to_string() }) {
            Ok(_) => {
                self.films.made += 1;
                self.films.current = Some(path);
            }
            Err(f) if f.code == codes::NO_FILM => {
                self.films.note = Some(format!(
                    "o {app} não filma: ele precisa ser servido com ravenrs_e2e_wire::serve_filmed e ravenrs_movie::ChannelFilm"
                ));
            }
            Err(f) => self.films.note = Some(format!("o {app} não começou a filmar: {f}")),
        }
    }

    fn stop_film(&mut self, failed: bool) {
        let Some(path) = self.films.current.take() else {
            return;
        };
        let Some(session) = self.session.as_mut() else {
            return;
        };
        match session.run(&Command::FilmStop) {
            Ok(_) if Path::new(&path).exists() => self.films.done.push((path, failed)),
            Ok(_) => self.films.note = Some(format!("o filme não ficou em {}", path.display())),
            Err(f) => self.films.note = Some(format!("o filme não foi guardado: {f}")),
        }
    }
}

impl Drop for Driver {
    fn drop(&mut self) {
        // A run dropped halfway still lets its app go; its films stay as they are.
        self.session = None;
    }
}
