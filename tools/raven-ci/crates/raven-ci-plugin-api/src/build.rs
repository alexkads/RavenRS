//! What a step finds of the build it runs in: its folder, its log and its environment.
//!
//! # The role
//!
//! A step is a [`NodeKind`](ravenrs_flows::NodeKind) of `ravenrs-flows`, which knows nothing of
//! builds: it gets its parameters and the items that reached it. A step of a CI needs more: the
//! folder the code is in, a log people read while it runs, and the variables of the build. The
//! core keeps those in a [`BuildScope`] for each build that runs, and a step finds the scope of
//! its build with [`BuildScope::of`].
//!
//! # How it works
//!
//! Before the steps of a build run, the core makes its scope and [`enter`]s it under the id of the
//! build's run; the queue gives that id to every node ([`NodeContext::run_id`]). When the build
//! ends, the guard [`enter`] returned is dropped and the scope goes. A step that runs outside a
//! build (in a test of its own, say) finds no scope and fails saying so.
//!
//! # How to use it
//!
//! ```
//! use raven_ci_plugin_api::BuildScope;
//! use ravenrs_flows::{NodeContext, NodeError, NodeKind, NodeSpec, Ports};
//!
//! struct Where;
//!
//! impl NodeKind for Where {
//!     fn spec(&self) -> NodeSpec {
//!         NodeSpec::new("where", 1).input("main").output("main")
//!     }
//!     fn execute(&self, cx: &NodeContext<'_>) -> Result<Ports, NodeError> {
//!         let build = BuildScope::of(cx)?;
//!         build.log(&cx.node().id, &format!("na pasta {}", build.context().workspace.display()));
//!         Ok(BuildScope::pass_on(cx))
//!     }
//! }
//! ```

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock, PoisonError};

use ravenrs_flows::{NodeContext, NodeError, Ports};
use serde::{Deserialize, Serialize};

/// Which build a scope is of, and where.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuildContext {
    /// The job, by its path: `apps/site`.
    pub job: String,
    /// The build's number in the job.
    pub number: u64,
    /// The folder of the build: where the code is fetched and the steps run.
    pub workspace: PathBuf,
    /// The id of the build's run in the queue.
    pub run: String,
}

impl BuildContext {
    /// The last part of the job's path: the branch or pull request of a job of many branches
    /// (`main`, `PR-12`), or the job's own name.
    pub fn head(&self) -> &str {
        self.job.rsplit('/').next().unwrap_or(&self.job)
    }

    /// Where the secret files of the build's steps go while each step runs: beside its folder,
    /// `<pasta>.segredos`, never in it, so an `artifacts` glob, a `git clean` or a workspace kept
    /// by `workspace.keep` never has them. The core removes it when the build ends, however it
    /// ended, and the step that wrote a file removes it when the step ends.
    pub fn secrets_dir(&self) -> PathBuf {
        let mut name = self.workspace.as_os_str().to_owned();
        name.push(".segredos");
        PathBuf::from(name)
    }
}

/// Where the lines of a build's log go. The core's log hides every secret before it keeps a
/// line.
pub trait BuildLog: Send + Sync {
    /// Keeps `text`, a line `step` wrote.
    fn line(&self, step: &str, text: &str);

    /// Hides `value` in every line from now on: a value a step made secret (a token it got).
    fn hide(&self, value: &str);
}

/// What is undone when a build ends: a lock let go, a container removed.
type AtEnd = Box<dyn FnOnce() + Send>;

/// What a step finds of the build it runs in. See the [module](self).
pub struct BuildScope {
    context: BuildContext,
    log: Arc<dyn BuildLog>,
    environment: BTreeMap<String, String>,
    at_end: Mutex<Vec<(String, AtEnd)>>,
}

impl std::fmt::Debug for BuildScope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // The environment may hold values a log hides; only its names show.
        f.debug_struct("BuildScope")
            .field("context", &self.context)
            .field("environment", &self.environment.keys().collect::<Vec<_>>())
            .finish_non_exhaustive()
    }
}

fn scopes() -> &'static Mutex<HashMap<String, Arc<BuildScope>>> {
    static SCOPES: OnceLock<Mutex<HashMap<String, Arc<BuildScope>>>> = OnceLock::new();
    SCOPES.get_or_init(Mutex::default)
}

