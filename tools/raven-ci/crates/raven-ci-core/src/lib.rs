//! The core of Raven CI: the queue, the model of jobs and builds, and the register of plugins.
//!
//! The core knows no git, no GitHub, no shell and no file format: with no plugin it starts and
//! answers, and has nothing to run. [`Ci`] puts the pieces together:
//!
//! - **The register** ([`Registry`]) holds every plugin, compiled in ([`Ci::add_plugin`]) or
//!   external ([`Ci::discover_plugins`], JSON-RPC by stdio, see [`external`]), plans them with
//!   `ravenrs_app::plan_modules`, and keeps the ones switched on started, in order. Switching
//!   one on or off needs no restart.
//! - **The model** (`model`): jobs in folders, builds numbered per job with their stages and
//!   steps, and what builds leave ([`Ci::attach`]), kept by job with [`JobSpec::keep_builds`]
//!   and [`JobSpec::keep_days`].
//! - **The queue**: a build is a run of `ravenrs-flows-runtime` in the same SQLite file, with a
//!   priority, a key that joins equal requests, cancelling and running again. The same queue
//!   serves the Raven Test Studio.
//!
//! ```
//! use std::sync::Arc;
//!
//! use raven_ci_core::{Ci, JobSpec};
//! use raven_ci_plugin_api::testing::Fake;
//! use raven_ci_plugin_api::{BuildRequest, BuildStatus, Point};
//!
//! let ci = Ci::in_memory()?;
//! ci.add_plugin(Arc::new(Fake::new(Point::Step)));
//! ci.add_plugin(Arc::new(Fake::new(Point::PipelineFormat)));
//! ci.apply_plugins()?;
//! ci.save_job(JobSpec::new("apps/site", "fake-lines", "Build: compila\nTeste: testa"))?;
//! let number = ci.request_build(BuildRequest::new("apps/site", "pedido à mão"))?.number;
//! ci.work_until_idle("executor-1")?;
//! assert_eq!(ci.build("apps/site", number)?.status, BuildStatus::Succeeded);
//! assert_eq!(ci.stages("apps/site", number)?[1].name, "Teste");
//! # Ok::<(), raven_ci_plugin_api::CiError>(())
//! ```

pub mod external;
mod host;
mod lifecycle;
mod model;
mod registry;
mod store;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, RwLock};
use std::thread::JoinHandle;
use std::time::Duration;

use raven_ci_plugin_api::{
    Attachment, BuildRequest, BuildStatus, Builds, CiError, CiPlugin, PipelineFormat, check_settings,
};
use ravenrs_app::EntityInfo;
use ravenrs_flows::{BinaryRef, Clock, FlowDefinition, SystemClock};
use ravenrs_flows_runtime::{Request, Retention, Revision, Runtime, StoreError};
use serde::Serialize;
use serde_json::Value;

pub use host::{MemorySecrets, NoSecrets, Secrets, ServerLog};
pub use lifecycle::WORKSPACES_DIR;
pub use model::{AttachmentRef, Build, Folder, JobSpec, LogLine, Stage, StepView};
pub use registry::{Memory, Origin, PluginState, PluginView, Registry, Requirement};
pub use store::{AuditEntry, PluginRow, Store};

/// The name of the core's file in its folder of data.
pub const DATABASE_FILE: &str = "raven-ci.sqlite";

/// The folder, in the folder of data, where each plugin gets a folder of its own (its files and
/// its database). External plugins are found apart, in `plugins/` and the folders given.
pub const PLUGIN_DATA_DIR: &str = "dados-dos-plugins";

/// The folder, in the folder of data, where external plugins are found: one folder per plugin,
/// each with its `plugin.toml`.
pub const PLUGINS_DIR: &str = "plugins";

/// A day, in milliseconds.
const DAY_MS: u64 = 24 * 60 * 60 * 1000;

/// What a request for a build got.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Ticket {
    /// The build's number.
    pub number: u64,
    /// It joined a build of the same job and parameters still in the queue.
    pub joined: bool,
}

