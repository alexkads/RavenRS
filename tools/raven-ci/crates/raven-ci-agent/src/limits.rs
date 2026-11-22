//! The limits of an agent: cores, memory and disk (A-04, #896).
//!
//! - **Cores** (`--nucleos N`): the agent runs no more executors than N, and every step gets
//!   `RAVEN_CI_NUCLEOS`, `CARGO_BUILD_JOBS` and `MAKEFLAGS=-jN`. On Linux the agent's cgroup gets
//!   `cpu.max` too.
//! - **Memory** (`--memoria 8G`): on Linux the agent moves itself into a cgroup of its own with
//!   `memory.max` (`ravenrs_process::confine`), when its cgroup is delegated to it. On every system
//!   the watch measures the agent's tree of processes ([`ravenrs_metrics::tree_rss_bytes`]) and
//!   cancels the newest build when the tree passes the limit.
//! - **Disk** (`--disco 50G`): the watch measures the folder of each build and cancels the one that
//!   passes. With less free room than the minimum (`--disco-livre`, else the 5 GB of
//!   `workspace-cleanup`), the agent takes no build.
//!
//! The watch looks every [`LOOK_EVERY`] (`--vigia-ms`, for tests).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use crate::site::AgentSite;

/// How often the watch looks, unless said.
pub const LOOK_EVERY: Duration = Duration::from_secs(2);

const GB: f64 = 1024.0 * 1024.0 * 1024.0;

/// A size as people write it: `512M`, `8G`, `1.5G`, `2048` (bytes), `20GB`.
pub fn parse_size(text: &str) -> Option<u64> {
    let text = text.trim().to_ascii_uppercase();
    let text = text.strip_suffix('B').unwrap_or(&text);
    let (number, unit) = match text.chars().last()? {
        'K' => (&text[..text.len() - 1], 1024.0),
        'M' => (&text[..text.len() - 1], 1024.0 * 1024.0),
        'G' => (&text[..text.len() - 1], GB),
        'T' => (&text[..text.len() - 1], GB * 1024.0),
        _ => (text, 1.0),
    };
    let number: f64 = number.trim().replace(',', ".").parse().ok()?;
    (number >= 0.0).then(|| (number * unit).round() as u64)
}

/// `bytes` for people: `1,5 GB`, `300 MB`.
pub fn words(bytes: u64) -> String {
    let (value, unit) =
        if bytes as f64 >= GB { (bytes as f64 / GB, "GB") } else { (bytes as f64 / (1024.0 * 1024.0), "MB") };
    format!("{value:.1} {unit}").replace('.', ",")
}

/// The limits of an agent.
#[derive(Clone, Debug)]
pub struct Limits {
    /// How many cores' worth of work.
    pub cores: Option<u32>,
    /// The most memory its tree of processes holds, in bytes.
    pub memory_bytes: Option<u64>,
    /// The most a build's folder takes, in bytes.
    pub disk_bytes: Option<u64>,
    /// The least free room on its disk to take a build, in bytes.
    pub min_free_bytes: u64,
    /// How often the watch looks.
    pub every: Duration,
    /// Whether the system holds the memory and cores (a cgroup of its own, on Linux).
    confined: Arc<AtomicBool>,
}

impl PartialEq for Limits {
    fn eq(&self, other: &Self) -> bool {
        (self.cores, self.memory_bytes, self.disk_bytes, self.min_free_bytes, self.every)
            == (other.cores, other.memory_bytes, other.disk_bytes, other.min_free_bytes, other.every)
    }
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            cores: None,
            memory_bytes: None,
            disk_bytes: None,
            min_free_bytes: (raven_ci_plugin_workspace::DEFAULT_MIN_FREE_GB * GB) as u64,
            every: LOOK_EVERY,
            confined: Arc::default(),
        }
    }
}

fn value<'a>(args: &'a [String], flag: &str) -> Result<Option<&'a str>, String> {
    match args.iter().position(|a| a == flag) {
        Some(at) => {
            args.get(at + 1).map(|v| Some(v.as_str())).ok_or_else(|| format!("{flag} precisa de um valor"))
        }
        None => Ok(None),
    }
}

fn size(args: &[String], flag: &str) -> Result<Option<u64>, String> {
    value(args, flag)?
        .map(|text| parse_size(text).ok_or_else(|| format!("{flag} é um tamanho, como 512M ou 8G: {text}")))
        .transpose()
}

