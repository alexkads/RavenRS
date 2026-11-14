//! Where compiled crates are kept: files named by the SHA-256 of their content, results that list
//! them, manifests that say which inputs lead to which result, and a log of every request.
//!
//! Every write goes to a temporary file first and is renamed into place, so a build that is stopped
//! halfway leaves nothing a later one could read as whole. Two builds writing the same thing write
//! the same bytes, and the last rename wins.

use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use sha2::{Digest, Sha256};

/// The cache on disk.
#[derive(Debug, Clone)]
pub(crate) struct Store {
    pub root: PathBuf,
}

/// One file a compilation produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Output {
    /// `dep-info`, `metadata` or `link`.
    pub emit: String,
    pub sha: String,
    pub size: u64,
    /// Its name inside the output directory.
    pub name: String,
}

/// What was kept of one compilation.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Stored {
    pub outputs: Vec<Output>,
    /// What rustc wrote to stderr, in order: `D <line>` for a diagnostic as it was, `A <emit> <name>`
    /// where it reported an output file.
    pub stderr: Vec<String>,
}

/// One way a command line was compiled before: the inputs it read, and the result they gave.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Entry {
    pub result: String,
    /// The real roots the result embeds, when it embeds them; it only serves under the same ones.
    pub pinned: Option<String>,
    /// Source files: content hash and path, with markers for the roots.
    pub files: Vec<(String, String)>,
    /// Environment variables rustc said it read: name, and the value with markers, or `None` if unset.
    pub env: Vec<(String, Option<String>)>,
}

/// SHA-256 of some bytes, in hex.
pub(crate) fn sha(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// SHA-256 of a file, read in pieces.
pub(crate) fn sha_of_file(path: &Path) -> io::Result<String> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; 1 << 16];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hex(&hasher.finalize()))
}

fn stamp(path: &Path) -> io::Result<(u64, u128)> {
    let meta = fs::metadata(path)?;
    let modified = meta.modified()?.duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    Ok((meta.len(), modified))
}

impl Store {
    /// The store named by `RAVENRS_XCACHE_DIR`, or `~/.cache/ravenrs/xcache`.
    pub(crate) fn open() -> io::Result<Store> {
        let root = match std::env::var_os("RAVENRS_XCACHE_DIR") {
            Some(dir) if !dir.is_empty() => PathBuf::from(dir),
            _ => {
                let home = std::env::var_os("HOME")
                    .or_else(|| std::env::var_os("USERPROFILE"))
                    .ok_or_else(|| io::Error::other("no home directory; set RAVENRS_XCACHE_DIR"))?;
                PathBuf::from(home).join(".cache").join("ravenrs").join("xcache")
            }
        };
        fs::create_dir_all(&root)?;
        Ok(Store { root })
    }

    fn fanned(&self, kind: &str, key: &str) -> PathBuf {
        self.root.join(kind).join(&key[..key.len().min(2)]).join(key)
    }