/// How a [`Ci`] is made: where it keeps its files, its clock and its secrets.
pub struct CiOptions {
    /// The folder of its data; in memory when `None`.
    pub data_dir: Option<PathBuf>,
    /// The clock builds are timed and kept by.
    pub clock: Arc<dyn Clock>,
    /// Where the secrets plugins ask for come from.
    pub secrets: Arc<dyn Secrets>,
}

impl Default for CiOptions {
    fn default() -> Self {
        Self { data_dir: None, clock: Arc::new(SystemClock), secrets: Arc::new(NoSecrets) }
    }
}

impl std::fmt::Debug for CiOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CiOptions").field("data_dir", &self.data_dir).finish_non_exhaustive()
    }
}

/// The core of Raven CI.
pub struct Ci {
    store: Arc<Store>,
    registry: Registry,
    runtime: RwLock<Arc<Runtime>>,
    log: Arc<ServerLog>,
    /// One request for a build at a time, so two equal requests never make two numbers.
    requests: Mutex<()>,
    /// The folder of a core in memory, where its plugins get theirs: removed with it.
    temporary: Option<PathBuf>,
    /// Where each build gets the folder it runs in.
    workspaces: PathBuf,
}

impl Drop for Ci {
    fn drop(&mut self) {
        if let Some(dir) = &self.temporary {
            let _ = std::fs::remove_dir_all(dir);
        }
    }
}

/// A folder of its own for a core in memory: the process and a count, so two cores of one
/// process never share it.
fn memory_dir() -> PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let n = NEXT.fetch_add(1, Ordering::SeqCst);
    std::env::temp_dir().join(format!("raven-ci-memoria-{}-{n}", std::process::id()))
}

impl std::fmt::Debug for Ci {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Ci").field("registry", &self.registry).finish_non_exhaustive()
    }
}

fn runtime_error(e: StoreError) -> CiError {
    CiError::failed(format!("a fila: {e}"))
}

impl Ci {
    /// The core whose data is in `dir`: `raven-ci.sqlite`, and a folder of its own per plugin
    /// under [`PLUGIN_DATA_DIR`].
    pub fn open(dir: &Path) -> Result<Self, CiError> {
        Self::new(CiOptions { data_dir: Some(dir.to_path_buf()), ..CiOptions::default() })
    }

    /// A core that lives only while it is held: for tests and the self-test.
    pub fn in_memory() -> Result<Self, CiError> {
        Self::new(CiOptions::default())
    }

    /// The core `options` say.
    pub fn new(options: CiOptions) -> Result<Self, CiError> {
        let log = Arc::new(ServerLog::default());
        let (store, runtime, plugins_dir) = match &options.data_dir {
            Some(dir) => {
                std::fs::create_dir_all(dir)
                    .map_err(|e| CiError::failed(format!("{}: {e}", dir.display())))?;
                let file = dir.join(DATABASE_FILE);
                let store = Store::open(&file, options.clock.clone())?;
                let runtime =
                    Runtime::open(&file, ravenrs_flows::Catalog::builtin()).map_err(runtime_error)?;
                (store, runtime, dir.join(PLUGIN_DATA_DIR))
            }
            None => {
                let store = Store::in_memory(options.clock.clone())?;
                let runtime = Runtime::in_memory(ravenrs_flows::Catalog::builtin()).map_err(runtime_error)?;
                (store, runtime, memory_dir().join(PLUGIN_DATA_DIR))
            }
        };
        let temporary =
            options.data_dir.is_none().then(|| plugins_dir.parent().map(Path::to_path_buf)).flatten();
        let workspaces = plugins_dir.parent().map_or_else(
            || PathBuf::from(lifecycle::WORKSPACES_DIR),
            |base| base.join(lifecycle::WORKSPACES_DIR),
        );
        let store = Arc::new(store);
        let runtime = runtime.clock(ClockOf(options.clock.clone()));
        let registry = Registry::new(store.clone(), plugins_dir, options.secrets, log.clone());
        Ok(Self {
            store,
            registry,
            runtime: RwLock::new(Arc::new(runtime)),
            log,
            requests: Mutex::default(),
            temporary,
            workspaces,
        })
    }

