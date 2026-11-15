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
