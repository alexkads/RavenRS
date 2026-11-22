//! The project on disk: what is in it, and reading and writing its files. Nothing here knows
//! about the screen, so it is tested with a folder and no window.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// A file or a folder of the project, by its path from the project's folder with `/` between the
/// parts, the way the compiler names files.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    /// From the project's folder, such as `src/main.rs`.
    pub path: String,
    /// The last part of the path.
    pub name: String,
    /// What a folder holds, folders first, each in the order of its name. Empty for a file.
    pub children: Vec<Entry>,
    /// Whether it is a folder.
    pub folder: bool,
}

/// A Rust project in a folder.
#[derive(Clone, Debug)]
pub struct Project {
    root: PathBuf,
}

/// What is left out of the tree: what the build makes, and what is hidden.
fn skipped(name: &str) -> bool {
    name == "target" || name.starts_with('.')
}

impl Project {
    /// The project in `root`, which has to be a folder.
    pub fn open(root: impl Into<PathBuf>) -> io::Result<Self> {
        let root = root.into();
        if !root.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("{} is not a folder", root.display()),
            ));
        }
        Ok(Self { root })
    }

    /// Its folder.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Its name: the name of its folder.
    pub fn name(&self) -> String {
        self.root.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default()
    }

    /// What is in it, without `target` or what is hidden.
    pub fn tree(&self) -> io::Result<Vec<Entry>> {
        read_folder(&self.root, "")
    }

    /// The text of one of its files.
    pub fn read(&self, path: &str) -> io::Result<String> {
        fs::read_to_string(self.file(path)?)
    }

    /// Writes one of its files.
    pub fn write(&self, path: &str, text: &str) -> io::Result<()> {
        fs::write(self.file(path)?, text)
    }

    /// Makes an empty file. Refused when something is there already.
    pub fn create_file(&self, path: &str) -> io::Result<()> {
        let file = self.file(path)?;
        if let Some(parent) = file.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::OpenOptions::new().write(true).create_new(true).open(file).map(drop)
    }

    /// Makes a folder. Refused when something is there already.
    pub fn create_folder(&self, path: &str) -> io::Result<()> {
        let folder = self.file(path)?;
        if folder.exists() {
            return Err(io::Error::new(io::ErrorKind::AlreadyExists, format!("{path} already exists")));
        }
        fs::create_dir_all(folder)
    }

    /// Renames or moves a file or a folder. Refused when something is at `to` already.
    pub fn rename(&self, from: &str, to: &str) -> io::Result<()> {
        let (from_at, to_at) = (self.file(from)?, self.file(to)?);
        if to_at.exists() {
            return Err(io::Error::new(io::ErrorKind::AlreadyExists, format!("{to} already exists")));
        }
        if let Some(parent) = to_at.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::rename(from_at, to_at)
    }

    /// Deletes a file, or a folder with all it holds. There is no way back.
    pub fn remove(&self, path: &str) -> io::Result<()> {
        let at = self.file(path)?;
        if at.is_dir() { fs::remove_dir_all(at) } else { fs::remove_file(at) }
    }

    /// Where `query` is in the text files of the project (not in `target` or what is hidden),
    /// line by line, at most [`MOST_HITS`]. With `case`, only with the same capitals.
    pub fn search(&self, query: &str, case: bool) -> Vec<Hit> {
        let mut hits = Vec::new();
        if query.is_empty() {
            return hits;
        }
        let folded = |text: &str| if case { text.to_owned() } else { text.to_lowercase() };
        let wanted = folded(query);
        let mut files = Vec::new();
        collect_files(&self.tree().unwrap_or_default(), &mut files);
        for path in files {
            let Ok(text) = self.read(&path) else { continue };
            if text.contains('\0') {
                continue;
            }
            for (n, line) in text.lines().enumerate() {
                if let Some(at) = folded(line).find(&wanted) {
                    let column = line.get(..at).map_or(at, |before| before.chars().count()) + 1;
                    hits.push(Hit { file: path.clone(), line: n + 1, column, text: line.trim().to_owned() });
                    if hits.len() >= MOST_HITS {
                        return hits;
                    }
                }
            }
        }
        hits
    }

    /// A number that changes when a file or a folder of the project comes, goes, grows or is
    /// written: what a watcher compares.
    pub fn stamp(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::hash::DefaultHasher::new();
        let mut files = Vec::new();
        let tree = self.tree().unwrap_or_default();
        collect_all(&tree, &mut files);
        for path in files {
            path.hash(&mut hasher);
            if let Ok(meta) = self.file(&path).and_then(fs::metadata) {
                meta.len().hash(&mut hasher);
                meta.modified().ok().hash(&mut hasher);
            }
        }
        hasher.finish()
    }

    /// Where a path of the project is on disk. A path that would leave the project is refused.
    fn file(&self, path: &str) -> io::Result<PathBuf> {
        let mut at = self.root.clone();
        for part in path.split('/') {
            if part.is_empty() || part == "." || part == ".." {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("{path} is not in the project"),
                ));
            }
            at.push(part);
        }
        Ok(at)
    }
}

