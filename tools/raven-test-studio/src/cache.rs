//! The Studio's own cache of runs, so a scenario that did not change, on a build that did not
//! change, is not run again: its report comes back at once, saying it came from the cache.
//!
//! A run is kept under two keys joined: the scenario's (the blocks along its path, with their
//! kind, target and text) and the build's (the size and time of the program that ran it).
//! Changing a block, or building again, changes the key, so a stale result is never reused.
//! This is the first piece of the history issue #627 plans (section 6.17).

use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use ravenrs::prelude::FlowGraph;
use serde_json::{Map, Value};

use crate::report::Report;
use crate::runner;

/// FNV-1a, a hash that stays the same between builds and machines.
fn fnv(bytes: &[u8], mut hash: u64) -> u64 {
    for b in bytes {
        hash ^= u64::from(*b);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    hash
}

/// The scenario's key: the blocks a run goes through, in order, with what each one does.
pub fn scenario_key(graph: &FlowGraph) -> String {
    let mut hash = 0xcbf2_9ce4_8422_2325;
    for id in runner::path(graph) {
        if let Some(node) = graph.find_node(&id) {
            for part in [node.id.as_str(), node.kind.as_str(), node.subtitle.as_deref().unwrap_or_default()] {
                hash = fnv(part.as_bytes(), hash);
                hash = fnv(&[0], hash);
            }
        }
    }
    format!("{hash:016x}")
}

/// The build's key: the size and the time of the program running now. `None` when it cannot
/// be read, and then nothing is cached, rather than risk a stale result.
pub fn build_key() -> Option<String> {
    let exe = std::env::current_exe().ok()?;
    let meta = std::fs::metadata(exe).ok()?;
    let at = meta.modified().ok()?.duration_since(UNIX_EPOCH).ok()?.as_secs();
    Some(format!("{}-{at}", meta.len()))
}

/// Where the cache file lives: the user's cache folder, outside every project and worktree.
pub fn default_file() -> Option<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let base = if cfg!(target_os = "macos") {
        home.map(|h| h.join("Library/Caches"))
    } else if cfg!(target_os = "windows") {
        std::env::var_os("LOCALAPPDATA").map(PathBuf::from)
    } else {
        std::env::var_os("XDG_CACHE_HOME").map(PathBuf::from).or_else(|| home.map(|h| h.join(".cache")))
    }?;
    Some(base.join("ravenrs").join("test-studio").join("runs.json"))
}

/// Runs kept on disk by key, as JSON.
#[derive(Debug)]
pub struct RunCache {
    file: PathBuf,
    entries: Map<String, Value>,
}

impl RunCache {
    /// The cache in `file`, empty when the file does not exist or cannot be read.
    pub fn at(file: &Path) -> Self {
        let entries = std::fs::read_to_string(file)
            .ok()
            .and_then(|text| serde_json::from_str::<Value>(&text).ok())
            .and_then(|v| v.as_object().cloned())
            .unwrap_or_default();
        Self { file: file.to_path_buf(), entries }
    }

    /// The key a run of `graph` on this build is kept under; `None` when the build is unknown.
    pub fn key(graph: &FlowGraph, build: Option<&str>) -> Option<String> {
        Some(format!("{}:{}", scenario_key(graph), build?))
    }

    /// The report kept under `key`, marked as coming from the cache.
    pub fn get(&self, key: &str) -> Option<Report> {
        let entry = self.entries.get(key)?;
        let mut report = Report::from_json(entry.get("report")?)?;
        report.cached_at = entry.get("at").and_then(Value::as_u64);
        Some(report)
    }

    /// Keeps `report` under `key` and writes the file.
    pub fn put(&mut self, key: &str, report: &Report) -> std::io::Result<()> {
        let at = std::time::SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
        let entry = serde_json::json!({ "at": at, "report": report.json() });
        self.entries.insert(key.to_owned(), entry);
        if let Some(dir) = self.file.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(&self.file, serde_json::to_string(&Value::Object(self.entries.clone()))?)
    }

    /// Forgets every run kept.
    pub fn clear(&mut self) -> std::io::Result<()> {
        self.entries.clear();
        match std::fs::remove_file(&self.file) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        }
    }
}
