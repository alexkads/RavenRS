//! The life of a build around its steps: its folder, its log, fetching its code, the post steps,
//! what is taken from it, the status of its commit and the notices.
//!
//! The queue runs the steps; everything around them happens here, in this order, on the thread
//! of the executor that took the build:
//!
//! 1. **The folder**: `espacos/<job>/<number>` in the folder of data, made empty.
//! 2. **The code**: when the job's settings name an `scm`, its plugin fetches into the folder.
//!    A failure ends the build as failed before any step, with the reason.
//! 3. **Pending**: each `status` of the job's settings gets `pending` for the commit fetched;
//!    each `notify` hears that the build started.
//! 4. **The steps**: the build's scope ([`BuildScope`]) is entered under its run, so each step
//!    finds the folder, the log and the environment, and the queue runs the flow. With a time
//!    limit in the pipeline, the build is cancelled when it passes.
//! 5. **Post steps**: `always`, then `success` or `failure`, in the same folder and log.
//! 6. **Publishers**: each publisher of the pipeline takes what it finds in the folder, and the
//!    core keeps it with the build.
//! 7. **The end**: each `status` gets `success` or `failure`, each `notify` hears the end, and
//!    the folder is removed, unless the job's settings say `workspace.keep`.
//!
//! What fails in 5, 6 and 7 goes to the build's log and does not change how it ended.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{RecvTimeoutError, channel};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use raven_ci_plugin_api::{
    BuildContext, BuildEvent, BuildInfo, BuildLog, BuildScope, BuildStatus, CheckoutRequest, CiError,
    CommitState, CommitStatus, PostSteps, PostWhen, Reference, Use,
};
use ravenrs_flows::{Executor, FlowDefinition, Item, NodeStatus, Plan};
use ravenrs_flows_runtime::Runtime;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use crate::host::ServerLog;
use crate::model::{self, BuildRow, LogLine};
use crate::store::Store;
use crate::{Ci, runtime_error};

/// The folder, in the folder of data, where each build gets the folder it runs in.
pub const WORKSPACES_DIR: &str = "espacos";

/// What a pipeline said besides its flow, kept with each build so a rerun does the same.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub(crate) struct Extras {
    #[serde(default)]
    pub environment: BTreeMap<String, String>,
    #[serde(default)]
    pub post: Vec<PostRow>,
    #[serde(default)]
    pub publishers: Vec<Use>,
    #[serde(default)]
    pub timeout_ms: Option<u64>,
}

/// Post steps as kept: the flow in its JSON.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct PostRow {
    pub when: PostWhen,
    pub flow: String,
    pub start: String,
}

impl Extras {
    pub(crate) fn of(pipeline: &raven_ci_plugin_api::Pipeline) -> Self {
        Self {
            environment: pipeline.environment.clone(),
            post: pipeline
                .post
                .iter()
                .map(|post| PostRow { when: post.when, flow: post.flow.to_json(), start: post.start.clone() })
                .collect(),
            publishers: pipeline.publishers.clone(),
            timeout_ms: pipeline.timeout_ms,
        }
    }

    pub(crate) fn to_value(&self) -> Value {
        serde_json::to_value(self).unwrap_or(Value::Null)
    }

    fn from_value(value: &Value) -> Self {
        serde_json::from_value(value.clone()).unwrap_or_default()
    }

    fn post(&self) -> Vec<PostSteps> {
        self.post
            .iter()
            .filter_map(|row| {
                let flow = FlowDefinition::from_json(&row.flow).ok()?;
                Some(PostSteps { when: row.when, flow, start: row.start.clone() })
            })
            .collect()
    }
}

/// What a job's settings say to fetch: `{"scm": {"id": "git", "url": "…", "reference": {"branch": "main"}}}`.
#[derive(Clone, Debug, Deserialize)]
struct ScmUse {
    id: String,
    url: String,
    #[serde(default)]
    reference: Option<Reference>,
    #[serde(default)]
    depth: Option<u32>,
    #[serde(default)]
    credential: Option<String>,
    #[serde(default)]
    merge_onto_target: bool,
}

/// The log of one build, kept in the core's file, each line with every secret hidden.
pub(crate) struct BuildLogger {
    store: Arc<Store>,
    job: String,
    number: u64,
    next: AtomicU64,
    server: Arc<ServerLog>,
    hidden: Mutex<Vec<String>>,
}

