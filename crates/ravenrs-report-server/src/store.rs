//! Keeps layouts, jobs, history and documents as files, so they can be backed
//! up and read with ordinary tools. Every name is validated before it touches
//! a path, and writes go through a temporary file so a crash leaves no half file.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use jiff::Timestamp;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::job::{Format, Job};

/// Whether `name` is safe as a single path component: letters, digits, `.`,
/// `_` and `-`, starting with a letter or digit.
pub fn valid_name(name: &str) -> bool {
    let mut chars = name.chars();
    name.len() <= 64
        && chars.next().is_some_and(|c| c.is_ascii_alphanumeric())
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

/// Whether a file name is safe to serve, as [`document_file_name`] writes them.
fn valid_document_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 255
        && !name.contains(['/', '\\', '\0'])
        && !name.starts_with('.')
        && name.chars().all(|c| !c.is_control())
}

/// A title as a file name, keeping accents and dropping what file systems refuse.
pub fn document_file_name(title: &str, date: &str, format: Format) -> String {
    let clean: String = title
        .chars()
        .map(|c| if c.is_control() || r#"<>:"/\|?*"#.contains(c) { ' ' } else { c })
        .take(100)
        .collect();
    let clean = clean.trim().trim_matches('.').trim();
    let clean = if clean.is_empty() { "report" } else { clean };
    let extension = match format {
        Format::Pdf => "pdf",
        Format::Xlsx => "xlsx",
        Format::Csv => "csv",
    };
    format!("{clean} {date}.{extension}")
}

/// What happened in a run.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RunStatus {
    /// Everything was delivered.
    Success,
    /// The report had no rows and the job skips empty reports.
    SkippedEmpty,
    /// Some deliveries worked and others did not, even after the retries.
    Partial,
    /// Nothing was delivered.
    Failed,
    /// The run was too late to happen (the server was down), or the previous one was still running.
    Missed,
}

/// A delivery made in a run: who received what.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Delivered {
    /// `email` or `folder`.
    pub kind: String,
    /// The recipients, or the folder.
    pub target: String,
    /// The files.
    pub files: Vec<String>,
    /// When it happened.
    pub at: Timestamp,
}

/// One run of a job, as the history keeps it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunRecord {
    /// Identifies the run within its job; later runs sort after earlier ones.
    pub id: String,
    /// The job.
    pub job: String,
    /// The layout version rendered.
    pub layout_version: Option<u32>,
    /// When the run was due.
    pub scheduled_for: Timestamp,
    /// When it started.
    pub started_at: Timestamp,
    /// When it ended.
    pub finished_at: Timestamp,
    /// Whether it was started by hand.
    pub manual: bool,
    /// The outcome.
    pub status: RunStatus,
    /// How many times it was attempted.
    pub attempts: u32,
    /// How many rows the report printed.
    pub rows: Option<usize>,
    /// What was delivered.
    pub delivered: Vec<Delivered>,
    /// Why deliveries or the render failed, in the last attempt.
    pub errors: Vec<String>,
    /// Problems in the report itself (expression errors, missing fields). The document is still produced.
    pub report_warnings: Vec<String>,
    /// The files kept on the server.
    pub documents: Vec<String>,
}

/// When a job runs next. Reset when the job changes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobState {
    /// The next run; `None` when the schedule has none left or it was not computed yet.
    pub next_run: Option<Timestamp>,
}

/// A stored layout version.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutVersion {
    /// The version number, counting from 1.
    pub version: u32,
    /// Its size in bytes.
    pub size: u64,
}

/// The server's files. Clones share the same folder.
#[derive(Clone, Debug)]
pub struct Store {
    root: Arc<PathBuf>,
    /// Makes temporary files and run ids unique within this process.
    counter: Arc<AtomicU64>,
}

impl Store {
    /// Opens the store, creating its folders.
    pub fn open(root: &Path) -> io::Result<Self> {
        for folder in ["layouts", "jobs", "runs", "documents", "tmp"] {
            fs::create_dir_all(root.join(folder))?;
        }
        Ok(Self { root: Arc::new(root.to_path_buf()), counter: Arc::new(AtomicU64::new(0)) })
    }