impl Limits {
    /// What `args` say: `--nucleos N`, `--memoria 8G`, `--disco 50G`, `--disco-livre 5G` and
    /// `--vigia-ms N`.
    pub fn from_args(args: &[String]) -> Result<Self, String> {
        let cores = value(args, "--nucleos")?
            .map(|n| {
                n.parse::<u32>().ok().filter(|n| *n > 0).ok_or_else(|| format!("--nucleos é um número: {n}"))
            })
            .transpose()?;
        let mut limits = Self {
            cores,
            memory_bytes: size(args, "--memoria")?,
            disk_bytes: size(args, "--disco")?,
            ..Self::default()
        };
        if let Some(free) = size(args, "--disco-livre")? {
            limits.min_free_bytes = free;
        }
        if let Some(ms) = value(args, "--vigia-ms")? {
            let ms: u64 = ms.parse().map_err(|_| format!("--vigia-ms é um número: {ms}"))?;
            limits.every = Duration::from_millis(ms.max(10));
        }
        Ok(limits)
    }

    /// How many executors it may have, of the `asked`: no more than its cores.
    pub fn executors(&self, asked: u32) -> u32 {
        self.cores.map_or(asked, |cores| asked.min(cores)).max(1)
    }

    /// Puts the limits on the agent's own process, where the system can hold them; what to say of
    /// them.
    pub fn apply(&self, name: &str) -> Vec<String> {
        if self.memory_bytes.is_none() && self.cores.is_none() {
            return Vec::new();
        }
        let limits =
            ravenrs_process::Limits { memory_bytes: self.memory_bytes, cores: self.cores.map(f64::from) };
        match ravenrs_process::confine(std::process::id(), &format!("raven-ci-agente-{name}"), &limits) {
            Ok(confined) => {
                self.confined.store(true, Ordering::SeqCst);
                vec![format!("limites no cgroup {}", confined.cgroup.display())]
            }
            Err(e) => vec![format!(
                "o sistema não garante os limites de memória e de núcleos ({e}); o vigia do agente os confere a cada {} ms",
                self.every.as_millis()
            )],
        }
    }

    /// The limits and their use, for the hello.
    pub fn report(&self, dir: &Path) -> Value {
        json!({
            "nucleos": self.cores,
            "memoria_bytes": self.memory_bytes,
            "disco_bytes": self.disk_bytes,
            "disco_livre_bytes": ravenrs_metrics::free_disk_bytes(dir),
            "cgroup": self.confined.load(Ordering::SeqCst),
        })
    }

    /// Whether there is room on the disk of `dir` to take a build.
    pub fn room(&self, dir: &Path) -> bool {
        ravenrs_metrics::free_disk_bytes(dir).is_none_or(|free| free >= self.min_free_bytes)
    }

    /// The variables the steps get: the cores, as the tools that compile read them.
    pub fn environment(&self) -> BTreeMap<String, String> {
        let Some(cores) = self.cores else { return BTreeMap::new() };
        BTreeMap::from([
            ("RAVEN_CI_NUCLEOS".to_owned(), cores.to_string()),
            ("CARGO_BUILD_JOBS".to_owned(), cores.to_string()),
            ("MAKEFLAGS".to_owned(), format!("-j{cores}")),
        ])
    }

    /// Watches the build of `site`, in `workspace`, while the result is held; on `watching`, the
    /// builds of the agent, so the memory of the tree is held by the newest.
    pub fn watch(&self, workspace: &Path, site: Arc<AgentSite>, watching: &Arc<Watching>) -> Watch {
        if self.memory_bytes.is_none() && self.disk_bytes.is_none() {
            return Watch { stop: None };
        }
        let id = watching.add();
        let stop = Arc::new(AtomicBool::new(false));
        let (limits, workspace, stopping, all) =
            (self.clone(), workspace.to_path_buf(), stop.clone(), watching.clone());
        let thread = std::thread::Builder::new().name("raven-ci-vigia".to_owned()).spawn(move || {
            while !stopping.load(Ordering::SeqCst) {
                if let Some(why) = limits.passed(&workspace, id, &all) {
                    site.log_line(&why);
                    site.cancel_steps(&why);
                    break;
                }
                let until = Instant::now() + limits.every;
                while Instant::now() < until && !stopping.load(Ordering::SeqCst) {
                    std::thread::sleep(Duration::from_millis(20));
                }
            }
            all.remove(id);
        });
        Watch { stop: thread.ok().map(|thread| (stop, thread)) }
    }