    /// Writes a file whole or not at all.
    pub(crate) fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
        if let Some(folder) = path.parent() {
            fs::create_dir_all(folder)?;
        }
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
        let temporary = path.with_extension(format!("tmp-{}-{nanos}", std::process::id()));
        let written = fs::File::create(&temporary).and_then(|mut file| file.write_all(bytes));
        match written.and_then(|()| fs::rename(&temporary, path)) {
            Ok(()) => Ok(()),
            Err(error) => {
                let _ = fs::remove_file(&temporary);
                Err(error)
            }
        }
    }

    /// Where the file with this content hash is.
    pub(crate) fn blob(&self, sha: &str) -> PathBuf {
        self.fanned("blobs", sha)
    }

    /// Keeps a file under its content hash, copy-on-write where the file system can.
    pub(crate) fn keep_file(&self, from: &Path, sha: &str) -> io::Result<()> {
        let to = self.blob(sha);
        if to.is_file() {
            return Ok(());
        }
        if let Some(folder) = to.parent() {
            fs::create_dir_all(folder)?;
        }
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
        let temporary = to.with_extension(format!("tmp-{}-{nanos}", std::process::id()));
        match fs::copy(from, &temporary).and_then(|_| fs::rename(&temporary, &to)) {
            Ok(()) => Ok(()),
            Err(error) => {
                let _ = fs::remove_file(&temporary);
                Err(error)
            }
        }
    }

    /// Keeps some bytes under their content hash.
    pub(crate) fn keep_bytes(&self, bytes: &[u8]) -> io::Result<String> {
        let sha = sha(bytes);
        let to = self.blob(&sha);
        if !to.is_file() {
            Self::write_atomic(&to, bytes)?;
        }
        Ok(sha)
    }

    /// The identity of a file a compilation depends on: what this cache recorded for it when it
    /// wrote or restored it, as long as its size and time are the same, else its content hash.
    pub(crate) fn identity(&self, path: &Path) -> io::Result<String> {
        let (size, modified) = stamp(path)?;
        let record = self.fanned("identities", &sha(path.to_string_lossy().as_bytes()));
        if let Ok(text) = fs::read_to_string(&record) {
            let mut parts = text.split_whitespace();
            let known = (parts.next(), parts.next(), parts.next());
            if let (Some(known_size), Some(known_time), Some(identity)) = known
                && known_size == size.to_string()
                && known_time == modified.to_string()
            {
                return Ok(identity.to_owned());
            }
        }
        let identity = sha_of_file(path)?;
        let _ = Self::write_atomic(&record, format!("{size} {modified} {identity}").as_bytes());
        Ok(identity)
    }

    /// Records the identity of a file this cache just wrote, so nothing has to read it again.
    pub(crate) fn remember(&self, path: &Path, identity: &str) {
        if let Ok((size, modified)) = stamp(path) {
            let record = self.fanned("identities", &sha(path.to_string_lossy().as_bytes()));
            let _ = Self::write_atomic(&record, format!("{size} {modified} {identity}").as_bytes());
        }
    }

    /// The ways this command line was compiled before, newest first.
    pub(crate) fn entries(&self, request: &str) -> Vec<Entry> {
        let Ok(text) = fs::read_to_string(self.fanned("manifests", request)) else {
            return Vec::new();
        };
        let mut entries: Vec<Entry> = Vec::new();
        for line in text.lines() {
            let (tag, rest) = line.split_once(' ').unwrap_or((line, ""));
            if tag == "R" {
                entries.push(Entry { result: rest.to_owned(), ..Entry::default() });
                continue;
            }
            let Some(entry) = entries.last_mut() else { continue };
            match tag {
                "P" => entry.pinned = Some(rest.to_owned()),
                "F" => {
                    if let Some((sha, path)) = rest.split_once(' ') {
                        entry.files.push((sha.to_owned(), path.to_owned()));
                    }
                }
                "V" => {
                    if let Some((name, value)) = rest.split_once('=') {
                        entry.env.push((name.to_owned(), Some(value.to_owned())));
                    }
                }
                "U" => entry.env.push((rest.to_owned(), None)),
                _ => {}
            }
        }
        entries
    }

    /// Puts an entry first among the ways this command line was compiled, keeping the last twenty.
    pub(crate) fn add_entry(&self, request: &str, entry: &Entry) -> io::Result<()> {
        let mut entries = self.entries(request);
        entries.retain(|known| known.result != entry.result);
        entries.insert(0, entry.clone());
        entries.truncate(20);
        let mut text = String::new();
        for entry in &entries {
            text.push_str(&entry_text(entry));
        }
        Self::write_atomic(&self.fanned("manifests", request), text.as_bytes())
    }

    /// What was kept under a result key.
    pub(crate) fn stored(&self, result: &str) -> Option<Stored> {
        let text = fs::read_to_string(self.fanned("results", result)).ok()?;
        let mut stored = Stored::default();
        for line in text.lines() {
            match line.split_once(' ') {
                Some(("O", rest)) => {
                    let mut parts = rest.splitn(4, ' ');
                    let (emit, sha, size, name) =
                        (parts.next()?, parts.next()?, parts.next()?, parts.next()?);
                    stored.outputs.push(Output {
                        emit: emit.to_owned(),
                        sha: sha.to_owned(),
                        size: size.parse().ok()?,
                        name: name.to_owned(),
                    });
                }
                Some(("S", rest)) => stored.stderr.push(rest.to_owned()),
                _ => {}
            }
        }
        (!stored.outputs.is_empty()).then_some(stored)
    }

    /// Keeps the list of what a compilation produced.
    pub(crate) fn keep_stored(&self, result: &str, stored: &Stored) -> io::Result<()> {
        let mut text = String::from("xcache-result 1\n");
        for output in &stored.outputs {
            text.push_str(&format!("O {} {} {} {}\n", output.emit, output.sha, output.size, output.name));
        }
        for line in &stored.stderr {
            text.push_str(&format!("S {line}\n"));
        }
        Self::write_atomic(&self.fanned("results", result), text.as_bytes())
    }

    /// Forgets a result that could not be restored.
    pub(crate) fn drop_stored(&self, result: &str) {
        let _ = fs::remove_file(self.fanned("results", result));
    }

    /// Marks a result as used now, which is what cleaning goes by.
    pub(crate) fn touch(&self, result: &str) {
        if let Ok(file) = fs::File::options().write(true).open(self.fanned("results", result)) {
            let _ = file.set_modified(SystemTime::now());
        }
    }

    /// Adds a line to the log of requests.
    pub(crate) fn log(&self, line: &str) {
        let path = self.root.join("events.tsv");
        if let Ok(mut file) = fs::File::options().create(true).append(true).open(path) {
            let _ = file.write_all(format!("{line}\n").as_bytes());
        }
    }
}

