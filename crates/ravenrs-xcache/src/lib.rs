//! A compiler cache for Rust that worktrees share.
//!
//! Cargo keeps what it compiled in a build directory, and each worktree has its own, so a new
//! worktree compiles again what another one just compiled. `ravenrs-xcache` stands in front of rustc
//! (`RUSTC_WRAPPER=ravenrs-xcache`), keeps what each library compiled to under a key made of what
//! went into it, and puts it back wherever the same inputs come up, in any build directory.
//!
//! What makes that possible between worktrees is taking the paths out of the compilation: rustc is
//! told to write `/xcache/target` and `/xcache/cwd` instead of the build directory and the checkout
//! (`--remap-path-prefix`), which makes the artifacts byte for byte the same wherever they are built.
//! After compiling, the artifacts are searched for the real paths. One that still carries them
//! embedded them as data, and is kept for the paths it was built under only.
//!
//! This first version keeps libraries (`lib`, `rlib`), from `cargo build` and `cargo check`. What
//! links (executables, tests, proc macros), Clippy on workspace crates, incremental compilation and
//! rustdoc go to rustc as they came, and `ravenrs-xcache stats` says how many and why. See the
//! README for what is planned.

mod args;
mod paths;
mod store;
mod wrapper;

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs;
use std::path::Path;

use store::Store;

const HELP: &str = "ravenrs-xcache: a compiler cache for Rust that worktrees share

  RUSTC_WRAPPER=ravenrs-xcache cargo build     use it for a build

  ravenrs-xcache stats [--reset]      hits, misses, what went past the cache and why, bytes and times
  ravenrs-xcache explain <crate>      what happened to a crate's last requests, and why
  ravenrs-xcache gc --max-size 20G    remove the results used longest ago until the cache fits
  ravenrs-xcache clear                empty the cache

  RAVENRS_XCACHE_DIR       where the cache is (default ~/.cache/ravenrs/xcache)
  RAVENRS_XCACHE_DISABLE=1 run rustc as if the wrapper were not there
  RAVENRS_XCACHE_VERIFY=0  restore without checking each file against its hash
";

/// Runs the program with its arguments (without the program name) and returns its exit code.
///
/// With a compiler as the first argument it is the wrapper Cargo calls; otherwise it is one of its
/// own commands.
pub fn main(arguments: Vec<OsString>) -> i32 {
    let first = arguments.first().and_then(|arg| arg.to_str()).unwrap_or_default();
    let rest: Vec<&str> = arguments.iter().skip(1).filter_map(|arg| arg.to_str()).collect();
    match first {
        "" | "help" | "--help" | "-h" => {
            print!("{HELP}");
            0
        }
        "--version" | "-V" => {
            println!("ravenrs-xcache {}", env!("CARGO_PKG_VERSION"));
            0
        }
        "stats" => command(|store| stats(store, rest.contains(&"--reset"))),
        "explain" => command(|store| explain(store, rest.first().copied().unwrap_or_default())),
        "gc" => command(|store| {
            let limit = rest.iter().position(|arg| *arg == "--max-size").and_then(|at| rest.get(at + 1));
            match limit.and_then(|text| size(text)) {
                Some(limit) => collect(store, limit),
                None => Err("gc needs --max-size, such as 20G or 500M".to_owned()),
            }
        }),
        "clear" => command(|store| fs::remove_dir_all(&store.root).map_err(|e| e.to_string())),
        _ => wrapper::run(&arguments[0], &arguments[1..]),
    }
}

fn command(run: impl FnOnce(&Store) -> Result<(), String>) -> i32 {
    match Store::open().map_err(|e| e.to_string()).and_then(|store| run(&store)) {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("ravenrs-xcache: {error}");
            1
        }
    }
}

/// One line of the log of requests.
struct Event {
    kind: String,
    crate_name: String,
    reason: String,
    key: u64,
    lookup: u64,
    restore: u64,
    compile: u64,
    bytes: u64,
}

