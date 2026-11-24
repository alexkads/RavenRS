//! The plugin `lock` of Raven CI: named locks that one build at a time holds.
//!
//! # Its role
//!
//! Two heavy builds on one machine slow each other down, or break each other when they share a
//! cache. A stage with `lock = "maquina"` in a `.raven-ci.toml` runs only while its build holds the
//! lock `maquina`; another build that asks for it waits. A script outside Raven CI that takes the
//! same file with `flock` waits too, and makes the builds wait.
//!
//! # How it works
//!
//! - **One file per lock**: `<dir>/<name>.lock`, where `dir` is the plugin's setting, or its own
//!   folder (`dados-dos-plugins/lock/travas`). It is locked with [`std::fs::File::lock`], which is
//!   `flock` on Linux and macOS and `LockFileEx` on Windows, so `flock /that/file cmd` in a script
//!   shares it.
//! - **A file of its own**: `lock = { name = "oficina", path = "~/oficina/.trava-compilacao" }`
//!   takes that file instead (#930), so a build shares the lock a machine's scripts already use;
//!   `~/` is the home of the user that runs the step, the agent's on an agent.
//! - **The step `lock`** (parameter `name`) waits for the lock, looking at its token, so cancelling
//!   the build ends the wait. It says once in the log who holds it (the holder writes itself in
//!   the file), and then that it took it.
//! - **The step `unlock`** lets it go. Whatever a build still holds when it ends, failed or
//!   cancelled, goes with it ([`BuildScope::at_end`]).
//! - **Never handed down.** Rust opens files closed on `exec`, so a program a step starts (a cache
//!   server left running, a process in the background) does not keep the lock after the build.
//!
//! # How to use it
//!
//! ```toml
//! [[stages]]
//! name = "Compila"
//! lock = "maquina"
//! steps = ["cargo build --release"]
//! ```

use std::collections::HashMap;
use std::fs::{File, OpenOptions, TryLockError};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};

use raven_ci_plugin_api::{BuildScope, CiError, CiPlugin, Host, Permission};
use ravenrs_app::{EntityInfo, FieldInfo, FieldKind, Module, ModuleManifest};
use ravenrs_flows::{Catalog, NodeContext, NodeError, NodeKind, NodeSpec, Ports, Reconciled};
use serde_json::Value;

/// The kind of the step that takes a lock.
pub const LOCK: &str = "lock";
/// The kind of the step that lets it go.
pub const UNLOCK: &str = "unlock";

/// How often a step that waits looks at the lock again, in milliseconds.
const LOOK_EVERY_MS: u64 = 100;

/// The plugin `lock`.
#[derive(Default)]
pub struct LockPlugin {
    state: Arc<State>,
}

impl std::fmt::Debug for LockPlugin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LockPlugin").finish_non_exhaustive()
    }
}

#[derive(Default)]
struct State {
    dir: Mutex<Option<PathBuf>>,
    /// The locks held, by the run of the build and the lock's name.
    held: Mutex<HashMap<(String, String), File>>,
}

impl State {
    fn dir(&self) -> Result<PathBuf, NodeError> {
        self.dir
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
            .ok_or_else(|| NodeError::new("o plugin lock não está ligado"))
    }

    /// Lets go of the lock `name` of the run `run`; whether it was held.
    fn release(&self, run: &str, name: &str) -> bool {
        self.held
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(&(run.to_owned(), name.to_owned()))
            .is_some()
    }
}

impl LockPlugin {
    /// The plugin, switched off until the core starts it.
    pub fn new() -> Self {
        Self::default()
    }
}

impl Module for LockPlugin {
    fn name(&self) -> &str {
        "lock"
    }

    fn manifest(&self) -> ModuleManifest {
        ModuleManifest::new()
            .version(env!("CARGO_PKG_VERSION"))
            .unwrap_or_default()
            .capability(Permission::Disk.capability())
    }
}

impl CiPlugin for LockPlugin {
    fn summary(&self) -> String {
        "Travas nomeadas entre builds e scripts de fora, nos mesmos arquivos do flock.".to_owned()
    }

    fn config(&self) -> Option<EntityInfo> {
        Some(EntityInfo::new("LockSettings").field(FieldInfo::new("dir", FieldKind::Text)))
    }

    fn start(&self, host: Arc<dyn Host>) -> Result<(), CiError> {
        let dir = match host.settings().get("dir").and_then(Value::as_str) {
            Some(dir) if !dir.is_empty() => PathBuf::from(dir),
            _ => host.data_dir()?.join("travas"),
        };
        std::fs::create_dir_all(&dir)
            .map_err(|e| CiError::failed(format!("a pasta das travas {}: {e}", dir.display())))?;
        *self.state.dir.lock().unwrap_or_else(PoisonError::into_inner) = Some(dir);
        Ok(())
    }