impl BuildScope {
    /// The scope of the build `context`, writing to `log`, whose steps get `environment`.
    pub fn new(context: BuildContext, log: Arc<dyn BuildLog>, environment: BTreeMap<String, String>) -> Self {
        Self { context, log, environment, at_end: Mutex::default() }
    }

    /// Runs `undo` when the build ends, however it ends (passed, failed, cancelled, out of
    /// time), after its post steps and publishers: what a step that leaves something behind
    /// (a lock it holds, a container, a pod) uses so nothing outlives the build. What was asked
    /// last runs first; `what` goes to the log before it runs.
    pub fn at_end(&self, what: impl Into<String>, undo: impl FnOnce() + Send + 'static) {
        self.at_end.lock().unwrap_or_else(PoisonError::into_inner).push((what.into(), Box::new(undo)));
    }

    /// Runs what [`at_end`](Self::at_end) was asked, last first, saying each in the log.
    fn end(&self) {
        let undos = std::mem::take(&mut *self.at_end.lock().unwrap_or_else(PoisonError::into_inner));
        for (what, undo) in undos.into_iter().rev() {
            self.log.line("", &format!("no fim do build: {what}"));
            undo();
        }
    }

    /// Which build it is.
    pub fn context(&self) -> &BuildContext {
        &self.context
    }

    /// The variables a step gives the programs it runs: the pipeline's `environment`, the
    /// parameters of the build, and `JOB_NAME`, `BUILD_NUMBER`, `BRANCH_NAME` and `WORKSPACE`,
    /// as a Jenkins build has them.
    pub fn environment(&self) -> &BTreeMap<String, String> {
        &self.environment
    }

    /// Writes `text` to the build's log, as a line of `step`.
    pub fn log(&self, step: &str, text: &str) {
        self.log.line(step, text);
    }

    /// Hides `value` in the build's log from now on.
    pub fn hide(&self, value: &str) {
        self.log.hide(value);
    }

    /// The scope of the build the node of `cx` runs in.
    pub fn of(cx: &NodeContext<'_>) -> Result<Arc<Self>, NodeError> {
        let run = cx.run_id().ok_or_else(|| NodeError::new("o passo rodou fora de um build"))?;
        Self::find(run)
            .ok_or_else(|| NodeError::new(format!("o run {run} não é de um build que está rodando")))
    }

    /// The scope of the build whose run is `run`, while it runs.
    pub fn find(run: &str) -> Option<Arc<Self>> {
        scopes().lock().unwrap_or_else(PoisonError::into_inner).get(run).cloned()
    }

    /// The scope of the build `number` of `job`, while it runs.
    ///
    /// **Its role.** A step finds its scope by the run of its node ([`of`](Self::of)); the points
    /// the core calls during the build with no node and no id of the run (a
    /// [`Publisher`](crate::Publisher), a [`BuildHook`](crate::BuildHook), a
    /// [`Notifier`](crate::Notifier), which get the job and the number) find it by those, to write to
    /// the log of the build (a pattern that found nothing, a report that could not be read).
    ///
    /// **How it works.** It looks among the scopes entered and not yet dropped. Outside the build
    /// (before it starts, after its guard is dropped, in a test with no build) it is `None`, and
    /// the plugin writes to the server's log with [`Host::log`](crate::Host::log) instead.
    pub fn of_build(job: &str, number: u64) -> Option<Arc<Self>> {
        scopes()
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .values()
            .find(|scope| scope.context.job == job && scope.context.number == number)
            .cloned()
    }

    /// The items that reached the node of `cx`, on its output `main`: what a step that changes
    /// nothing in them gives, so the next step gets them.
    pub fn pass_on(cx: &NodeContext<'_>) -> Ports {
        Ports::from([("main".to_owned(), cx.input("main").to_vec())])
    }
}

/// Keeps a build's scope while it is held: dropped, what the steps asked to undo at the end runs
/// and the scope goes.
#[derive(Debug)]
#[must_use = "the scope goes when the guard is dropped"]
pub struct ScopeGuard {
    run: String,
}