fn events(store: &Store) -> Vec<Event> {
    let text = fs::read_to_string(store.root.join("events.tsv")).unwrap_or_default();
    text.lines()
        .filter_map(|line| {
            let parts: Vec<&str> = line.split('\t').collect();
            let number = |at: usize| parts.get(at).and_then(|text| text.parse::<u64>().ok()).unwrap_or(0);
            Some(Event {
                kind: (*parts.get(1)?).to_owned(),
                crate_name: (*parts.get(2)?).to_owned(),
                reason: (*parts.get(3)?).to_owned(),
                key: number(4),
                lookup: number(5),
                restore: number(6),
                compile: number(7),
                bytes: number(8),
            })
        })
        .collect()
}

fn seconds(milliseconds: u64) -> String {
    format!("{:.1} s", milliseconds as f64 / 1000.0)
}

fn megabytes(bytes: u64) -> String {
    format!("{:.1} MiB", bytes as f64 / 1_048_576.0)
}

fn stats(store: &Store, reset: bool) -> Result<(), String> {
    let events = events(store);
    let of = |kind: &str| events.iter().filter(|event| event.kind == kind).collect::<Vec<_>>();
    let (hits, misses, passes, failed) = (of("hit"), of("miss"), of("pass"), of("failed"));
    let total = hits.len() + misses.len();
    let sum =
        |events: &[&Event], field: fn(&Event) -> u64| events.iter().map(|event| field(event)).sum::<u64>();
    println!("ravenrs-xcache, {}", store.root.display());
    println!("  libraries      {total}: {} from the cache, {} compiled", hits.len(), misses.len());
    if total > 0 {
        println!("  hit rate       {:.1}%", 100.0 * hits.len() as f64 / total as f64);
    }
    println!(
        "  past the cache {} compiled as they came, {} questions to rustc",
        passes.len(),
        of("probe").len()
    );
    println!("  failed         {}", failed.len());
    println!("  restored       {}", megabytes(sum(&hits, |e| e.bytes)));
    println!("  kept           {}", megabytes(sum(&misses, |e| e.bytes)));
    println!(
        "  time, hits     keys {}, looking up {}, restoring {}",
        seconds(sum(&hits, |e| e.key)),
        seconds(sum(&hits, |e| e.lookup)),
        seconds(sum(&hits, |e| e.restore))
    );
    println!(
        "  time, misses   keys {}, looking up {}, compiling {}",
        seconds(sum(&misses, |e| e.key)),
        seconds(sum(&misses, |e| e.lookup)),
        seconds(sum(&misses, |e| e.compile))
    );
    println!("  time, past     compiling {}", seconds(sum(&passes, |e| e.compile)));
    for (title, group) in
        [("why libraries were compiled", &misses), ("why requests went past the cache", &passes)]
    {
        let mut reasons: BTreeMap<String, (usize, u64)> = BTreeMap::new();
        for event in group {
            // A reason that names a file or a variable is one reason for every file.
            let reason = event.reason.split(": ").next().unwrap_or_default().to_owned();
            let entry = reasons.entry(reason).or_default();
            entry.0 += 1;
            entry.1 += event.compile;
        }
        let mut reasons: Vec<_> = reasons.into_iter().collect();
        reasons.sort_by_key(|(_, (count, time))| (std::cmp::Reverse(*time), std::cmp::Reverse(*count)));
        if !reasons.is_empty() {
            println!("  {title}:");
        }
        for (reason, (count, time)) in reasons {
            println!("    {count:>5}  {:>9}  {reason}", seconds(time));
        }
    }
    let (count, bytes) =
        blobs(store).iter().fold((0, 0), |(count, bytes), (_, size)| (count + 1, bytes + size));
    println!("  on disk        {} in {count} files", megabytes(bytes));
    if reset {
        fs::remove_file(store.root.join("events.tsv")).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn explain(store: &Store, crate_name: &str) -> Result<(), String> {
    if crate_name.is_empty() {
        return Err("explain needs a crate name, as rustc writes it (ravenrs_app)".to_owned());
    }
    let events = events(store);
    let about: Vec<&Event> = events.iter().filter(|event| event.crate_name == crate_name).collect();
    if about.is_empty() {
        return Err(format!("no request for {crate_name} is in the log"));
    }
    for event in about.iter().rev().take(10).rev() {
        let what = match event.kind.as_str() {
            "hit" => format!("from the cache ({}, restored in {} ms)", megabytes(event.bytes), event.restore),
            "miss" => format!("compiled in {}: {}", seconds(event.compile), event.reason),
            "pass" | "probe" => format!("past the cache: {}", event.reason),
            _ => format!("failed to compile ({})", event.reason),
        };
        println!("{crate_name}: {what}");
    }
    Ok(())
}

/// Every kept file and its size.
fn blobs(store: &Store) -> Vec<(std::path::PathBuf, u64)> {
    files_under(&store.root.join("blobs"))
        .into_iter()
        .filter_map(|path| Some((fs::metadata(&path).ok()?.len(), path)))
        .map(|(size, path)| (path, size))
        .collect()
}

fn files_under(folder: &Path) -> Vec<std::path::PathBuf> {
    let mut found = Vec::new();
    let Ok(entries) = fs::read_dir(folder) else { return found };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            found.extend(files_under(&path));
        } else {
            found.push(path);
        }
    }
    found
}