    fn write_atomic(&self, path: &Path, bytes: &[u8]) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let n = self.counter.fetch_add(1, Ordering::Relaxed);
        let temporary = self.root.join("tmp").join(format!("{}-{n}", std::process::id()));
        fs::write(&temporary, bytes)?;
        fs::rename(&temporary, path)
    }

    fn write_json<T: Serialize>(&self, path: &Path, value: &T) -> io::Result<()> {
        let bytes = serde_json::to_vec_pretty(value).map_err(io::Error::other)?;
        self.write_atomic(path, &bytes)
    }

    fn read_json<T: DeserializeOwned>(path: &Path) -> io::Result<Option<T>> {
        match fs::read(path) {
            Ok(bytes) => serde_json::from_slice(&bytes).map(Some).map_err(io::Error::other),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e),
        }
    }

    fn check(name: &str) -> io::Result<()> {
        if valid_name(name) {
            Ok(())
        } else {
            Err(io::Error::new(io::ErrorKind::InvalidInput, format!("'{name}' is not a valid name")))
        }
    }

    /// The names in a folder that are valid names, sorted.
    fn names_in(folder: &Path) -> io::Result<Vec<String>> {
        let mut names = Vec::new();
        let entries = match fs::read_dir(folder) {
            Ok(entries) => entries,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(names),
            Err(e) => return Err(e),
        };
        for entry in entries {
            if let Some(name) = entry?.file_name().to_str() {
                names.push(name.to_owned());
            }
        }
        names.sort();
        Ok(names)
    }

    fn layout_dir(&self, name: &str) -> PathBuf {
        self.root.join("layouts").join(name)
    }

    /// The names of the stored layouts.
    pub fn layouts(&self) -> io::Result<Vec<String>> {
        Ok(Self::names_in(&self.root.join("layouts"))?.into_iter().filter(|n| valid_name(n)).collect())
    }

    /// A layout's versions, oldest first.
    pub fn layout_versions(&self, name: &str) -> io::Result<Vec<LayoutVersion>> {
        Self::check(name)?;
        let dir = self.layout_dir(name);
        let mut versions = Vec::new();
        for file in Self::names_in(&dir)? {
            if let Some(version) = file.strip_suffix(".rvreport.json").and_then(|v| v.parse().ok()) {
                versions.push(LayoutVersion { version, size: fs::metadata(dir.join(&file))?.len() });
            }
        }
        versions.sort_by_key(|v| v.version);
        Ok(versions)
    }

    /// Saves a layout. Saving what the latest version already holds adds no
    /// version. Returns the version that holds it.
    pub fn save_layout(&self, name: &str, json: &str) -> io::Result<u32> {
        Self::check(name)?;
        if let Some((latest, current)) = self.layout(name, None)?
            && current == json
        {
            return Ok(latest);
        }
        let next = self.layout_versions(name)?.last().map_or(1, |v| v.version + 1);
        self.write_atomic(&self.layout_dir(name).join(format!("{next}.rvreport.json")), json.as_bytes())?;
        Ok(next)
    }

    /// A layout's text with its version: the latest, or the one asked for.
    pub fn layout(&self, name: &str, version: Option<u32>) -> io::Result<Option<(u32, String)>> {
        Self::check(name)?;
        let version = match version {
            Some(version) => version,
            None => match self.layout_versions(name)?.last() {
                Some(latest) => latest.version,
                None => return Ok(None),
            },
        };
        match fs::read_to_string(self.layout_dir(name).join(format!("{version}.rvreport.json"))) {
            Ok(text) => Ok(Some((version, text))),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Deletes a layout and all its versions. Returns whether it existed.
    pub fn delete_layout(&self, name: &str) -> io::Result<bool> {
        Self::check(name)?;
        let dir = self.layout_dir(name);
        if !dir.exists() {
            return Ok(false);
        }
        fs::remove_dir_all(dir)?;
        Ok(true)
    }

    fn job_path(&self, id: &str) -> PathBuf {
        self.root.join("jobs").join(format!("{id}.job.json"))
    }

    fn state_path(&self, id: &str) -> PathBuf {
        self.root.join("jobs").join(format!("{id}.state.json"))
    }

    /// The ids of the stored jobs.
    pub fn job_ids(&self) -> io::Result<Vec<String>> {
        Ok(Self::names_in(&self.root.join("jobs"))?
            .iter()
            .filter_map(|file| file.strip_suffix(".job.json"))
            .filter(|id| valid_name(id))
            .map(str::to_owned)
            .collect())
    }

    /// Saves a job. Its next run is computed again.
    pub fn save_job(&self, id: &str, job: &Job) -> io::Result<()> {
        Self::check(id)?;
        self.write_json(&self.job_path(id), job)?;
        match fs::remove_file(self.state_path(id)) {
            Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        }
    }

    /// A job by id.
    pub fn job(&self, id: &str) -> io::Result<Option<Job>> {
        Self::check(id)?;
        Self::read_json(&self.job_path(id))
    }

    /// Deletes a job, with its history and kept documents. Returns whether it existed.
    pub fn delete_job(&self, id: &str) -> io::Result<bool> {
        Self::check(id)?;
        let existed = self.job_path(id).exists();
        for file in [self.job_path(id), self.state_path(id)] {
            match fs::remove_file(file) {
                Err(e) if e.kind() != io::ErrorKind::NotFound => return Err(e),
                _ => {}
            }
        }
        for folder in ["runs", "documents"] {
            match fs::remove_dir_all(self.root.join(folder).join(id)) {
                Err(e) if e.kind() != io::ErrorKind::NotFound => return Err(e),
                _ => {}
            }
        }
        Ok(existed)
    }

    /// A job's schedule state.
    pub fn job_state(&self, id: &str) -> io::Result<JobState> {
        Self::check(id)?;
        Ok(Self::read_json(&self.state_path(id))?.unwrap_or_default())
    }

    /// Saves a job's schedule state.
    pub fn save_job_state(&self, id: &str, state: &JobState) -> io::Result<()> {
        Self::check(id)?;
        self.write_json(&self.state_path(id), state)
    }

    /// A new run id, later than the ones before it.
    pub fn new_run_id(&self, now: Timestamp) -> String {
        let n = self.counter.fetch_add(1, Ordering::Relaxed) % 1_000_000;
        format!("{}-{n:06}", now.strftime("%Y%m%dT%H%M%S"))
    }

    /// Adds a run to a job's history, dropping the oldest past `limit`.
    pub fn save_run(&self, run: &RunRecord, limit: usize) -> io::Result<()> {
        Self::check(&run.job)?;
        Self::check(&run.id)?;
        let dir = self.root.join("runs").join(&run.job);
        self.write_json(&dir.join(format!("{}.json", run.id)), run)?;
        let files = Self::names_in(&dir)?;
        for old in files.iter().take(files.len().saturating_sub(limit)) {
            fs::remove_file(dir.join(old))?;
        }
        Ok(())
    }

    /// A job's runs, newest first, at most `limit`.
    pub fn runs(&self, job: &str, limit: usize) -> io::Result<Vec<RunRecord>> {
        Self::check(job)?;
        let dir = self.root.join("runs").join(job);
        let mut runs = Vec::new();
        for file in Self::names_in(&dir)?.iter().rev().take(limit) {
            if let Some(run) = Self::read_json(&dir.join(file))? {
                runs.push(run);
            }
        }
        Ok(runs)
    }

    /// One run of a job.
    pub fn run(&self, job: &str, id: &str) -> io::Result<Option<RunRecord>> {
        Self::check(job)?;
        Self::check(id)?;
        Self::read_json(&self.root.join("runs").join(job).join(format!("{id}.json")))
    }

    /// Keeps a run's file on the server.
    pub fn save_document(&self, job: &str, run: &str, file: &str, bytes: &[u8]) -> io::Result<()> {
        Self::check(job)?;
        Self::check(run)?;
        if !valid_document_name(file) {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "invalid file name"));
        }
        self.write_atomic(&self.root.join("documents").join(job).join(run).join(file), bytes)
    }

    /// A kept file.
    pub fn document(&self, job: &str, run: &str, file: &str) -> io::Result<Option<Vec<u8>>> {
        Self::check(job)?;
        Self::check(run)?;
        if !valid_document_name(file) {
            return Ok(None);
        }
        match fs::read(self.root.join("documents").join(job).join(run).join(file)) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Deletes a job's kept files older than `cutoff`. Returns how many runs lost theirs.
    pub fn purge_documents(&self, job: &str, cutoff: Timestamp) -> io::Result<usize> {
        Self::check(job)?;
        let dir = self.root.join("documents").join(job);
        let mut purged = 0;
        for run in Self::names_in(&dir)? {
            let Ok(record) =
                Self::read_json::<RunRecord>(&self.root.join("runs").join(job).join(format!("{run}.json")))
            else {
                continue;
            };
            // A run the history no longer has is older than anything it keeps.
            if record.is_none_or(|r| r.started_at < cutoff) {
                fs::remove_dir_all(dir.join(run))?;
                purged += 1;
            }
        }
        Ok(purged)
    }
}