impl Drop for ScopeGuard {
    fn drop(&mut self) {
        let scope = scopes().lock().unwrap_or_else(PoisonError::into_inner).remove(&self.run);
        if let Some(scope) = scope {
            scope.end();
        }
    }
}

/// Makes `scope` the one the steps of its build find, until the guard is dropped. The core calls
/// it before the steps of a build run.
pub fn enter(scope: BuildScope) -> ScopeGuard {
    let run = scope.context.run.clone();
    scopes().lock().unwrap_or_else(PoisonError::into_inner).insert(run.clone(), Arc::new(scope));
    ScopeGuard { run }
}

/// A log that keeps its lines in memory, hiding what it was told to: for tests and for a step
/// run on its own.
#[derive(Debug, Default)]
pub struct MemoryLog {
    lines: Mutex<Vec<(String, String)>>,
    hidden: Mutex<Vec<String>>,
}

impl MemoryLog {
    /// A log with nothing in it.
    pub fn new() -> Self {
        Self::default()
    }

    /// Every line kept, as `(step, text)`.
    pub fn lines(&self) -> Vec<(String, String)> {
        self.lines.lock().unwrap_or_else(PoisonError::into_inner).clone()
    }

    /// The text of every line, in order.
    pub fn texts(&self) -> Vec<String> {
        self.lines().into_iter().map(|(_, text)| text).collect()
    }
}

impl BuildLog for MemoryLog {
    fn line(&self, step: &str, text: &str) {
        let hidden = self.hidden.lock().unwrap_or_else(PoisonError::into_inner).clone();
        let text = hidden.iter().fold(text.to_owned(), |text, value| text.replace(value.as_str(), "****"));
        self.lines.lock().unwrap_or_else(PoisonError::into_inner).push((step.to_owned(), text));
    }

    fn hide(&self, value: &str) {
        if !value.is_empty() {
            self.hidden.lock().unwrap_or_else(PoisonError::into_inner).push(value.to_owned());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_scope_is_found_by_its_run_while_its_guard_is_held() {
        let log = Arc::new(MemoryLog::new());
        let context = BuildContext {
            job: "repo/PR-3".into(),
            number: 4,
            workspace: PathBuf::from("/tmp/x"),
            run: "run-scope-test".into(),
        };
        assert_eq!(context.head(), "PR-3");
        let guard = enter(BuildScope::new(context, log.clone(), BTreeMap::new()));
        let scope = BuildScope::find("run-scope-test").expect("entered");
        scope.hide("segredo");
        scope.log("a", "o segredo é segredo");
        assert_eq!(log.texts(), ["o **** é ****"]);
        let undone = Arc::new(Mutex::new(Vec::new()));
        for name in ["trava", "contêiner"] {
            let undone = undone.clone();
            scope.at_end(format!("soltar {name}"), move || undone.lock().expect("lock").push(name));
        }
        drop(scope);
        drop(guard);
        assert!(BuildScope::find("run-scope-test").is_none());
        assert_eq!(*undone.lock().expect("lock"), ["contêiner", "trava"], "the last asked runs first");
        assert!(log.texts().contains(&"no fim do build: soltar trava".to_owned()));
    }

    #[test]
    fn a_scope_is_found_by_its_job_and_number_while_its_guard_is_held() {
        let log = Arc::new(MemoryLog::new());
        let context = BuildContext {
            job: "apps/publica".into(),
            number: 7,
            workspace: PathBuf::from("/tmp/x"),
            run: "run-of-build-test".into(),
        };
        assert!(BuildScope::of_build("apps/publica", 7).is_none(), "before the build");
        let guard = enter(BuildScope::new(context, log.clone(), BTreeMap::new()));
        let scope = BuildScope::of_build("apps/publica", 7).expect("entered");
        scope.log("artifacts", "dist/*.zip: nada achado");
        assert_eq!(log.texts(), ["dist/*.zip: nada achado"]);
        assert!(BuildScope::of_build("apps/publica", 8).is_none(), "another number");
        assert!(BuildScope::of_build("apps/outra", 7).is_none(), "another job");
        drop(scope);
        drop(guard);
        assert!(BuildScope::of_build("apps/publica", 7).is_none(), "after the build");
    }
}