impl BuildLogger {
    fn new(store: Arc<Store>, job: &str, number: u64, server: Arc<ServerLog>) -> Result<Self, CiError> {
        let last = model::last_log_seq(&store.db(), job, number)?;
        Ok(Self {
            store,
            job: job.to_owned(),
            number,
            next: AtomicU64::new(last + 1),
            server,
            hidden: Mutex::default(),
        })
    }

    /// A line the core writes, with no step.
    fn core(&self, text: &str) {
        self.line("", text);
    }
}

impl BuildLog for BuildLogger {
    fn line(&self, step: &str, text: &str) {
        let text = self.server.hide(text);
        let hidden = self.hidden.lock().unwrap_or_else(PoisonError::into_inner).clone();
        let text = hidden.iter().fold(text, |text, value| text.replace(value.as_str(), "****"));
        let line = LogLine {
            seq: self.next.fetch_add(1, Ordering::SeqCst),
            step: step.to_owned(),
            at_ms: self.store.now_ms(),
            text,
        };
        if let Err(e) = model::insert_log(&self.store.db(), &self.job, self.number, &line) {
            self.server.write(&format!("o log de {} #{}: {e}", self.job, self.number));
        }
    }

    fn hide(&self, value: &str) {
        if value.len() >= 3 {
            self.hidden.lock().unwrap_or_else(PoisonError::into_inner).push(value.to_owned());
        }
    }
}

