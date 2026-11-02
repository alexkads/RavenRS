//! The models: which fit, and their files in a folder.

use std::fmt;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use sha2::{Digest, Sha256};

use crate::Machine;

/// The memory left for the system and the other apps while a model runs.
const MEMORY_LEFT_FREE: u64 = 2_000_000_000;
/// The disk left free after a download.
const DISK_LEFT_FREE: u64 = 2_000_000_000;
const CHUNK: usize = 1 << 16;
/// What marks a file still being downloaded.
const PART: &str = "part";

/// A file of a model, pinned to one published version.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ModelFile {
    /// Its name in the folder.
    pub name: &'static str,
    /// Where it comes from, pinned to a commit.
    pub url: &'static str,
    /// Its size in bytes.
    pub bytes: u64,
    /// Its SHA-256, in lowercase hexadecimal.
    pub sha256: &'static str,
}

/// The kind of model, which decides how it runs and how to talk to it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ModelFamily {
    /// Qwen3.
    Qwen3,
    /// Qwen 2.5.
    Qwen2,
    /// Gemma 3.
    Gemma3,
    /// Llama 3.
    Llama3,
    /// Phi-3.
    Phi3,
    /// Mistral.
    Mistral,
}

/// A model an app knows how to use.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LocalModel {
    /// Its name in the folder, which never changes.
    pub id: &'static str,
    /// The name people read.
    pub name: &'static str,
    /// The license of its weights, which is not the app's.
    pub license: &'static str,
    /// How it runs.
    pub family: ModelFamily,
    /// The most memory it takes while answering, in bytes.
    pub memory: u64,
    /// How many of every hundred requests it understood when measured, if it was.
    pub understood: Option<u8>,
    /// Its files.
    pub files: &'static [ModelFile],
}

impl LocalModel {
    /// All of its files, in bytes.
    pub fn bytes(&self) -> u64 {
        self.files.iter().map(|file| file.bytes).sum()
    }

    /// Whether it can run here: enough memory for it, and room on the disk unless
    /// it is already there.
    pub fn fits(&self, machine: &Machine, installed: bool) -> bool {
        let memory = machine.memory >= self.memory.saturating_add(MEMORY_LEFT_FREE);
        let disk = installed || machine.free_disk >= self.bytes().saturating_add(DISK_LEFT_FREE);
        memory && disk
    }
}

/// Where a model stands in a folder.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ModelState {
    /// Not there.
    Absent,
    /// All its files are there, with the right sizes.
    Installed,
}

/// Why a model did not arrive. Nothing half done stays looking installed.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ModelError {
    /// The connection failed or dropped.
    Network(String),
    /// Reading or writing the folder failed.
    Disk(String),
    /// What arrived is not the published file.
    WrongFile,
    /// Someone cancelled it.
    Cancelled,
}

impl fmt::Display for ModelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Network(why) => write!(f, "the download failed: {why}"),
            Self::Disk(why) => write!(f, "the model could not be written: {why}"),
            Self::WrongFile => f.write_str("the file is not the published model"),
            Self::Cancelled => f.write_str("the download was cancelled"),
        }
    }
}

impl std::error::Error for ModelError {}

fn disk(error: std::io::Error) -> ModelError {
    ModelError::Disk(error.to_string())
}

/// A folder that holds models, each in a folder of its own.
///
/// ```no_run
/// use std::sync::atomic::AtomicBool;
/// use ravenrs_ai::{LocalModel, ModelFamily, ModelFile, ModelFolder};
///
/// const MODEL: LocalModel = LocalModel {
///     id: "tiny",
///     name: "Tiny",
///     license: "MIT",
///     family: ModelFamily::Qwen3,
///     memory: 2_000_000_000,
///     understood: None,
///     files: &[ModelFile {
///         name: "tiny.gguf",
///         url: "https://example.invalid/tiny.gguf",
///         bytes: 1024,
///         sha256: "0000000000000000000000000000000000000000000000000000000000000000",
///     }],
/// };
/// let folder = ModelFolder::new("models");
/// folder.download(&MODEL, &|received, total| println!("{received} of {total}"), &AtomicBool::new(false))?;
/// folder.remove(&MODEL)?;
/// # Ok::<(), ravenrs_ai::ModelError>(())
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelFolder {
    root: PathBuf,
}