    /// The register of plugins.
    pub fn registry(&self) -> &Registry {
        &self.registry
    }

    /// The core's file.
    pub fn store(&self) -> &Store {
        &self.store
    }

    /// The server's log.
    pub fn log(&self) -> &ServerLog {
        &self.log
    }

    /// Adds a plugin compiled in; it starts at the next [`apply_plugins`](Self::apply_plugins).
    pub fn add_plugin(&self, plugin: Arc<dyn CiPlugin>) {
        self.registry.add(plugin);
    }

    /// Adds the external plugins of the folders in `folder`.
    pub fn discover_plugins(&self, folder: &Path) {
        self.registry.discover(folder);
    }

    /// Puts the plugins in the state set and the queue on the steps of those on; what changed.
    pub fn apply_plugins(&self) -> Result<Vec<String>, CiError> {
        let changes = self.registry.apply()?;
        self.refresh_catalog();
        Ok(changes)
    }

    /// Switches the plugin `name` on; what changed.
    pub fn enable_plugin(&self, name: &str) -> Result<Vec<String>, CiError> {
        let changes = self.registry.enable(name)?;
        self.refresh_catalog();
        Ok(changes)
    }

    /// Switches the plugin `name` off, with those that require it; what changed.
    pub fn disable_plugin(&self, name: &str) -> Result<Vec<String>, CiError> {
        let changes = self.registry.disable(name)?;
        self.refresh_catalog();
        Ok(changes)
    }

    /// Sets the configuration of the plugin `name`; what changed.
    pub fn configure_plugin(&self, name: &str, settings: Value) -> Result<Vec<String>, CiError> {
        let changes = self.registry.configure(name, settings)?;
        self.refresh_catalog();
        Ok(changes)
    }

    fn refresh_catalog(&self) {
        let catalog = self.registry.catalog();
        let mut runtime = self.runtime.write().unwrap_or_else(|poisoned| poisoned.into_inner());
        *runtime = Arc::new(runtime.with_catalog(catalog));
    }

    fn runtime(&self) -> Arc<Runtime> {
        self.runtime.read().unwrap_or_else(|poisoned| poisoned.into_inner()).clone()
    }