/// An entry as text. The result key is made from this, without its first line.
pub(crate) fn entry_text(entry: &Entry) -> String {
    let mut text = format!("R {}\n", entry.result);
    if let Some(roots) = &entry.pinned {
        text.push_str(&format!("P {roots}\n"));
    }
    for (sha, path) in &entry.files {
        text.push_str(&format!("F {sha} {path}\n"));
    }
    for (name, value) in &entry.env {
        match value {
            Some(value) => text.push_str(&format!("V {name}={value}\n")),
            None => text.push_str(&format!("U {name}\n")),
        }
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store(name: &str) -> Store {
        let root = std::env::temp_dir().join(format!("xcache-store-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap_or_default();
        Store { root }
    }

    #[test]
    fn entries_and_results_come_back_as_they_were_kept() {
        let store = store("roundtrip");
        let entry = Entry {
            result: "r1".into(),
            pinned: Some("/a|/b".into()),
            files: vec![("abc".into(), "<xcache:cwd>/src/lib with space.rs".into())],
            env: vec![("OUT_DIR".into(), Some("<xcache:target>/out=x".into())), ("GONE".into(), None)],
        };
        store.add_entry("req", &entry).unwrap_or_default();
        let newer = Entry { result: "r2".into(), ..Entry::default() };
        store.add_entry("req", &newer).unwrap_or_default();
        assert_eq!(store.entries("req"), vec![newer, entry]);

        let stored = Stored {
            outputs: vec![Output {
                emit: "link".into(),
                sha: "f00".into(),
                size: 3,
                name: "libx-1 2.rlib".into(),
            }],
            stderr: vec!["A link libx-1 2.rlib".into(), "D {\"message\":\"x\"}".into()],
        };
        store.keep_stored("r1", &stored).unwrap_or_default();
        assert_eq!(store.stored("r1"), Some(stored));
        assert_eq!(store.stored("missing"), None);
        let _ = fs::remove_dir_all(&store.root);
    }

    #[test]
    fn a_file_is_known_by_what_was_recorded_until_it_changes() {
        let store = store("identity");
        let file = store.root.join("lib.rmeta");
        fs::write(&file, b"one").unwrap_or_default();
        assert_eq!(store.identity(&file).ok(), Some(sha(b"one")));
        store.remember(&file, "what-it-was-built-from");
        assert_eq!(store.identity(&file).ok().as_deref(), Some("what-it-was-built-from"));
        fs::write(&file, b"three").unwrap_or_default();
        assert_eq!(store.identity(&file).ok(), Some(sha(b"three")));
        let _ = fs::remove_dir_all(&store.root);
    }

    #[test]
    fn content_is_kept_once_under_its_hash() {
        let store = store("blobs");
        let kept = store.keep_bytes(b"hello").unwrap_or_default();
        assert_eq!(kept, sha(b"hello"));
        assert_eq!(fs::read(store.blob(&kept)).ok().as_deref(), Some(&b"hello"[..]));
        assert_eq!(sha_of_file(&store.blob(&kept)).ok(), Some(kept));
        let _ = fs::remove_dir_all(&store.root);
    }
}