/// A plain record of what a job's file listing returns over the API.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobSummary {
    /// The id.
    pub id: String,
    /// The job.
    pub job: Job,
    /// Its next run, when known.
    pub next_run: Option<Timestamp>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn open() -> Result<(tempfile::TempDir, Store), io::Error> {
        let dir = tempfile::tempdir()?;
        let store = Store::open(dir.path())?;
        Ok((dir, store))
    }

    #[test]
    fn names_that_could_leave_a_folder_are_refused() {
        for bad in ["", "..", ".", "../x", "a/b", "a\\b", ".hidden", "a\0", "a b", &"x".repeat(65)] {
            assert!(!valid_name(bad), "{bad:?}");
        }
        for good in ["a", "sales", "Sales-2026_v1.2", "20261101T120000-000001"] {
            assert!(valid_name(good), "{good}");
        }
    }

    #[test]
    fn every_entry_point_refuses_a_bad_name() -> io::Result<()> {
        let (_dir, store) = open()?;
        assert!(store.save_layout("../x", "{}").is_err());
        assert!(store.layout("../x", None).is_err());
        assert!(store.delete_layout("..").is_err());
        assert!(store.layout_versions("../..").is_err());
        assert!(store.job("../x").is_err());
        assert!(store.delete_job("../x").is_err());
        assert!(store.runs("../x", 1).is_err());
        assert!(store.run("x", "../y").is_err());
        assert!(store.document("x", "../y", "z").is_err());
        assert_eq!(store.document("x", "y", "../../secret")?, None);
        assert!(store.save_document("x", "y", "../z", b"").is_err());
        Ok(())
    }

    #[test]
    fn layouts_keep_versions_and_skip_identical_saves() -> io::Result<()> {
        let (_dir, store) = open()?;
        assert_eq!(store.layout("a", None)?, None);
        assert_eq!(store.save_layout("a", "one")?, 1);
        assert_eq!(store.save_layout("a", "one")?, 1);
        assert_eq!(store.save_layout("a", "two")?, 2);
        assert_eq!(store.layout("a", None)?, Some((2, "two".to_owned())));
        assert_eq!(store.layout("a", Some(1))?, Some((1, "one".to_owned())));
        assert_eq!(store.layout("a", Some(9))?, None);
        assert_eq!(store.layouts()?, ["a"]);
        assert_eq!(store.layout_versions("a")?.len(), 2);
        assert!(store.delete_layout("a")?);
        assert!(!store.delete_layout("a")?);
        Ok(())
    }

    #[test]
    fn version_ten_comes_after_nine() -> io::Result<()> {
        let (_dir, store) = open()?;
        for n in 1..=11 {
            store.save_layout("a", &n.to_string())?;
        }
        assert_eq!(store.layout("a", None)?, Some((11, "11".to_owned())));
        Ok(())
    }

    #[test]
    fn file_names_drop_what_file_systems_refuse() {
        assert_eq!(
            document_file_name("Vendas: Sul/Norte", "2026-11-01", Format::Pdf),
            "Vendas  Sul Norte 2026-11-01.pdf"
        );
        assert_eq!(document_file_name("Relatório", "2026-11-01", Format::Xlsx), "Relatório 2026-11-01.xlsx");
        assert_eq!(document_file_name("../..", "d", Format::Csv), "report d.csv");
    }
}
