//! `e2e.toml`: the choices of an app's suite, kept next to its `Cargo.toml` (T11 of #627).
//!
//! ```toml
//! folder = "tests/e2e"   # where its .raventest files are
//! format = "junit"       # how `cargo ravenrs e2e run` reports, when --format is not given
//! out = "target/e2e"     # where each run is kept, when --out is not given
//! film = "on-failure"    # keep the film of the app a scenario failed on (never, always)
//! window = "headed"      # show the app's real window (headless, the default, shows none)
//! slow_mo = 500          # with the window: milliseconds to wait after each step
//! keep_open = "on-failure" # with the window: leave it open at the end (never, always)
//! ```
//!
//! The options of the command line come first: `--headed`, `--slow-mo` and `--keep-open` win
//! over `window`, `slow_mo` and `keep_open`. `app`, `themes` and `load` are the keys #627 plans next; they are read
//! and said to have no effect yet. Any other key, or a value of the wrong kind, is refused with
//! the file and the key, so a typo never runs a suite with choices nobody made.

use std::path::{Path, PathBuf};

use crate::report::FORMATS;

/// The file's name.
pub const FILE: &str = "e2e.toml";

/// Keys #627 plans for the file, read now and used later.
pub const PLANNED: [&str; 3] = ["app", "themes", "load"];

/// What an `e2e.toml` says.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
    /// Where the scenarios are, from the app's folder.
    pub folder: PathBuf,
    /// How to report, one of [`FORMATS`].
    pub format: Option<String>,
    /// Where to keep each run, from the app's folder.
    pub out: Option<PathBuf>,
    /// When to film the apps a run opens.
    pub film: Option<crate::driver::FilmMode>,
    /// Whether the apps show their window.
    pub window: Option<crate::window::WindowMode>,
    /// With the window, how long to wait after each step.
    pub slow_mo: Option<std::time::Duration>,
    /// With the window, whether it stays open at the end.
    pub keep_open: Option<crate::window::KeepOpen>,
    /// Planned keys the file has, which do nothing yet.
    pub planned: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            folder: PathBuf::from(ravenrs_e2e_wire::FOLDER),
            format: None,
            out: None,
            film: None,
            window: None,
            slow_mo: None,
            keep_open: None,
            planned: Vec::new(),
        }
    }
}

