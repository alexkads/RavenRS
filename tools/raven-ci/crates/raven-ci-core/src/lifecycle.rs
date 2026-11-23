//! The life of a build around its steps: its folder, its log, fetching its code, the post steps,
//! what is taken from it, the status of its commit and the notices.
//!
//! The queue runs the steps; everything around them happens here, in this order, on the thread
//! of the executor that took the build:
//!
//! 1. **The folder**: `espacos/<job>/<number>` in the folder of data, made empty. Then each
//!    [`BuildHook`](raven_ci_plugin_api::BuildHook) of the plugins on is asked whether the build
//!    goes on; a refusal ends it as failed before any step.
//! 2. **The code**: when the job's settings name an `scm`, its plugin fetches into the folder.
//!    A failure ends the build as failed before any step, with the reason.
//! 3. **Pending**: each `status` of the job's settings gets `pending` for the commit fetched;
//!    each `notify` hears that the build started.
//! 4. **The steps**: the build's scope ([`BuildScope`]) is entered under its run, so each step
//!    finds the folder, the log and the environment, and the queue runs the flow. With a time
//!    limit in the pipeline, the build is cancelled when it passes.
//! 5. **Post steps**: `always`, then `success` or `failure`, in the same folder and log.
//! 6. **Publishers**: each publisher of the pipeline takes what it finds in the folder, and the
//!    core keeps it with the build. Then each hook hears how the build ended.
//! 7. **The end**: each `status` gets `success` or `failure`, each `notify` hears the end, and
//!    the folder is removed, unless the job's settings say `workspace.keep`.
//!
//! What fails in 5, 6 and 7 goes to the build's log and does not change how it ended.
//!
//! # Where it runs
//!
//! The same life runs on the server, for its own executors, and on a build agent (see
//! [`agents`](crate::agents)). What differs is said by a [`BuildSite`]: where the steps run and
//! where the log, what is taken and the end go. On the server the site is the queue itself; on an
//! agent, the server over HTTP. A build that its worker left halfway (an agent that fell, a server
//! that stopped) and that the same folder takes up again goes on from the code it had fetched:
//! the folder keeps a note of it beside it (`<number>.estado`), and the steps resume from the
//! checkpoint of the queue.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{RecvTimeoutError, channel};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use raven_ci_plugin_api::{
    Attachment, BuildContext, BuildEvent, BuildInfo, BuildLog, BuildScope, BuildStatus, CheckoutRequest,
    CiError, CommitState, CommitStatus, PostSteps, PostWhen, Reference, Use,
};
use ravenrs_flows::{Catalog, Executor, FlowDefinition, Item, NodeStatus, Plan};
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
    /// Where the build runs, when the pipeline says: a label, a launcher.
    #[serde(default)]
    pub agent: Option<Use>,
    /// The file of the repository the pipeline was read from, when it was (`pipeline_from`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pipeline_from: Option<crate::multibranch::PipelineFrom>,
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
            agent: pipeline.agent.clone(),
            pipeline_from: None,
        }
    }

    pub(crate) fn to_value(&self) -> Value {
        serde_json::to_value(self).unwrap_or(Value::Null)
    }

    pub(crate) fn from_value(value: &Value) -> Self {
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
    pub(crate) fn new(
        store: Arc<Store>,
        job: &str,
        number: u64,
        server: Arc<ServerLog>,
    ) -> Result<Self, CiError> {
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
}

/// A line the core writes to the log of a build, with no step.
fn core(log: &dyn BuildLog, text: &str) {
    log.line("", text);
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
pub(crate) fn uses(settings: &Value, key: &str) -> Vec<Value> {
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

    /// Runs the build of `row`, which `worker` took from the queue, here, with everything around
    /// its steps (see the [module](self)).
    pub(crate) fn run_build(
        &self,
        runtime: &Arc<Runtime>,
        row: &BuildRow,
        worker: &str,
    ) -> Result<(), CiError> {
        let order = self.order_of(row);
        let logger = Arc::new(BuildLogger::new(self.store.clone(), &row.job, row.number, self.log.clone())?);
        let site = LocalSite {
            ci: self,
            runtime: runtime.clone(),
            order: &order,
            worker,
            logger,
            commit: Mutex::default(),
        };
        self.run_order(&order, &site)
    }

    /// The order to run the build of `row`: its job's settings as they are now, and what its
    /// pipeline said.
    pub(crate) fn order_of(&self, row: &BuildRow) -> BuildOrder {
        BuildOrder {
            job: row.job.clone(),
            number: row.number,
            run: row.run_id.clone(),
            parameters: row.parameters.clone(),
            settings: self.job(&row.job).map(|job| job.settings).unwrap_or(Value::Null),
            extras: row.extras.clone(),
        }
    }

    /// Runs `order` with everything around its steps (see the [module](self)), with the plugins on
    /// here, in a folder of [`workspace`](Self::workspace); `site` says where the steps run and
    /// where the log, what is taken and the end go. What a build agent calls for each build it
    /// takes, with the server as its site.
    pub fn run_order(&self, order: &BuildOrder, site: &dyn BuildSite) -> Result<(), CiError> {
        let settings = &order.settings;
        let extras = Extras::from_value(&order.extras);
        let log = site.log();
        let workspace = self.workspace(&order.job, order.number);
        let keep = settings.pointer("/workspace/keep").and_then(Value::as_bool) == Some(true);
        let context = BuildContext {
            job: order.job.clone(),
            number: order.number,
            workspace: workspace.clone(),
            run: order.run.clone(),
        };
        let resumed = Resume::read(&workspace).filter(|resume| resume.run == order.run);
        let commit = if let Some(resume) = &resumed {
            core(
                log.as_ref(),
                &format!(
                    "retomado em {}: o código já estava aqui{}",
                    workspace.display(),
                    resume.commit.as_deref().map(|c| format!(", no commit {c}")).unwrap_or_default()
                ),
            );
            resume.commit.clone()
        } else {
            let _ = std::fs::remove_dir_all(&workspace);
            if let Err(e) = std::fs::create_dir_all(&workspace) {
                let reason = format!("a pasta do build {}: {e}", workspace.display());
                core(log.as_ref(), &reason);
                return site.fail(&reason);
            }
            core(log.as_ref(), &format!("build {} #{} em {}", order.job, order.number, workspace.display()));
            if let Some(from) = &extras.pipeline_from {
                let at = if from.commit.is_empty() { String::new() } else { format!(" no commit {}", from.commit) };
                core(log.as_ref(), &format!("pipeline: {} do repositório, em {}{at}", from.file, from.head));
            }
            if let Err(reason) = self.hooks_before(settings, &context) {
                core(log.as_ref(), &reason);
                site.fail(&reason)?;
                finish(&self.workspaces, &workspace, keep, log.as_ref());
                return Ok(());
            }
            let commit = match self.fetch(settings, &context, log.as_ref()) {
                Ok(commit) => {
                    if let (Some(from), Some(fetched)) = (&extras.pipeline_from, &commit)
                        && !from.commit.is_empty()
                        && &from.commit != fetched
                        && settings.pointer("/scm/merge_onto_target").and_then(Value::as_bool) != Some(true)
                    {
                        core(
                            log.as_ref(),
                            &format!(
                                "a branch andou entre o pedido e o build: o pipeline é o de {}, e o código é o de {fetched}",
                                from.commit
                            ),
                        );
                    }
                    commit
                }
                Err(e) => {
                    let reason = format!("buscar o código: {e}");
                    core(log.as_ref(), &reason);
                    site.fail(&reason)?;
                    finish(&self.workspaces, &workspace, keep, log.as_ref());
                    return Ok(());
                }
            };
            Resume { run: order.run.clone(), commit: commit.clone() }.write(&workspace);
            site.started(commit.as_deref());
            commit
        };
        let info = |status| BuildInfo {
            job: order.job.clone(),
            number: order.number,
            status,
            url: None,
            parameters: order.parameters.clone(),
            commit: commit.clone(),
            ..BuildInfo::default()
        };

        let mut environment = BTreeMap::from([
            ("JOB_NAME".to_owned(), order.job.clone()),
            ("BUILD_NUMBER".to_owned(), order.number.to_string()),
            ("BRANCH_NAME".to_owned(), context.head().to_owned()),
            ("WORKSPACE".to_owned(), workspace.display().to_string()),
        ]);
        if let Some(commit) = &commit {
            environment.insert("GIT_COMMIT".to_owned(), commit.clone());
        }
        for (name, value) in &order.parameters {
            environment.insert(name.clone(), as_variable(value));
        }
        environment.extend(site.environment());
        environment.extend(extras.environment.clone());
        let guard = raven_ci_plugin_api::enter(BuildScope::new(context.clone(), log.clone(), environment));

        let catalog = self.registry.catalog();
        let (stop_watch, watched) = channel::<()>();
        let status = std::thread::scope(|scope| {
            if let Some(ms) = extras.timeout_ms {
                scope.spawn(move || {
                    if watched.recv_timeout(Duration::from_millis(ms)) == Err(RecvTimeoutError::Timeout) {
                        site.cancel(&format!("passou do prazo de {} s do pipeline", ms / 1000));
                    }
                });
            }
            let status = site.steps(&catalog);
            drop(stop_watch);
            status
        })?;

        let passed = status == BuildStatus::Succeeded;
        if status.ended() {
            self.post(&extras, order, passed, log.as_ref());
            self.publish(&extras, &info(status), &workspace, site, log.as_ref());
            self.hooks_after(settings, &context, status, log.as_ref());
        }
        drop(guard);
        if status.ended() {
            site.ended(status);
            core(log.as_ref(), &format!("terminou: {}", status.words()));
            finish(&self.workspaces, &workspace, keep, log.as_ref());
        }
        Ok(())
    }

    /// The commit whose status a build reports: a pull request's own commit, which the provider
    /// shows on it (and not the merge built), else the one fetched.
    fn reported(settings: &Value, commit: Option<&str>) -> Option<String> {
        settings
            .pointer(&format!("/{}/commit", crate::multibranch::HEAD_KEY))
            .and_then(Value::as_str)
            .map(str::to_owned)
            .or_else(|| commit.map(str::to_owned))
    }

    /// The code of `order` is fetched at `commit`: its commit is pending and the notices hear the
    /// build started.
    pub(crate) fn announce_start(&self, order: &BuildOrder, commit: Option<&str>, log: &dyn BuildLog) {
        if let Some(commit) = commit
            && let Err(e) = crate::model::set_commit(&self.db(), &order.job, order.number, commit)
        {
            log.line("", &format!("guardar o commit {commit}: {e}"));
        }
        let reported = Self::reported(&order.settings, commit);
        self.report(&order.settings, &order.job, reported.as_deref(), CommitState::Pending, "rodando", log);
        let info = BuildInfo {
            job: order.job.clone(),
            number: order.number,
            status: BuildStatus::Running,
            url: self.panel_url(),
            parameters: order.parameters.clone(),
            commit: commit.map(str::to_owned),
            previous: self.previous_status(&order.job, order.number),
            ..BuildInfo::default()
        };
        self.notify(&order.settings, &BuildEvent::Started(info), log);
    }

    /// `order` ended `status`: its commit's status and the notices.
    pub(crate) fn announce_end(
        &self,
        order: &BuildOrder,
        commit: Option<&str>,
        status: BuildStatus,
        log: &dyn BuildLog,
    ) {
        let reported = Self::reported(&order.settings, commit);
        let state = match status {
            BuildStatus::Succeeded => CommitState::Success,
            // Nothing in the commit failed: the build could not be finished.
            BuildStatus::Interrupted => CommitState::Error,
            _ => CommitState::Failure,
        };
        self.report(&order.settings, &order.job, reported.as_deref(), state, status.words(), log);
        let kept = self.build(&order.job, order.number).ok();
        let duration_ms = kept.as_ref().map(|build| {
            let end = build.ended_ms.unwrap_or_else(|| self.store().now_ms());
            u64::try_from(end.saturating_sub(build.created_ms).max(0)).unwrap_or(0)
        });
        let info = BuildInfo {
            job: order.job.clone(),
            number: order.number,
            status,
            url: self.panel_url(),
            parameters: order.parameters.clone(),
            commit: commit.map(str::to_owned),
            duration_ms,
            reason: kept.and_then(|build| build.reason),
            previous: self.previous_status(&order.job, order.number),
        };
        self.notify(&order.settings, &BuildEvent::Ended(info), log);
    }

    /// How the last build of `job` before `number` that ended, ended.
    fn previous_status(&self, job: &str, number: u64) -> Option<BuildStatus> {
        let builds = self.builds(job).ok()?;
        builds
            .into_iter()
            .filter(|build| build.number < number && build.status.ended())
            .map(|build| build.status)
            .next()
    }

    /// Asks each hook of the plugins on whether the build goes on; the first refusal, in words.
    fn hooks_before(&self, settings: &Value, context: &BuildContext) -> Result<(), String> {
        for hook in self.registry.on().iter().flat_map(|plugin| plugin.build_hooks()) {
            let own = settings.get(hook.id()).cloned().unwrap_or(Value::Null);
            hook.before(context, &own).map_err(|e| format!("{}: {e}", hook.id()))?;
        }
        Ok(())
    }

    /// Tells each hook of the plugins on how the build ended.
    fn hooks_after(&self, settings: &Value, context: &BuildContext, status: BuildStatus, log: &dyn BuildLog) {
        for hook in self.registry.on().iter().flat_map(|plugin| plugin.build_hooks()) {
            let own = settings.get(hook.id()).cloned().unwrap_or(Value::Null);
            if let Err(e) = hook.after(context, status, &own) {
                core(log, &format!("{}: {e}", hook.id()));
            }
        }
    }

    /// Fetches the code the job's settings name; the commit, or `None` when they name no `scm`.
    fn fetch(
        &self,
        settings: &Value,
        context: &BuildContext,
        log: &dyn BuildLog,
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
        let checkout = plugin.checkout(&request, &|line| log.line("checkout", line))?;
        core(log, &format!("código no commit {}", checkout.commit));
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
        log: &dyn BuildLog,
    ) {
        let Some(commit) = commit else { return };
        let on = self.registry.on();
        for entry in uses(settings, "status") {
            let id = entry.get("id").and_then(Value::as_str).unwrap_or_default();
            let Some(reporter) = on.iter().flat_map(|p| p.status_reporters()).find(|r| r.id() == id) else {
                core(log, &format!("status: falta o plugin {id}"));
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
                core(log, &format!("status {id}: {e}"));
            }
        }
    }

    /// Tells each `notify` of the job's settings about `event`.
    fn notify(&self, settings: &Value, event: &BuildEvent, log: &dyn BuildLog) {
        let on = self.registry.on();
        for entry in uses(settings, "notify") {
            let id = entry.get("id").and_then(Value::as_str).unwrap_or_default();
            let Some(notifier) = on.iter().flat_map(|p| p.notifiers()).find(|n| n.id() == id) else {
                core(log, &format!("aviso: falta o plugin {id}"));
                continue;
            };
            if let Err(e) = notifier.notify(event, &entry) {
                core(log, &format!("aviso {id}: {e}"));
            }
        }
    }

    /// Runs the post steps due after steps that `passed` (or not), in the build's scope.
    fn post(&self, extras: &Extras, order: &BuildOrder, passed: bool, log: &dyn BuildLog) {
        let catalog = self.registry.catalog();
        let item = start_item(&order.job, &order.parameters);
        for post in extras.post() {
            if !post.when.runs_after(passed) {
                continue;
            }
            let when = serde_json::to_value(post.when).ok().and_then(|v| v.as_str().map(str::to_owned));
            let when = when.unwrap_or_default();
            core(log, &format!("post {when}"));
            let plan = match Plan::compile(&post.flow, &catalog) {
                Ok(plan) => plan,
                Err(errors) => {
                    let said: Vec<String> = errors.iter().map(ToString::to_string).collect();
                    core(log, &format!("post {when}: {}", said.join("; ")));
                    continue;
                }
            };
            let run = Executor::new(&plan, &catalog)
                .run_id(order.run.clone())
                .start_with(post.start.clone(), vec![item.clone()])
                .run();
            for (node, part) in &run.nodes {
                if part.status == NodeStatus::Failed {
                    core(
                        log,
                        &format!(
                            "post {when}: o passo {node} falhou: {}",
                            part.error.clone().unwrap_or_default()
                        ),
                    );
                }
            }
        }
    }

    /// Takes from the folder what each publisher of the pipeline finds, and keeps it.
    fn publish(
        &self,
        extras: &Extras,
        info: &BuildInfo,
        workspace: &Path,
        site: &dyn BuildSite,
        log: &dyn BuildLog,
    ) {
        let on = self.registry.on();
        for wanted in &extras.publishers {
            let Some(publisher) = on.iter().flat_map(|p| p.publishers()).find(|p| p.id() == wanted.id) else {
                core(
                    log,
                    &format!(
                        "publicador {}: falta o plugin {}",
                        wanted.id,
                        wanted.plugin.as_deref().unwrap_or(&wanted.id)
                    ),
                );
                continue;
            };
            match publisher.publish(info, &wanted.settings, workspace) {
                Ok(attachments) => {
                    for attachment in attachments {
                        match site.attach(&attachment) {
                            Ok(_) => core(log, &format!("{}: {} guardado", wanted.id, attachment.name)),
                            Err(e) => core(log, &format!("{}: {}: {e}", wanted.id, attachment.name)),
                        }
                    }
                }
                Err(e) => core(log, &format!("publicador {}: {e}", wanted.id)),
            }
        }
    }
}

/// Removes the build's folder, unless the job keeps it, and each folder above it that is left
/// empty, up to `root` (the folder of every build), which stays.
fn finish(root: &Path, workspace: &Path, keep: bool, log: &dyn BuildLog) {
    Resume::forget(workspace);
    if keep {
        core(log, &format!("a pasta {} fica, como o job pede", workspace.display()));
        return;
    }
    if let Err(e) = std::fs::remove_dir_all(workspace)
        && e.kind() != std::io::ErrorKind::NotFound
    {
        core(log, &format!("apagar a pasta {}: {e}", workspace.display()));
    }
    let mut above = workspace.parent();
    while let Some(folder) = above {
        if folder == root || !folder.starts_with(root) || std::fs::remove_dir(folder).is_err() {
            break;
        }
        above = folder.parent();
    }
}

/// The item a build's flow starts with: its job, and the values of its parameters under
/// `params`, which an expression of the flow reads as `[params.E2E]`. Not `parameters`: in the
/// criteria language `[Parameters.X]` is the older spelling of `?X`, a parameter of the query.
pub(crate) fn start_item(job: &str, parameters: &Map<String, Value>) -> Item {
    Item::new(json!({ "job": job, "params": Value::Object(parameters.clone()) }))
}

/// A build to run, as the side that runs it gets it: which build, the settings of its job and
/// what its pipeline said besides the flow. Plain data: it goes to an agent as JSON.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BuildOrder {
    /// The job, by its path.
    pub job: String,
    /// The build's number.
    pub number: u64,
    /// The id of its run in the queue.
    pub run: String,
    /// The values of its parameters.
    pub parameters: Map<String, Value>,
    /// The settings of its job: `scm`, `workspace`, `status`, `notify` and the hooks'.
    pub settings: Value,
    /// What its pipeline said besides the flow: environment, post steps, publishers, time limit,
    /// agent.
    pub extras: Value,
}

/// Where the steps of a build run and where what it leaves goes, for [`Ci::run_order`].
///
/// **Its role.** The life of a build is the same on the server and on an agent; what differs is
/// here. On the server the steps run in the queue and the log goes to the core's file. On an agent
/// the steps run there and everything else goes to the server, which keeps the queue, the log, the
/// attachments and the status.
///
/// **How it works.** `run_order` calls, in order: [`fail`](Self::fail) when the build cannot start
/// (no folder, a hook said no, the code could not be fetched), else [`started`](Self::started)
/// once the code is there, [`steps`](Self::steps), and [`ended`](Self::ended) once the post
/// steps, the publishers and the hooks are done. [`cancel`](Self::cancel) may come from another
/// thread at any time while the steps run: the time limit of the pipeline, a limit of the agent.
pub trait BuildSite: Sync {
    /// The build's log.
    fn log(&self) -> Arc<dyn BuildLog>;

    /// Variables the steps get besides the build's own: the limits of an agent, say.
    fn environment(&self) -> BTreeMap<String, String> {
        BTreeMap::new()
    }

    /// Ends the build as failed, saying `reason`, before any step.
    fn fail(&self, reason: &str) -> Result<(), CiError>;

    /// The code is in the folder, at `commit` when there is one.
    fn started(&self, commit: Option<&str>);

    /// Runs the steps, with the build's scope entered, with the kinds of `catalog`; how the build
    /// ended, or [`BuildStatus::Running`] when its worker lost it on the way.
    fn steps(&self, catalog: &Catalog) -> Result<BuildStatus, CiError>;

    /// Cancels the steps, saying `reason`.
    fn cancel(&self, reason: &str);

    /// Keeps what a publisher took.
    fn attach(&self, attachment: &Attachment) -> Result<(), CiError>;

    /// The build ended `status`, with its post steps, publishers and hooks done.
    fn ended(&self, status: BuildStatus);
}

/// The note beside a build's folder that says its code was fetched, for a worker that takes the
/// same build up again in the same folder.
#[derive(Debug, Serialize, Deserialize)]
struct Resume {
    run: String,
    commit: Option<String>,
}

impl Resume {
    fn file(workspace: &Path) -> PathBuf {
        workspace.with_extension("estado")
    }

    fn read(workspace: &Path) -> Option<Self> {
        if !workspace.is_dir() {
            return None;
        }
        serde_json::from_str(&std::fs::read_to_string(Self::file(workspace)).ok()?).ok()
    }

    fn write(&self, workspace: &Path) {
        if let Ok(text) = serde_json::to_string(self) {
            let _ = std::fs::write(Self::file(workspace), text);
        }
    }

    fn forget(workspace: &Path) {
        let _ = std::fs::remove_file(Self::file(workspace));
    }
}

/// The site of a build the server runs itself, on one of its executors.
struct LocalSite<'a> {
    ci: &'a Ci,
    runtime: Arc<Runtime>,
    order: &'a BuildOrder,
    worker: &'a str,
    logger: Arc<BuildLogger>,
    commit: Mutex<Option<String>>,
}

