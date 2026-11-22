//! What Raven Code remembers between runs, in the user's folder of settings: the projects opened
//! last, and how each project was left (the arrangement of the window, the open files and where
//! their carets were). Nothing is written inside the projects. Tests give it a folder of their
//! own with [`Config::at`].

use std::fs;
use std::hash::{Hash, Hasher};
use std::io;
use std::path::{Path, PathBuf};

/// How many projects the list of recent ones keeps.
pub const RECENT: usize = 10;

/// The first line of a saved session, with the version of its format.
const SESSION_HEADER: &str = "raven-code session 1";

/// Where Raven Code keeps what it remembers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
    dir: PathBuf,
}

impl Config {
    /// In this folder, made when something is first written.
    pub fn at(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    /// In the user's folder of settings: `~/Library/Application Support/raven-code` on a Mac,
    /// `%APPDATA%\raven-code` on Windows, and `$XDG_CONFIG_HOME/raven-code` (or
    /// `~/.config/raven-code`) elsewhere. `None` when the system says no home.
    pub fn of_user() -> Option<Self> {
        let var = |name: &str| std::env::var_os(name).filter(|v| !v.is_empty()).map(PathBuf::from);
        let base = if cfg!(target_os = "macos") {
            var("HOME").map(|home| home.join("Library/Application Support"))
        } else if cfg!(windows) {
            var("APPDATA")
        } else {
            var("XDG_CONFIG_HOME").or_else(|| var("HOME").map(|home| home.join(".config")))
        };
        base.map(|base| Self::at(base.join("raven-code")))
    }

    /// Its folder.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    fn recent_file(&self) -> PathBuf {
        self.dir.join("recent.txt")
    }

    /// The projects opened last, the latest first.
    pub fn recent(&self) -> Vec<PathBuf> {
        fs::read_to_string(self.recent_file())
            .map(|text| text.lines().filter(|line| !line.is_empty()).map(PathBuf::from).collect())
            .unwrap_or_default()
    }

    /// Puts a project at the top of the recent ones, once, keeping ten.
    pub fn remember(&self, project: &Path) -> io::Result<()> {
        let mut recent = self.recent();
        recent.retain(|p| p != project);
        recent.insert(0, project.to_path_buf());
        recent.truncate(RECENT);
        self.write_recent(&recent)
    }

    /// Takes a project out of the recent ones, as when its folder is gone.
    pub fn forget(&self, project: &Path) -> io::Result<()> {
        let mut recent = self.recent();
        recent.retain(|p| p != project);
        self.write_recent(&recent)
    }

    fn write_recent(&self, recent: &[PathBuf]) -> io::Result<()> {
        fs::create_dir_all(&self.dir)?;
        let text: String = recent.iter().map(|p| format!("{}\n", p.display())).collect();
        fs::write(self.recent_file(), text)
    }

    /// Where the session of a project is kept: its name and a hash of its whole path, so two
    /// projects of the same name do not share one.
    fn session_file(&self, project: &Path) -> PathBuf {
        let mut hasher = std::hash::DefaultHasher::new();
        project.hash(&mut hasher);
        let name = project.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let name: String =
            name.chars().map(|c| if c.is_alphanumeric() || c == '-' { c } else { '_' }).collect();
        self.dir.join("sessions").join(format!("{name}-{:016x}.txt", hasher.finish()))
    }

    /// How a project was left, if it was.
    pub fn session(&self, project: &Path) -> Option<Session> {
        Session::from_text(&fs::read_to_string(self.session_file(project)).ok()?)
    }

    /// Keeps how a project is left.
    pub fn save_session(&self, project: &Path, session: &Session) -> io::Result<()> {
        let file = self.session_file(project);
        if let Some(parent) = file.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(file, session.to_text())
    }
}

/// How a project was left: what the window looked like and what was open in it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Session {
    /// The arrangement of the dock, as `DockLayout::to_text` writes it.
    pub layout: String,
    /// The open files, in the order of their tabs, each with where its caret was.
    pub open: Vec<(String, usize)>,
    /// The file at the front.
    pub active: Option<String>,
}

impl Session {
    /// As text: a header, the file at the front, one line per open file, then the layout.
    pub fn to_text(&self) -> String {
        let mut text = format!("{SESSION_HEADER}\n");
        if let Some(active) = &self.active {
            text.push_str(&format!("active {active}\n"));
        }
        for (path, caret) in &self.open {
            text.push_str(&format!("open {caret} {path}\n"));
        }
        text.push_str("layout\n");
        text.push_str(&self.layout);
        text
    }

    /// Reads [`to_text`](Self::to_text); `None` for another format or version.
    pub fn from_text(text: &str) -> Option<Self> {
        let mut lines = text.lines();
        if lines.next()? != SESSION_HEADER {
            return None;
        }
        let mut session = Self::default();
        for line in lines.by_ref() {
            if line == "layout" {
                break;
            } else if let Some(active) = line.strip_prefix("active ") {
                session.active = Some(active.to_owned());
            } else if let Some(open) = line.strip_prefix("open ") {
                let (caret, path) = open.split_once(' ')?;
                session.open.push((path.to_owned(), caret.parse().ok()?));
            }
        }
        session.layout = lines.collect::<Vec<_>>().join("\n");
        Some(session)
    }
}