/// The most a [`Project::search`] lists.
pub const MOST_HITS: usize = 2000;

/// A line of the project where a search found what it looked for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hit {
    /// The file, from the project's folder.
    pub file: String,
    /// From 1.
    pub line: usize,
    /// Where it starts on the line, from 1, in characters.
    pub column: usize,
    /// The line, without the spaces around it.
    pub text: String,
}

/// Why a name typed for a file or a folder cannot be used, if it cannot: empty, `.` or `..`,
/// or with a separator in it.
pub fn bad_name(name: &str) -> Option<&'static str> {
    if name.trim().is_empty() {
        Some("The name is empty")
    } else if name == "." || name == ".." {
        Some("That name is taken by the system")
    } else if name.contains(['/', '\\']) {
        Some("A name cannot have / or \\ in it")
    } else {
        None
    }
}

/// Every file of a tree, by path.
pub fn collect_files(entries: &[Entry], files: &mut Vec<String>) {
    for entry in entries {
        if entry.folder {
            collect_files(&entry.children, files);
        } else {
            files.push(entry.path.clone());
        }
    }
}

fn collect_all(entries: &[Entry], all: &mut Vec<String>) {
    for entry in entries {
        all.push(entry.path.clone());
        collect_all(&entry.children, all);
    }
}

fn read_folder(folder: &Path, prefix: &str) -> io::Result<Vec<Entry>> {
    let mut entries = Vec::new();
    for item in fs::read_dir(folder)? {
        let item = item?;
        let name = item.file_name().to_string_lossy().into_owned();
        if skipped(&name) {
            continue;
        }
        let path = if prefix.is_empty() { name.clone() } else { format!("{prefix}/{name}") };
        let folder = item.file_type()?.is_dir();
        let children = if folder { read_folder(&item.path(), &path)? } else { Vec::new() };
        entries.push(Entry { path, name, children, folder });
    }
    entries.sort_by(|a, b| b.folder.cmp(&a.folder).then_with(|| a.name.cmp(&b.name)));
    Ok(entries)
}

/// The project Raven Code opens when it is given none: a crate that prints a line. It has a
/// `[workspace]` of its own, so that cargo does not take it for part of a workspace around it.
pub const SAMPLE: [(&str, &str); 3] = [
    ("Cargo.toml", "[package]\nname = \"hello\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[workspace]\n"),
    (
        "src/main.rs",
        "mod greeting;\n\nfn main() {\n    println!(\"{}\", greeting::hello(\"Raven Code\"));\n}\n",
    ),
    (
        "src/greeting.rs",
        "/// A greeting for `name`.\npub fn hello(name: &str) -> String {\n    format!(\"Hello from {name}!\")\n}\n",
    ),
];

/// Writes the sample project into `folder`, making it if it is not there, and leaves alone what
/// is already there, so a second start keeps what was edited.
pub fn write_sample(folder: &Path) -> io::Result<()> {
    for (path, text) in SAMPLE {
        let file = folder.join(path);
        if file.exists() {
            continue;
        }
        if let Some(parent) = file.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(file, text)?;
    }
    Ok(())
}