impl BuildSite for LocalSite<'_> {
    fn log(&self) -> Arc<dyn BuildLog> {
        self.logger.clone()
    }

    fn fail(&self, reason: &str) -> Result<(), CiError> {
        self.runtime.fail(&self.order.run, self.worker, reason).map_err(runtime_error)?;
        Ok(())
    }

    fn started(&self, commit: Option<&str>) {
        *self.commit.lock().unwrap_or_else(PoisonError::into_inner) = commit.map(str::to_owned);
        self.ci.announce_start(self.order, commit, self.logger.as_ref());
    }

    fn steps(&self, _: &Catalog) -> Result<BuildStatus, CiError> {
        // A step that runs for longer than the lease writes nothing; the lease is renewed while it
        // runs, so another executor does not take the build halfway.
        let (stop, stopped) = channel::<()>();
        let every = Duration::from_millis((self.runtime.lease_length() / 4).max(10));
        let executed = std::thread::scope(|scope| {
            scope.spawn(move || {
                while stopped.recv_timeout(every) == Err(RecvTimeoutError::Timeout) {
                    let _ = self.runtime.renew(&self.order.run, self.worker);
                }
            });
            let executed = self.runtime.execute(&self.order.run, self.worker).map_err(runtime_error);
            drop(stop);
            executed
        })?;
        if executed == ravenrs_flows_runtime::RunStatus::Running {
            return Ok(BuildStatus::Running);
        }
        Ok(self
            .ci
            .build(&self.order.job, self.order.number)
            .map_or(BuildStatus::Failed, |build| build.status))
    }

    fn cancel(&self, reason: &str) {
        let _ = self.runtime.cancel(&self.order.run, reason);
    }

    fn attach(&self, attachment: &Attachment) -> Result<(), CiError> {
        self.ci.attach(&self.order.job, self.order.number, attachment).map(|_| ())
    }

    fn ended(&self, status: BuildStatus) {
        let commit = self.commit.lock().unwrap_or_else(PoisonError::into_inner).clone();
        self.ci.announce_end(self.order, commit.as_deref(), status, self.logger.as_ref());
    }
}