impl ModelFolder {
    /// A folder at `root`. It is created when something is downloaded.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Where it is.
    pub fn root(&self) -> &Path {
        &self.root
    }

    fn folder_of(&self, model: &LocalModel) -> PathBuf {
        self.root.join(model.id)
    }

    /// Where a file of `model` is, or would be.
    pub fn path_of(&self, model: &LocalModel, file: &ModelFile) -> PathBuf {
        self.folder_of(model).join(file.name)
    }

    /// Where `model` stands. Quick: it compares sizes, as the hash was checked when
    /// it arrived.
    pub fn state(&self, model: &LocalModel) -> ModelState {
        let whole = model
            .files
            .iter()
            .all(|file| fs::metadata(self.path_of(model, file)).is_ok_and(|found| found.len() == file.bytes));
        if whole { ModelState::Installed } else { ModelState::Absent }
    }

    /// Downloads what is missing of `model` and installs each file only if its hash
    /// is right. It blocks until the end, so call it away from the UI thread.
    ///
    /// `progress` gets the bytes received and the total, and `cancelled` is looked
    /// at between chunks.
    ///
    /// # Errors
    ///
    /// Why it did not arrive. Files already checked stay, so trying again goes on
    /// from there, and the one being downloaded is erased.
    pub fn download(
        &self,
        model: &LocalModel,
        progress: &dyn Fn(u64, u64),
        cancelled: &AtomicBool,
    ) -> Result<(), ModelError> {
        fs::create_dir_all(self.folder_of(model)).map_err(disk)?;
        let total = model.bytes();
        let mut done = 0;
        for file in model.files {
            let path = self.path_of(model, file);
            if fs::metadata(&path).is_ok_and(|found| found.len() == file.bytes) {
                done += file.bytes;
                progress(done, total);
                continue;
            }
            let part = path.with_extension(PART);
            let result = fetch(file, &part, done, total, progress, cancelled);
            if result.is_err() {
                // Whatever it was, there is nothing to keep.
                let _ = fs::remove_file(&part);
            }
            result?;
            fs::rename(&part, &path).map_err(disk)?;
            done += file.bytes;
        }
        Ok(())
    }

    /// Removes everything of `model`, including a download left halfway.
    ///
    /// # Errors
    ///
    /// When the disk refuses. A model that is not there is not an error.
    pub fn remove(&self, model: &LocalModel) -> Result<(), ModelError> {
        match fs::remove_dir_all(self.folder_of(model)) {
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(disk(error)),
            _ => Ok(()),
        }
    }
}

/// Downloads one file to `part`, checking its hash.
fn fetch(
    file: &ModelFile,
    part: &Path,
    before: u64,
    total: u64,
    progress: &dyn Fn(u64, u64),
    cancelled: &AtomicBool,
) -> Result<(), ModelError> {
    let response = ureq::get(file.url).call().map_err(|error| ModelError::Network(error.to_string()))?;
    let mut reader = response.into_body().into_reader();
    let mut out = File::create(part).map_err(disk)?;
    let mut hash = Sha256::new();
    let mut buffer = vec![0_u8; CHUNK];
    let mut received = 0_u64;
    loop {
        if cancelled.load(Ordering::Relaxed) {
            return Err(ModelError::Cancelled);
        }
        let read = reader.read(&mut buffer).map_err(|error| ModelError::Network(error.to_string()))?;
        if read == 0 {
            break;
        }
        let chunk = buffer.get(..read).unwrap_or_default();
        hash.update(chunk);
        out.write_all(chunk).map_err(disk)?;
        received += read as u64;
        progress(before + received, total);
    }
    out.flush().map_err(disk)?;
    let found: String = hash.finalize().iter().map(|byte| format!("{byte:02x}")).collect();
    if found == file.sha256 && received == file.bytes { Ok(()) } else { Err(ModelError::WrongFile) }
}