/// A value of a parameter as a variable of the environment: text as it is, the rest as JSON.
fn as_variable(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

/// The list in `settings[key]`: an array of objects, or one object.
fn uses(settings: &Value, key: &str) -> Vec<Value> {
    match settings.get(key) {
        Some(Value::Array(items)) => items.clone(),
        Some(item @ Value::Object(_)) => vec![item.clone()],
        _ => Vec::new(),
    }
}

impl Ci {
    /// The folder the build `number` of `job` runs in.
    pub fn workspace(&self, job: &str, number: u64) -> PathBuf {
        self.workspaces.join(job).join(number.to_string())
    }

    /// The lines of the log of the build `number` of `job` after the line `after` (0 for all):
    /// what a reader that follows the log asks for, each time with the last `seq` it got.
    pub fn build_log(&self, job: &str, number: u64, after: u64) -> Result<Vec<LogLine>, CiError> {
        model::log_after(&self.db(), job, number, after)
    }

    /// Runs the build of `row`, which `worker` took from the queue, with everything around its
    /// steps (see the [module](self)).
    pub(crate) fn run_build(
        &self,
        runtime: &Arc<Runtime>,
        row: &BuildRow,
        worker: &str,
    ) -> Result<(), CiError> {
        let settings = self.job(&row.job).map(|job| job.settings).unwrap_or(Value::Null);
        let extras = Extras::from_value(&row.extras);
        let logger = Arc::new(BuildLogger::new(self.store.clone(), &row.job, row.number, self.log.clone())?);
        let workspace = self.workspace(&row.job, row.number);
        let keep = settings.pointer("/workspace/keep").and_then(Value::as_bool) == Some(true);
        let _ = std::fs::remove_dir_all(&workspace);
        if let Err(e) = std::fs::create_dir_all(&workspace) {
            let reason = format!("a pasta do build {}: {e}", workspace.display());
            logger.core(&reason);
            runtime.fail(&row.run_id, worker, &reason).map_err(runtime_error)?;
            return Ok(());
        }
        logger.core(&format!("build {} #{} em {}", row.job, row.number, workspace.display()));

        let context = BuildContext {
            job: row.job.clone(),
            number: row.number,
            workspace: workspace.clone(),
            run: row.run_id.clone(),
        };
        let commit = match self.fetch(&settings, &context, &logger) {
            Ok(commit) => commit,
            Err(e) => {
                let reason = format!("buscar o código: {e}");
                logger.core(&reason);
                runtime.fail(&row.run_id, worker, &reason).map_err(runtime_error)?;
                finish(&workspace, keep, &logger);
                return Ok(());
            }
        };
        let info = |status| BuildInfo {
            job: row.job.clone(),
            number: row.number,
            status,
            url: None,
            parameters: row.parameters.clone(),
            commit: commit.clone(),
        };
        self.report(&settings, &row.job, commit.as_deref(), CommitState::Pending, "rodando", &logger);
        self.notify(&settings, &BuildEvent::Started(info(BuildStatus::Running)), &logger);

        let mut environment = BTreeMap::from([
            ("JOB_NAME".to_owned(), row.job.clone()),
            ("BUILD_NUMBER".to_owned(), row.number.to_string()),
            ("BRANCH_NAME".to_owned(), context.head().to_owned()),
            ("WORKSPACE".to_owned(), workspace.display().to_string()),
        ]);
        if let Some(commit) = &commit {
            environment.insert("GIT_COMMIT".to_owned(), commit.clone());
        }
        for (name, value) in &row.parameters {
            environment.insert(name.clone(), as_variable(value));
        }
        environment.extend(extras.environment.clone());
        let guard = raven_ci_plugin_api::enter(BuildScope::new(context, logger.clone(), environment));

        let (stop_watch, watched) = channel::<()>();
        let watchdog = extras.timeout_ms.map(|ms| {
            let (runtime, run) = (runtime.clone(), row.run_id.clone());
            std::thread::spawn(move || {
                if watched.recv_timeout(Duration::from_millis(ms)) == Err(RecvTimeoutError::Timeout) {
                    let _ = runtime.cancel(&run, &format!("passou do prazo de {} s do pipeline", ms / 1000));
                }
            })
        });
        let executed = runtime.execute(&row.run_id, worker).map_err(runtime_error);
        drop(stop_watch);
        if let Some(watchdog) = watchdog {
            let _ = watchdog.join();
        }
        executed?;

        let status =
            self.build(&row.job, row.number).map(|build| build.status).unwrap_or(BuildStatus::Failed);
        let passed = status == BuildStatus::Succeeded;
        if status.ended() {
            self.post(&extras, row, passed, &logger);
            self.publish(&extras, row, &info(status), &workspace, &logger);
        }
        drop(guard);
        if status.ended() {
            let state = if passed { CommitState::Success } else { CommitState::Failure };
            self.report(&settings, &row.job, commit.as_deref(), state, status.words(), &logger);
            self.notify(&settings, &BuildEvent::Ended(info(status)), &logger);
            logger.core(&format!("terminou: {}", status.words()));
            finish(&workspace, keep, &logger);
        }
        Ok(())
    }

    /// Fetches the code the job's settings name; the commit, or `None` when they name no `scm`.
    fn fetch(
        &self,
        settings: &Value,
        context: &BuildContext,
        logger: &BuildLogger,
    ) -> Result<Option<String>, CiError> {
        let Some(scm) = settings.get("scm") else { return Ok(None) };
        let scm: ScmUse = serde_json::from_value(scm.clone())
            .map_err(|e| CiError::settings("scm", format!("não deu para ler: {e}")))?;
        let plugin = self
            .registry
            .on()
            .iter()
            .flat_map(|plugin| plugin.scms())
            .find(|candidate| candidate.id() == scm.id)
            .ok_or_else(|| {
                CiError::unavailable(format!("falta o plugin {}: o job busca o código com ele", scm.id))
            })?;
        let request = CheckoutRequest {
            url: scm.url,
            reference: scm.reference.unwrap_or_else(|| Reference::Branch(context.head().to_owned())),
            workspace: context.workspace.clone(),
            depth: scm.depth,
            credential: scm.credential,
            merge_onto_target: scm.merge_onto_target,
        };
        let checkout = plugin.checkout(&request, &|line| logger.line("checkout", line))?;
        logger.core(&format!("código no commit {}", checkout.commit));
        Ok(Some(checkout.commit))
    }

    /// Sends `state` for `commit` to each `status` of the job's settings.
    fn report(
        &self,
        settings: &Value,
        job: &str,
        commit: Option<&str>,
        state: CommitState,
        description: &str,
        logger: &BuildLogger,
    ) {
        let Some(commit) = commit else { return };
        let on = self.registry.on();
        for entry in uses(settings, "status") {
            let id = entry.get("id").and_then(Value::as_str).unwrap_or_default();
            let Some(reporter) = on.iter().flat_map(|p| p.status_reporters()).find(|r| r.id() == id) else {
                logger.core(&format!("status: falta o plugin {id}"));
                continue;
            };
            let status = CommitStatus {
                repository: entry.get("repository").and_then(Value::as_str).unwrap_or_default().to_owned(),
                commit: commit.to_owned(),
                context: entry
                    .get("context")
                    .and_then(Value::as_str)
                    .map_or_else(|| format!("raven-ci/{job}"), str::to_owned),
                state,
                description: description.to_owned(),
                url: None,
            };
            if let Err(e) = reporter.report(&status, &entry) {
                logger.core(&format!("status {id}: {e}"));
            }
        }
    }

    /// Tells each `notify` of the job's settings about `event`.
    fn notify(&self, settings: &Value, event: &BuildEvent, logger: &BuildLogger) {
        let on = self.registry.on();
        for entry in uses(settings, "notify") {
            let id = entry.get("id").and_then(Value::as_str).unwrap_or_default();
            let Some(notifier) = on.iter().flat_map(|p| p.notifiers()).find(|n| n.id() == id) else {
                logger.core(&format!("aviso: falta o plugin {id}"));
                continue;
            };
            if let Err(e) = notifier.notify(event, &entry) {
                logger.core(&format!("aviso {id}: {e}"));
            }
        }
    }

    /// Runs the post steps due after steps that `passed` (or not), in the build's scope.
    fn post(&self, extras: &Extras, row: &BuildRow, passed: bool, logger: &BuildLogger) {
        let catalog = self.registry.catalog();
        let item = start_item(&row.job, &row.parameters);
        for post in extras.post() {
            if !post.when.runs_after(passed) {
                continue;
            }
            let when = serde_json::to_value(post.when).ok().and_then(|v| v.as_str().map(str::to_owned));
            let when = when.unwrap_or_default();
            logger.core(&format!("post {when}"));
            let plan = match Plan::compile(&post.flow, &catalog) {
                Ok(plan) => plan,
                Err(errors) => {
                    let said: Vec<String> = errors.iter().map(ToString::to_string).collect();
                    logger.core(&format!("post {when}: {}", said.join("; ")));
                    continue;
                }
            };
            let run = Executor::new(&plan, &catalog)
                .run_id(row.run_id.clone())
                .start_with(post.start.clone(), vec![item.clone()])
                .run();
            for (node, part) in &run.nodes {
                if part.status == NodeStatus::Failed {
                    logger.core(&format!(
                        "post {when}: o passo {node} falhou: {}",
                        part.error.clone().unwrap_or_default()
                    ));
                }
            }
        }
    }

    /// Takes from the folder what each publisher of the pipeline finds, and keeps it.
    fn publish(
        &self,
        extras: &Extras,
        row: &BuildRow,
        info: &BuildInfo,
        workspace: &Path,
        logger: &BuildLogger,
    ) {
        let on = self.registry.on();
        for wanted in &extras.publishers {
            let Some(publisher) = on.iter().flat_map(|p| p.publishers()).find(|p| p.id() == wanted.id) else {
                logger.core(&format!(
                    "publicador {}: falta o plugin {}",
                    wanted.id,
                    wanted.plugin.as_deref().unwrap_or(&wanted.id)
                ));
                continue;
            };
            match publisher.publish(info, &wanted.settings, workspace) {
                Ok(attachments) => {
                    for attachment in attachments {
                        match self.attach(&row.job, row.number, &attachment) {
                            Ok(_) => logger.core(&format!("{}: {} guardado", wanted.id, attachment.name)),
                            Err(e) => logger.core(&format!("{}: {}: {e}", wanted.id, attachment.name)),
                        }
                    }
                }
                Err(e) => logger.core(&format!("publicador {}: {e}", wanted.id)),
            }
        }
    }
}

/// Removes the build's folder, unless the job keeps it.
fn finish(workspace: &Path, keep: bool, logger: &BuildLogger) {
    if keep {
        logger.core(&format!("a pasta {} fica, como o job pede", workspace.display()));
        return;
    }
    if let Err(e) = std::fs::remove_dir_all(workspace)
        && e.kind() != std::io::ErrorKind::NotFound
    {
        logger.core(&format!("apagar a pasta {}: {e}", workspace.display()));
    }
    // The folder of the job goes too, when no other build of it is there.
    if let Some(parent) = workspace.parent() {
        let _ = std::fs::remove_dir(parent);
    }
}

/// The item a build's flow starts with: its job, and the values of its parameters under
/// `params`, which an expression of the flow reads as `[params.E2E]`. Not `parameters`: in the
/// criteria language `[Parameters.X]` is the older spelling of `?X`, a parameter of the query.
pub(crate) fn start_item(job: &str, parameters: &Map<String, Value>) -> Item {
    Item::new(json!({ "job": job, "params": Value::Object(parameters.clone()) }))
}
