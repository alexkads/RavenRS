//! `e2e.toml`: the choices of an app's suite, kept next to its `Cargo.toml` (T11 of #627).
//!
//! ```toml
//! folder = "tests/e2e"   # where its .raventest files are
//! format = "junit"       # how `cargo ravenrs e2e run` reports, when --format is not given
//! out = "target/e2e"     # where each run is kept, when --out is not given
//! film = "on-failure"    # keep the film of the app a scenario failed on (never, always)
//! ```
//!
//! `app`, `window`, `themes` and `load` are the keys #627 plans next; they are read
//! and said to have no effect yet. Any other key, or a value of the wrong kind, is refused with
//! the file and the key, so a typo never runs a suite with choices nobody made.

use std::path::{Path, PathBuf};

use crate::report::FORMATS;

/// The file's name.
pub const FILE: &str = "e2e.toml";

/// Keys #627 plans for the file, read now and used later.
pub const PLANNED: [&str; 4] = ["app", "window", "themes", "load"];

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
                planned if PLANNED.contains(&planned) => config.planned.push(planned.to_owned()),
                other => {
                    return Err(format!(
                        "chave desconhecida {other:?}; as que o e2e.toml entende: folder, format, out, film (e, ainda sem efeito, {})",
                        PLANNED.join(", ")
                    ));
                }
            }
        }
        Ok(config)
    }

    /// What to tell about the planned keys the file has, if any.
    pub fn planned_note(&self) -> Option<String> {
        (!self.planned.is_empty()).then(|| {
            let verb = if self.planned.len() == 1 { "ainda não tem" } else { "ainda não têm" };
            format!("e2e.toml: {} {verb} efeito (vem nas próximas fases da #627)", self.planned.join(", "))
        })
    }
}