/// `20G`, `500M`, `1024K` or a number of bytes.
fn size(text: &str) -> Option<u64> {
    let (number, unit) = match text.trim().to_ascii_uppercase() {
        text if text.ends_with('G') => (text.trim_end_matches('G').to_owned(), 1_u64 << 30),
        text if text.ends_with('M') => (text.trim_end_matches('M').to_owned(), 1 << 20),
        text if text.ends_with('K') => (text.trim_end_matches('K').to_owned(), 1 << 10),
        text => (text, 1),
    };
    number.parse::<f64>().ok().filter(|n| *n >= 0.0).map(|n| (n * unit as f64) as u64)
}

/// Removes the results used longest ago until what they hold fits in `limit` bytes, then every kept
/// file no result names.
fn collect(store: &Store, limit: u64) -> Result<(), String> {
    /// A result: when it was last used, its file, and the kept files it names with their sizes.
    type Kept = (std::time::SystemTime, std::path::PathBuf, Vec<(String, u64)>);
    let mut results: Vec<Kept> = Vec::new();
    for path in files_under(&store.root.join("results")) {
        let used = fs::metadata(&path).and_then(|meta| meta.modified()).map_err(|e| e.to_string())?;
        let key = path.file_name().and_then(|name| name.to_str()).unwrap_or_default().to_owned();
        let outputs = store.stored(&key).map(|stored| stored.outputs).unwrap_or_default();
        results.push((used, path, outputs.into_iter().map(|output| (output.sha, output.size)).collect()));
    }
    results.sort_by_key(|(used, ..)| std::cmp::Reverse(*used));
    let mut keep: BTreeMap<String, u64> = BTreeMap::new();
    let (mut total, mut removed) = (0_u64, 0_usize);
    for (_, path, outputs) in &results {
        let extra: u64 =
            outputs.iter().filter(|(sha, _)| !keep.contains_key(sha)).map(|(_, size)| size).sum();
        if total + extra > limit {
            let _ = fs::remove_file(path);
            removed += 1;
            continue;
        }
        total += extra;
        keep.extend(outputs.iter().cloned());
    }
    let mut freed = 0;
    for (path, size) in blobs(store) {
        let name = path.file_name().and_then(|name| name.to_str()).unwrap_or_default();
        if !keep.contains_key(name) {
            let _ = fs::remove_file(&path);
            freed += size;
        }
    }
    println!("removed {removed} results and {}; {} kept", megabytes(freed), megabytes(total));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_are_read_with_their_units() {
        assert_eq!(size("20G"), Some(20 << 30));
        assert_eq!(size("1.5m"), Some(3 << 19));
        assert_eq!(size("4096"), Some(4096));
        assert_eq!(size("big"), None);
    }
}