    /// Why the build `id`, in `workspace`, has to stop, when it does.
    fn passed(&self, workspace: &Path, id: u64, all: &Watching) -> Option<String> {
        if let Some(most) = self.disk_bytes {
            let used = size_of(workspace);
            if used > most {
                return Some(format!(
                    "passou do limite de disco de {}: a pasta do build tem {}",
                    words(most),
                    words(used)
                ));
            }
        }
        if let Some(most) = self.memory_bytes
            && all.newest() == Some(id)
            && let Some(used) = ravenrs_metrics::tree_rss_bytes(std::process::id())
            && used > most
        {
            return Some(format!(
                "passou do limite de memória de {}: o agente e os passos têm {}",
                words(most),
                words(used)
            ));
        }
        None
    }
}

/// The builds an agent watches, the newest last.
#[derive(Debug, Default)]
pub struct Watching {
    builds: Mutex<Vec<u64>>,
    next: Mutex<u64>,
}

impl Watching {
    fn add(&self) -> u64 {
        let mut next = self.next.lock().unwrap_or_else(PoisonError::into_inner);
        *next += 1;
        self.builds.lock().unwrap_or_else(PoisonError::into_inner).push(*next);
        *next
    }

    fn remove(&self, id: u64) {
        self.builds.lock().unwrap_or_else(PoisonError::into_inner).retain(|b| *b != id);
    }

    fn newest(&self) -> Option<u64> {
        self.builds.lock().unwrap_or_else(PoisonError::into_inner).last().copied()
    }
}

/// A build watched; dropped, the watch stops.
#[derive(Debug)]
pub struct Watch {
    stop: Option<(Arc<AtomicBool>, std::thread::JoinHandle<()>)>,
}

impl Drop for Watch {
    fn drop(&mut self) {
        if let Some((stop, thread)) = self.stop.take() {
            stop.store(true, Ordering::SeqCst);
            let _ = thread.join();
        }
    }
}

/// The size of everything under `folder`, in bytes.
fn size_of(folder: &Path) -> u64 {
    let mut total = 0;
    let mut waiting: Vec<PathBuf> = vec![folder.to_path_buf()];
    while let Some(folder) = waiting.pop() {
        let Ok(entries) = std::fs::read_dir(&folder) else { continue };
        for entry in entries.flatten() {
            match entry.file_type() {
                Ok(kind) if kind.is_dir() => waiting.push(entry.path()),
                Ok(_) => total += entry.metadata().map_or(0, |m| m.len()),
                Err(_) => {}
            }
        }
    }
    total
}

/// The memory the agent holds now, in bytes, when the system says.
pub fn memory_now() -> Option<u64> {
    ravenrs_metrics::process_rss_bytes().or_else(|| ravenrs_metrics::rss_bytes_of(std::process::id()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_are_read_as_people_write_them() {
        assert_eq!(parse_size("512M"), Some(512 * 1024 * 1024));
        assert_eq!(parse_size("8G"), Some(8 * 1024 * 1024 * 1024));
        assert_eq!(parse_size("1,5GB"), Some((1.5 * GB) as u64));
        assert_eq!(parse_size("2048"), Some(2048));
        assert_eq!(parse_size("muito"), None);
        assert_eq!(words(1536 * 1024 * 1024), "1,5 GB");
    }

    #[test]
    fn the_cores_cap_the_executors_and_reach_the_steps() {
        let args: Vec<String> = ["--nucleos", "2", "--memoria", "1G"].map(str::to_owned).to_vec();
        let limits = Limits::from_args(&args).expect("limits");
        assert_eq!(limits.executors(4), 2);
        assert_eq!(limits.executors(1), 1);
        assert_eq!(limits.environment()["MAKEFLAGS"], "-j2");
        assert_eq!(limits.memory_bytes, Some(1024 * 1024 * 1024));
        assert!(Limits::from_args(&["--nucleos".to_owned(), "0".to_owned()]).is_err());
    }
}