    fn db(&self) -> MutexGuard<'_, rusqlite::Connection> {
        self.store.db()
    }

    /// Keeps `job`, made or changed.
    pub fn save_job(&self, job: JobSpec) -> Result<(), CiError> {
        job.check()?;
        model::save_job(&self.db(), &job, self.store.now_ms())
    }

    /// The job `name`.
    pub fn job(&self, name: &str) -> Result<JobSpec, CiError> {
        model::job(&self.db(), name)
    }

    /// Every job, by name.
    pub fn jobs(&self) -> Result<Vec<JobSpec>, CiError> {
        let names = model::job_names(&self.db())?;
        names.iter().map(|name| self.job(name)).collect()
    }

    /// Forgets the job `name`; its builds stay until the retention takes them.
    pub fn delete_job(&self, name: &str) -> Result<(), CiError> {
        model::delete_job(&self.db(), name)
    }

    /// The folders, each with its jobs, by path.
    pub fn folders(&self) -> Result<Vec<Folder>, CiError> {
        model::folders(&self.db())
    }

    /// Names the folder `path`.
    pub fn set_folder_title(&self, path: &str, title: &str) -> Result<(), CiError> {
        model::set_folder_title(&self.db(), path, title)
    }

    /// The format a job names, among the plugins on.
    fn format(&self, id: &str) -> Result<Arc<dyn PipelineFormat>, CiError> {
        self.registry
            .on()
            .iter()
            .flat_map(|plugin| plugin.pipeline_formats())
            .find(|format| format.id() == id)
            .ok_or_else(|| CiError::unavailable(format!("nenhum plugin ligado lê pipelines do formato {id}")))
    }

    /// Asks for a build; its number, and whether it joined an equal build in the queue. The
    /// pipeline is read now, with the plugins on, and every step it uses has to be there.
    pub fn request_build(&self, request: BuildRequest) -> Result<Ticket, CiError> {
        let job = self.job(&request.job)?;
        let format = self.format(&job.format)?;
        let mut pipeline = format.read(&job.name, &job.pipeline).map_err(|errors| {
            let said: Vec<String> = errors.iter().map(ToString::to_string).collect();
            CiError::failed(format!("o pipeline de {}: {}", job.name, said.join("; ")))
        })?;
        pipeline.flow.id = job.name.clone();
        let on: Vec<String> = self.registry.on().iter().map(|plugin| plugin.name().to_owned()).collect();
        for (plugin, what) in pipeline.needs() {
            if !on.contains(&plugin) {
                return Err(CiError::unavailable(format!(
                    "falta o plugin {plugin}: {what} precisa dele, e ele não está ligado"
                )));
            }
        }
        let catalog = self.registry.catalog();
        for node in &pipeline.flow.nodes {
            if catalog.get(&node.kind, node.version).is_none() {
                return Err(CiError::unavailable(format!(
                    "o passo {} usa {}, que nenhum plugin ligado oferece",
                    node.id, node.kind
                )));
            }
        }
        if !pipeline.parameters.is_empty() {
            let mut entity = EntityInfo::new("Parameters");
            entity.fields = pipeline.parameters.clone();
            check_settings(&entity, &Value::Object(request.parameters.clone())).map_err(|problems| {
                problems.into_iter().next().unwrap_or_else(|| CiError::failed("parâmetros"))
            })?;
        }
        let input = lifecycle::start_item(&job.name, &request.parameters);
        let extras = lifecycle::Extras::of(&pipeline).to_value();
        let mut run = Request::new(pipeline.flow.clone())
            .starting(&pipeline.start, vec![input])
            .priority(request.priority);
        if let Some(actor) = &request.actor {
            run = run.by(actor);
        }
        if job.coalesce {
            run = run.key(format!("{}\u{1f}{}", job.name, Value::Object(request.parameters.clone())));
        }
        let _one = self.requests.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let run_id = self.runtime().enqueue(run).map_err(runtime_error)?;
        let db = self.db();
        if let Some(existing) = model::build_by_run(&db, &run_id)? {
            return Ok(Ticket { number: existing.number, joined: true });
        }
        let number = model::next_number(&db, &job.name)?;
        model::insert_build(
            &db,
            &model::BuildRow {
                job: job.name.clone(),
                number,
                run_id,
                flow: pipeline.flow.to_json(),
                cause: request.cause,
                actor: request.actor,
                parameters: request.parameters,
                origin: None,
                created_ms: self.store.now_ms(),
                extras,
            },
        )?;
        Ok(Ticket { number, joined: false })
    }

    fn view(&self, row: model::BuildRow) -> Result<Build, CiError> {
        let runtime = self.runtime();
        let record = runtime.get(&row.run_id).map_err(runtime_error)?;
        let parts = runtime.parts(&row.run_id).map_err(runtime_error)?;
        let (status, reason) = model::status_of(&record, &parts);
        Ok(Build {
            job: row.job,
            number: row.number,
            status,
            cause: row.cause,
            actor: row.actor,
            parameters: row.parameters,
            origin: row.origin,
            created_ms: row.created_ms,
            ended_ms: record.ended_ms,
            priority: record.priority,
            joined: record.joined,
            reason,
            run_id: row.run_id,
        })
    }

    /// The build `number` of `job`.
    pub fn build(&self, job: &str, number: u64) -> Result<Build, CiError> {
        let row = model::build(&self.db(), job, number)?;
        self.view(row)
    }

    /// The builds of `job`, the newest first.
    pub fn builds(&self, job: &str) -> Result<Vec<Build>, CiError> {
        let rows = model::builds_of(&self.db(), job)?;
        rows.into_iter().map(|row| self.view(row)).collect()
    }

    /// The builds that wait, in the order they leave the queue.
    pub fn queue(&self) -> Result<Vec<Build>, CiError> {
        let records = self.runtime().queue().map_err(runtime_error)?;
        let mut builds = Vec::new();
        for record in records {
            let row = model::build_by_run(&self.db(), &record.id)?;
            if let Some(row) = row {
                builds.push(self.view(row)?);
            }
        }
        Ok(builds)
    }

    /// The stages of the build `number` of `job`, with its steps.
    pub fn stages(&self, job: &str, number: u64) -> Result<Vec<Stage>, CiError> {
        let row = model::build(&self.db(), job, number)?;
        let flow = FlowDefinition::from_json(&row.flow).map_err(|e| CiError::failed(e.to_string()))?;
        let parts = self.runtime().parts(&row.run_id).map_err(runtime_error)?;
        Ok(model::stages_of(&flow, &parts))
    }

    /// Cancels the build `number` of `job`, saying `reason`; where it is after.
    pub fn cancel(&self, job: &str, number: u64, reason: &str) -> Result<BuildStatus, CiError> {
        let row = model::build(&self.db(), job, number)?;
        self.runtime().cancel(&row.run_id, reason).map_err(runtime_error)?;
        Ok(self.build(job, number)?.status)
    }

    /// Asks for the build `number` of `job` again, with the same parameters and the same flow;
    /// the number of the new build.
    pub fn rebuild(&self, job: &str, number: u64, actor: Option<&str>) -> Result<u64, CiError> {
        let row = model::build(&self.db(), job, number)?;
        let _one = self.requests.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let run_id = self.runtime().rerun(&row.run_id, Revision::Original).map_err(runtime_error)?;
        let db = self.db();
        let next = model::next_number(&db, job)?;
        model::insert_build(
            &db,
            &model::BuildRow {
                job: job.to_owned(),
                number: next,
                run_id,
                flow: row.flow,
                cause: format!("repetição do #{number}"),
                actor: actor.map(str::to_owned),
                parameters: row.parameters,
                origin: Some(number),
                created_ms: self.store.now_ms(),
                extras: row.extras,
            },
        )?;
        Ok(next)
    }

    /// Moves the build `number` of `job` in the queue.
    pub fn set_priority(&self, job: &str, number: u64, priority: i32) -> Result<(), CiError> {
        let row = model::build(&self.db(), job, number)?;
        self.runtime().set_priority(&row.run_id, priority).map_err(runtime_error)
    }

    /// Keeps `attachment` with the build `number` of `job`; its id. The bytes go with the
    /// build when the retention takes it.
    pub fn attach(&self, job: &str, number: u64, attachment: &Attachment) -> Result<i64, CiError> {
        let row = model::build(&self.db(), job, number)?;
        let blob = self
            .runtime()
            .put_blob(&row.run_id, &attachment.mime, &attachment.bytes)
            .map_err(runtime_error)?;
        model::insert_attachment(&self.db(), job, number, attachment, &blob.location)
    }

    /// What the build `number` of `job` left.
    pub fn attachments(&self, job: &str, number: u64) -> Result<Vec<AttachmentRef>, CiError> {
        model::attachments(&self.db(), job, number)
    }

    /// The bytes of the attachment `id`.
    pub fn attachment_bytes(&self, id: i64) -> Result<Vec<u8>, CiError> {
        let (mime, size, location) = model::attachment_blob(&self.db(), id)?;
        self.runtime()
            .blob(&BinaryRef { mime, size, location })
            .map_err(runtime_error)?
            .ok_or_else(|| CiError::unavailable(format!("os bytes do anexo {id} já foram apagados")))
    }

    /// Takes the build due first and runs it, as `worker`, with everything around its steps (its
    /// folder, its code, the post steps, publishers, status and notices: see `lifecycle`); the
    /// job and number, or `None` when nothing was due. Once it ends, the job's retention runs.
    pub fn work_once(&self, worker: &str) -> Result<Option<(String, u64)>, CiError> {
        let runtime = self.runtime();
        let Some(run_id) = runtime.claim(worker).map_err(runtime_error)? else { return Ok(None) };
        let row = model::build_by_run(&self.db(), &run_id)?;
        let Some(row) = row else {
            // A run of the same queue that is not a build: the Test Studio's, say.
            runtime.execute(&run_id, worker).map_err(runtime_error)?;
            return Ok(None);
        };
        self.run_build(&runtime, &row, worker)?;
        self.prune_job(&row.job)?;
        Ok(Some((row.job, row.number)))
    }

    /// Runs whatever is due, one after another, until nothing is; how many it ran.
    pub fn work_until_idle(&self, worker: &str) -> Result<usize, CiError> {
        let mut ran = 0;
        while self.work_once(worker)?.is_some() {
            ran += 1;
        }
        Ok(ran)
    }

    /// Forgets the ended builds of `job` its retention no longer keeps, with what they left;
    /// how many.
    pub fn prune_job(&self, job: &str) -> Result<usize, CiError> {
        let Ok(spec) = self.job(job) else { return Ok(0) };
        if spec.keep_builds.is_none() && spec.keep_days.is_none() {
            return Ok(0);
        }
        let retention = Retention {
            flow_id: Some(job.to_owned()),
            keep_last: spec.keep_builds.map(|n| n as usize),
            older_than_ms: spec.keep_days.map(|days| u64::from(days) * DAY_MS),
            ..Retention::default()
        };
        // Without a number of days, every ended build past the number kept goes, however new.
        let retention = if retention.older_than_ms.is_none() {
            Retention { older_than_ms: Some(0), ..retention }
        } else {
            retention
        };
        let runtime = self.runtime();
        runtime.prune(&retention).map_err(runtime_error)?;
        let rows = model::builds_of(&self.db(), job)?;
        let mut forgotten = 0;
        for row in rows {
            if runtime.get(&row.run_id).is_err() {
                model::delete_build(&self.db(), job, row.number)?;
                forgotten += 1;
            }
        }
        Ok(forgotten)
    }

    /// Runs the retention of every job; how many builds it forgot.
    pub fn prune(&self) -> Result<usize, CiError> {
        let mut forgotten = 0;
        // The names first: a guard in the head of the loop would hold the file through it.
        let jobs = model::job_names(&self.db())?;
        for job in jobs {
            forgotten += self.prune_job(&job)?;
        }
        Ok(forgotten)
    }
}