impl Config {
    /// The `e2e.toml` in `folder`; the defaults when there is none.
    pub fn read(folder: &Path) -> Result<Self, String> {
        let file = folder.join(FILE);
        match std::fs::read_to_string(&file) {
            Ok(text) => Self::parse(&text).map_err(|why| format!("{}: {why}", file.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(format!("{}: {e}", file.display())),
        }
    }

    /// Reads the text of an `e2e.toml`.
    pub fn parse(text: &str) -> Result<Self, String> {
        let doc: toml_edit::DocumentMut = text.parse().map_err(|e: toml_edit::TomlError| e.to_string())?;
        let mut config = Self::default();
        for (key, item) in doc.iter() {
            let as_text = || {
                item.as_str()
                    .map(str::to_owned)
                    .ok_or_else(|| format!("{key} precisa ser um texto entre aspas"))
            };
            match key {
                "folder" => config.folder = PathBuf::from(as_text()?),
                "out" => config.out = Some(PathBuf::from(as_text()?)),
                "format" => {
                    let format = as_text()?;
                    if !FORMATS.contains(&format.as_str()) {
                        return Err(format!(
                            "format não aceita {format:?}; os formatos: {}",
                            FORMATS.join(", ")
                        ));
                    }
                    config.format = Some(format);
                }
                "film" => {
                    let word = as_text()?;
                    config.film = Some(crate::driver::FilmMode::read(&word).ok_or_else(|| {
                        format!(
                            "film não aceita {word:?}; aceita: {}",
                            crate::driver::FilmMode::WORDS.join(", ")
                        )
                    })?);
                }
                "window" => {
                    let word = as_text()?;
                    config.window = Some(crate::window::WindowMode::read(&word).ok_or_else(|| {
                        format!(
                            "window não aceita {word:?}; aceita: {}",
                            crate::window::WindowMode::WORDS.join(", ")
                        )
                    })?);
                }
                "slow_mo" => {
                    let ms = item.as_integer().and_then(|ms| u64::try_from(ms).ok()).ok_or_else(|| {
                        "slow_mo precisa ser um número de milissegundos, 0 ou mais".to_owned()
                    })?;
                    config.slow_mo = Some(std::time::Duration::from_millis(ms));
                }
                "keep_open" => {
                    let word = as_text()?;
                    config.keep_open = Some(crate::window::KeepOpen::read(&word).ok_or_else(|| {
                        format!(
                            "keep_open não aceita {word:?}; aceita: {}",
                            crate::window::KeepOpen::WORDS.join(", ")
                        )
                    })?);
                }
                planned if PLANNED.contains(&planned) => config.planned.push(planned.to_owned()),
                other => {
                    return Err(format!(
                        "chave desconhecida {other:?}; as que o e2e.toml entende: folder, format, out, film, window, slow_mo, keep_open (e, ainda sem efeito, {})",
                        PLANNED.join(", ")
                    ));
                }
            }
        }
        Ok(config)
    }

    /// How the run shows the apps: what the command line says (`None` when it said nothing),
    /// over what the file says, over the defaults. A pace or a window kept open with no window
    /// is refused, as [`crate::window::Window::check`] says.
    pub fn window_with(
        &self,
        mode: Option<crate::window::WindowMode>,
        slow_mo: Option<std::time::Duration>,
        keep_open: Option<crate::window::KeepOpen>,
    ) -> Result<crate::window::Window, String> {
        let mode = mode.or(self.window).unwrap_or_default();
        let window = crate::window::Window {
            mode,
            slow_mo: slow_mo
                .or(self.slow_mo.filter(|_| mode == crate::window::WindowMode::Headed))
                .unwrap_or_default(),
            keep_open: keep_open
                .or(self.keep_open.filter(|_| mode == crate::window::WindowMode::Headed))
                .unwrap_or_default(),
        };
        window.check()?;
        Ok(window)
    }

    /// How a run shows the apps, from what a command line asked and this file: the window, its
    /// pace and whether it stays, and the control the person at the terminal steers (paused
    /// before the first block with `--step`). `Err` says which option is wrong, in its words.
    pub fn window_from(
        &self,
        ask: &crate::window::Ask,
    ) -> Result<(crate::window::Window, Option<crate::window::Control>), String> {
        use crate::window::{Control, KeepOpen, WindowMode};
        if ask.headed && ask.headless {
            return Err("escolha --headed ou --headless, não os dois".to_owned());
        }
        let mode = match (ask.headed, ask.headless) {
            (true, _) => Some(WindowMode::Headed),
            (_, true) => Some(WindowMode::Headless),
            _ => None,
        };
        let slow_mo = match &ask.slow_mo {
            None => None,
            Some(ms) => Some(std::time::Duration::from_millis(ms.trim().parse::<u64>().map_err(|_| {
                format!("--slow-mo precisa de milissegundos, como --slow-mo 500, não {ms:?}")
            })?)),
        };
        let keep_open = match &ask.keep_open {
            None => None,
            Some(word) => Some(KeepOpen::read(word).ok_or_else(|| {
                format!("--keep-open não aceita {word:?}; aceita: {}", KeepOpen::WORDS.join(", "))
            })?),
        };
        let window = self.window_with(mode, slow_mo, keep_open)?;
        if ask.step && !window.headed() {
            return Err("--step só vale com a janela: acrescente --headed".to_owned());
        }
        let control = window.headed().then(|| {
            let control = Control::new(window.slow_mo);
            if ask.step {
                control.pause();
            }
            control
        });
        Ok((window, control))
    }

    /// What to tell about the planned keys the file has, if any.
    pub fn planned_note(&self) -> Option<String> {
        (!self.planned.is_empty()).then(|| {
            let verb = if self.planned.len() == 1 { "ainda não tem" } else { "ainda não têm" };
            format!("e2e.toml: {} {verb} efeito (vem nas próximas fases da #627)", self.planned.join(", "))
        })
    }
}