    fn stop(&self) {
        self.state.dir.lock().unwrap_or_else(PoisonError::into_inner).take();
        self.state.held.lock().unwrap_or_else(PoisonError::into_inner).clear();
    }

    fn steps(&self, catalog: &mut Catalog) {
        catalog.add(Lock(self.state.clone()));
        catalog.add(Unlock(self.state.clone()));
    }
}

/// The name of the lock of the node of `cx`, checked: letters, numbers, `-`, `_` and `.`.
/// `path` with a leading `~/` made the home of the user running the step: on an agent, the
/// agent's (#930).
fn home_of(path: &str) -> PathBuf {
    match (path.strip_prefix("~/"), std::env::var_os("HOME")) {
        (Some(rest), Some(home)) => PathBuf::from(home).join(rest),
        _ => PathBuf::from(path),
    }
}

fn name_of(cx: &NodeContext<'_>) -> Result<String, NodeError> {
    let name = cx.parameter("name").and_then(Value::as_str).unwrap_or_default();
    let fine = !name.is_empty()
        && !name.starts_with('.')
        && name.chars().all(|c| c.is_alphanumeric() || matches!(c, '-' | '_' | '.'));
    if fine {
        Ok(name.to_owned())
    } else {
        Err(NodeError::new(format!(
            "{name:?} não serve de nome de trava: use letras, números, '-', '_' e '.'"
        )))
    }
}

/// Who holds `file`, as its holder wrote it; `None` when it cannot be read.
fn holder(file: &mut File) -> Option<String> {
    let mut said = String::new();
    file.seek(SeekFrom::Start(0)).ok()?;
    file.read_to_string(&mut said).ok()?;
    let said = said.trim().to_owned();
    (!said.is_empty()).then_some(said)
}

/// The step `lock`. See the [crate](crate).
struct Lock(Arc<State>);

impl NodeKind for Lock {
    fn spec(&self) -> NodeSpec {
        NodeSpec::new(LOCK, 1).title("Trava").input("main").output("main").requires("name")
    }

    /// A lock taken by a worker that died is free again (the system lets go of a dead process's
    /// `flock`), so taking it again is right.
    fn reconcile(&self, _: &NodeContext<'_>) -> Reconciled {
        Reconciled::NotDone
    }

    fn execute(&self, cx: &NodeContext<'_>) -> Result<Ports, NodeError> {
        let build = BuildScope::of(cx)?;
        let step = cx.node().id.clone();
        let name = name_of(cx)?;
        let path = match cx.parameter("path").and_then(Value::as_str).filter(|p| !p.is_empty()) {
            Some(path) => home_of(path),
            None => self.0.dir()?.join(format!("{name}.lock")),
        };
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)
            .map_err(|e| NodeError::new(format!("a trava {}: {e}", path.display())))?;
        let mut said = false;
        loop {
            match file.try_lock() {
                Ok(()) => break,
                Err(TryLockError::WouldBlock) => {
                    if !said {
                        let who = holder(&mut file).unwrap_or_else(|| "outro processo".to_owned());
                        build.log(&step, &format!("esperando a trava {name}, com {who}"));
                        said = true;
                    }
                    cx.sleep(LOOK_EVERY_MS)?;
                }
                Err(TryLockError::Error(e)) => {
                    return Err(NodeError::new(format!("a trava {}: {e}", path.display())));
                }
            }
        }
        let context = build.context();
        let me = format!("{} #{} (Raven CI)", context.job, context.number);
        // Who holds it, for whoever waits; a failure here does not undo the lock.
        let _ = file.set_len(0).and_then(|()| file.write_all(me.as_bytes())).and_then(|()| file.flush());
        build.log(&step, &format!("peguei a trava {name}"));
        let run = context.run.clone();
        self.0.held.lock().unwrap_or_else(PoisonError::into_inner).insert((run.clone(), name.clone()), file);
        let state = self.0.clone();
        let what = format!("soltar a trava {name}");
        build.at_end(what, move || {
            state.release(&run, &name);
        });
        Ok(BuildScope::pass_on(cx))
    }
}

/// The step `unlock`. See the [crate](crate).
struct Unlock(Arc<State>);

impl NodeKind for Unlock {
    fn spec(&self) -> NodeSpec {
        NodeSpec::new(UNLOCK, 1).title("Solta a trava").input("main").output("main").requires("name")
    }

    fn execute(&self, cx: &NodeContext<'_>) -> Result<Ports, NodeError> {
        let build = BuildScope::of(cx)?;
        let name = name_of(cx)?;
        if self.0.release(&build.context().run, &name) {
            build.log(&cx.node().id, &format!("soltei a trava {name}"));
        }
        Ok(BuildScope::pass_on(cx))
    }
}