impl Builds for Ci {
    fn request(&self, request: BuildRequest) -> Result<u64, CiError> {
        self.request_build(request).map(|ticket| ticket.number)
    }
}

/// The core's clock, handed to the queue.
struct ClockOf(Arc<dyn Clock>);

impl Clock for ClockOf {
    fn now_ms(&self) -> i64 {
        self.0.now_ms()
    }

    fn sleep_ms(&self, ms: u64) {
        self.0.sleep_ms(ms);
    }
}

/// Threads that take builds from the queue and run them, each as a worker of its own.
pub struct Executors {
    stop: Arc<AtomicBool>,
    threads: Vec<JoinHandle<()>>,
}

impl std::fmt::Debug for Executors {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Executors").field("threads", &self.threads.len()).finish()
    }
}

impl Executors {
    /// `count` executors on `ci`, looking at the queue every `poll` when it is empty.
    pub fn start(ci: Arc<Ci>, count: usize, poll: Duration) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let threads = (1..=count.max(1))
            .filter_map(|n| {
                let (ci, stop) = (ci.clone(), stop.clone());
                let name = format!("executor-{n}");
                std::thread::Builder::new()
                    .name(name.clone())
                    .spawn(move || {
                        while !stop.load(Ordering::SeqCst) {
                            match ci.work_once(&name) {
                                Ok(Some(_)) => {}
                                Ok(None) => std::thread::sleep(poll),
                                Err(e) => {
                                    ci.log().write(&format!("{name}: {e}"));
                                    std::thread::sleep(poll);
                                }
                            }
                        }
                    })
                    .ok()
            })
            .collect();
        Self { stop, threads }
    }

    /// Stops taking builds and waits for the ones running to end.
    pub fn stop(self) {
        self.stop.store(true, Ordering::SeqCst);
        for thread in self.threads {
            let _ = thread.join();
        }
    }
}
