//! Plugins written in any language: a program that speaks JSON-RPC 2.0 on its standard input and
//! output, one message per line, as MCP servers do.
//!
//! Its folder has a `plugin.toml` that says who it is and every point it offers, without starting
//! it (the whole file is [`PluginToml`]):
//!
//! ```toml
//! name = "hello"
//! version = "0.1.0"
//! summary = "Diz olá."
//! command = "python3"
//! args = ["hello.py"]
//! permissions = ["disk"]
//! timeout_ms = 10000      # each call, at most
//! idle_ms = 60000         # stopped after this long with no call
//!
//! [requires]
//! git = "^2"
//!
//! [[steps]]
//! kind = "hello"
//! title = "Olá"
//!
//! [[commands]]
//! name = "hello"
//! summary = "Diz olá no terminal."
//!
//! [[publishers]]
//! id = "relatorio"
//! ```
//!
//! The program starts at the first call, in its folder, with only the basic variables of the
//! environment. It is asked `initialize` first and must answer its name and version as the
//! manifest says. A thread reads what it says the whole time: the answer to a call goes to the
//! call that waits for it, and what it asks the host (`host/*`) is answered at once, during a call
//! or not. Each call has a time limit; past it, or when the program dies, the program is stopped
//! and the call fails with the reason, and the next call starts it again. With no call for
//! `idle_ms`, it is stopped, unless a trigger of it watches a job: that plugin stays up, and when
//! its program falls it is started again and told every job it watched. The server never goes
//! down with it.

use std::collections::{BTreeMap, HashMap};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{RecvTimeoutError, Sender, channel};
use std::sync::{Arc, Mutex, MutexGuard, Weak};
use std::time::{Duration, Instant};

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use raven_ci_plugin_api::protocol::{
    API_VERSION, AttachmentWire, CredentialDecl, FormatDecl, PROTOCOL, PipelineWire, PluginToml, RouteDecl,
    api_accepts, entity_of, error_from_wire, error_to_wire, format_errors_from_wire, http_request_to_wire,
    http_response_from_wire, method,
};
pub use raven_ci_plugin_api::protocol::{CommandDecl, StepDecl};
use raven_ci_plugin_api::{
    AgentLauncher, Attachment, BranchSource, BuildContext, BuildEvent, BuildHook, BuildInfo, BuildScope,
    BuildStatus, Builds, Checkout, CheckoutRequest, CiError, CiPlugin, CliCommand, CommitStatus,
    CredentialKind, FormatError, Head, Host, HttpResponse, LaunchRequest, LaunchedAgent, Notifier,
    Permission, Pipeline, PipelineFormat, Point, Publisher, Route, Scm, StatusReporter, Trigger,
};
use ravenrs_app::{EntityInfo, Module, ModuleManifest, Target, Version, VersionRange};
use ravenrs_flows::{CancelToken, Catalog, Item, NodeContext, NodeError, NodeKind, NodeSpec, Ports};
use ravenrs_process::{Control, End, Event, Running, Spec};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

/// The name of the file that says who an external plugin is.
pub const MANIFEST_FILE: &str = "plugin.toml";

/// How long a call may take when the manifest does not say.
const DEFAULT_TIMEOUT_MS: u64 = 30_000;

/// How long a plugin may sit with no call when the manifest does not say.
const DEFAULT_IDLE_MS: u64 = 60_000;

/// The longest line a plugin may write: a pipeline or an attachment in base64 fits.
const LINE_LIMIT: usize = 16 * 1024 * 1024;

/// How many times a minute a plugin that watches jobs is started again after it falls.
const REVIVALS_PER_MINUTE: usize = 3;

/// What the `plugin.toml` of an external plugin says, checked.
#[derive(Clone, Debug, PartialEq)]
pub struct ExternalManifest {
    /// Its name.
    pub name: String,
    /// Its version.
    pub version: Version,
    /// What it is for.
    pub summary: String,
    /// The program, by its path in the folder or its name in `PATH`.
    pub command: String,
    /// Its arguments.
    pub args: Vec<String>,
    /// What it asks the host for.
    pub permissions: Vec<Permission>,
    /// The versions it accepts of other plugins.
    pub requires: Vec<(String, VersionRange)>,
    /// Plugins that come before it, with no version asked.
    pub after: Vec<String>,
    /// The longest a call may take.
    pub timeout: Duration,
    /// How long it may sit with no call before it is stopped.
    pub idle: Duration,
    /// Its kinds of step.
    pub steps: Vec<StepDecl>,
    /// Its subcommands.
    pub commands: Vec<CommandDecl>,
    /// The version of `raven-ci-plugin-api` it was built with, for a plugin compiled from Rust.
    pub api: Option<String>,
    /// The whole file, with every point it declares.
    pub file: PluginToml,
}

/// Reads and checks the `plugin.toml` in `folder`, saying every problem it finds.
pub fn read_manifest(folder: &Path) -> Result<ExternalManifest, String> {
    let path = folder.join(MANIFEST_FILE);
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let file: PluginToml = toml::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    let problems = file.check();
    if !problems.is_empty() {
        return Err(format!("{}: {}", path.display(), problems.join("; ")));
    }
    let version = Version::parse(&file.version).map_err(|e| format!("{}: a versão: {e}", path.display()))?;
    let permissions = file.permissions.iter().filter_map(|name| Permission::parse(name)).collect();
    let requires = file
        .requires
        .iter()
        .filter_map(|(plugin, range)| {
            VersionRange::compatible(range.trim_start_matches('^')).ok().map(|range| (plugin.clone(), range))
        })
        .collect();
    let command = match (&file.windows_command, cfg!(windows)) {
        (Some(command), true) => command.clone(),
        _ => file.command.clone(),
    };
    Ok(ExternalManifest {
        name: file.name.clone(),
        version,
        summary: file.summary.clone(),
        command,
        args: file.args.clone(),
        permissions,
        requires,
        after: file.after.clone(),
        timeout: Duration::from_millis(file.timeout_ms.unwrap_or(DEFAULT_TIMEOUT_MS).max(1)),
        idle: Duration::from_millis(file.idle_ms.unwrap_or(DEFAULT_IDLE_MS).max(1)),
        steps: file.steps.clone(),
        commands: file.commands.clone(),
        api: file.api.clone(),
        file,
    })
}

/// A plugin whose code is a program apart, reached by JSON-RPC.
pub struct ExternalPlugin {
    manifest: ExternalManifest,
    folder: PathBuf,
    link: Arc<Link>,
}

impl std::fmt::Debug for ExternalPlugin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExternalPlugin")
            .field("name", &self.manifest.name)
            .field("folder", &self.folder)
            .finish()
    }
}

impl ExternalPlugin {
    /// The plugin in `folder`, as its `plugin.toml` says. Nothing starts until the first call.
    pub fn open(folder: &Path) -> Result<Self, String> {
        let manifest = read_manifest(folder)?;
        let folder = folder.canonicalize().unwrap_or_else(|_| folder.to_path_buf());
        let link = Arc::new(Link {
            name: manifest.name.clone(),
            version: manifest.version,
            command: manifest.command.clone(),
            args: manifest.args.clone(),
            folder: folder.clone(),
            timeout: manifest.timeout,
            idle: manifest.idle,
            process: Mutex::default(),
            pending: Mutex::default(),
            host: Mutex::default(),
            watches: Mutex::default(),
            revivals: Mutex::default(),
            next_id: AtomicU64::new(1),
            starts: AtomicU64::new(0),
        });
        Ok(Self { manifest, folder, link })
    }

    /// What its `plugin.toml` says.
    pub fn manifest(&self) -> &ExternalManifest {
        &self.manifest
    }

    /// Its folder.
    pub fn folder(&self) -> &Path {
        &self.folder
    }

    /// The points its manifest offers, in the order of [`Point::ALL`].
    pub fn declared_points(&self) -> Vec<Point> {
        let file = &self.manifest.file;
        let offers = [
            (Point::Step, !file.steps.is_empty()),
            (Point::Scm, !file.scms.is_empty()),
            (Point::BranchSource, !file.branch_sources.is_empty()),
            (Point::Trigger, !file.triggers.is_empty()),
            (Point::Publisher, !file.publishers.is_empty()),
            (Point::StatusReporter, !file.status_reporters.is_empty()),
            (Point::Notifier, !file.notifiers.is_empty()),
            (Point::CredentialKind, !file.credential_kinds.is_empty()),
            (Point::AgentLauncher, !file.agent_launchers.is_empty()),
            (Point::PipelineFormat, !file.pipeline_formats.is_empty()),
            (Point::Routes, !file.routes.is_empty()),
            (Point::CliCommand, !file.commands.is_empty()),
            (Point::BuildHook, !file.build_hooks.is_empty()),
        ];
        Point::ALL
            .into_iter()
            .filter(|point| offers.iter().any(|(offered, yes)| offered == point && *yes))
            .collect()
    }

    /// The process id of its program, while it runs.
    pub fn pid(&self) -> Option<u32> {
        self.link.slot().as_ref().map(|process| process.control.pid())
    }

    /// The memory its program holds, while it runs and the system says it.
    pub fn memory(&self) -> Option<u64> {
        self.pid().and_then(ravenrs_metrics::rss_bytes_of)
    }

    /// How many times its program was started.
    pub fn starts(&self) -> u64 {
        self.link.starts.load(Ordering::SeqCst)
    }

    /// The jobs its triggers watch now, as `(trigger, job)`.
    pub fn watched(&self) -> Vec<(String, String)> {
        lock(&self.link.watches).keys().cloned().collect()
    }

    /// Calls `method` with `params`, starting the program when it is not running.
    pub fn call(&self, method: &str, params: Value) -> Result<Value, CiError> {
        self.link.call(method, params, &Call::default())
    }
}

impl Module for ExternalPlugin {
    fn name(&self) -> &str {
        &self.manifest.name
    }

    fn dependencies(&self) -> Vec<String> {
        self.manifest.after.clone()
    }

    fn manifest(&self) -> ModuleManifest {
        let mut manifest = ModuleManifest::new().target(Target::Server);
        manifest.version = Some(self.manifest.version);
        manifest.requires = self.manifest.requires.clone();
        manifest.capabilities = self.manifest.permissions.iter().map(|p| p.capability().to_owned()).collect();
        manifest
    }
}

impl CiPlugin for ExternalPlugin {
    fn summary(&self) -> String {
        self.manifest.summary.clone()
    }

    fn config(&self) -> Option<EntityInfo> {
        let fields = &self.manifest.file.config;
        if fields.is_empty() { None } else { entity_of(&self.manifest.name, fields).ok() }
    }

    fn start(&self, host: Arc<dyn Host>) -> Result<(), CiError> {
        *lock(&self.link.host) = Some(host);
        Ok(())
    }

    fn stop(&self) {
        // With no host, a program that ends now is not started again.
        *lock(&self.link.host) = None;
        lock(&self.link.watches).clear();
        self.link.shut_down();
    }

    fn steps(&self, catalog: &mut Catalog) {
        for step in &self.manifest.steps {
            catalog.add(ExternalStep { decl: step.clone(), link: self.link.clone() });
        }
    }

    fn cli_commands(&self) -> Vec<Arc<dyn CliCommand>> {
        self.manifest
            .commands
            .iter()
            .map(|decl| {
                Arc::new(ExternalCommand { decl: decl.clone(), link: self.link.clone() })
                    as Arc<dyn CliCommand>
            })
            .collect()
    }

    fn scms(&self) -> Vec<Arc<dyn Scm>> {
        self.manifest.file.scms.iter().map(|d| Arc::new(self.point(&d.id)) as Arc<dyn Scm>).collect()
    }

    fn branch_sources(&self) -> Vec<Arc<dyn BranchSource>> {
        self.manifest
            .file
            .branch_sources
            .iter()
            .map(|d| Arc::new(self.point(&d.id)) as Arc<dyn BranchSource>)
            .collect()
    }

    fn triggers(&self) -> Vec<Arc<dyn Trigger>> {
        self.manifest.file.triggers.iter().map(|d| Arc::new(self.point(&d.id)) as Arc<dyn Trigger>).collect()
    }

    fn publishers(&self) -> Vec<Arc<dyn Publisher>> {
        self.manifest
            .file
            .publishers
            .iter()
            .map(|d| Arc::new(self.point(&d.id)) as Arc<dyn Publisher>)
            .collect()
    }

    fn status_reporters(&self) -> Vec<Arc<dyn StatusReporter>> {
        self.manifest
            .file
            .status_reporters
            .iter()
            .map(|d| Arc::new(self.point(&d.id)) as Arc<dyn StatusReporter>)
            .collect()
    }

    fn notifiers(&self) -> Vec<Arc<dyn Notifier>> {
        self.manifest
            .file
            .notifiers
            .iter()
            .map(|d| Arc::new(self.point(&d.id)) as Arc<dyn Notifier>)
            .collect()
    }

    fn credential_kinds(&self) -> Vec<Arc<dyn CredentialKind>> {
        self.manifest
            .file
            .credential_kinds
            .iter()
            .filter_map(|decl| {
                entity_of(&decl.id, &decl.fields).ok()?;
                Some(Arc::new(ExternalCredential { decl: decl.clone(), link: self.link.clone() })
                    as Arc<dyn CredentialKind>)
            })
            .collect()
    }

    fn agent_launchers(&self) -> Vec<Arc<dyn AgentLauncher>> {
        self.manifest
            .file
            .agent_launchers
            .iter()
            .map(|d| Arc::new(self.point(&d.id)) as Arc<dyn AgentLauncher>)
            .collect()
    }

    fn pipeline_formats(&self) -> Vec<Arc<dyn PipelineFormat>> {
        self.manifest
            .file
            .pipeline_formats
            .iter()
            .map(|decl| {
                Arc::new(ExternalFormat { decl: decl.clone(), link: self.link.clone() })
                    as Arc<dyn PipelineFormat>
            })
            .collect()
    }

    fn routes(&self) -> Vec<Route> {
        self.manifest.file.routes.iter().map(|decl| route(decl, &self.link)).collect()
    }

    fn build_hooks(&self) -> Vec<Arc<dyn BuildHook>> {
        self.manifest
            .file
            .build_hooks
            .iter()
            .map(|d| Arc::new(self.point(&d.id)) as Arc<dyn BuildHook>)
            .collect()
    }
}

impl ExternalPlugin {
    fn point(&self, id: &str) -> ExternalPoint {
        ExternalPoint { id: id.to_owned(), link: self.link.clone() }
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// The program of a plugin, running. Its output is read by a thread of its own.
struct Process {
    control: Control,
    generation: u64,
    last_call: Instant,
}

/// What the reader of a program hands to the call that waits.
enum Incoming {
    /// A line for the log of the build of the call.
    Log { step: String, line: String },
    /// A value the log of the build of the call hides from now on.
    Hide(String),
    /// The answer.
    Answer(Value),
    /// The plugin answered with an error.
    Refused(Value),
    /// The program ended.
    Ended(End),
}

/// A call that waits for its answer.
struct Pending {
    generation: u64,
    tx: Sender<Incoming>,
}

/// A job a trigger of the plugin watches, told again when its program starts again.
struct Watch {
    settings: Value,
    builds: Arc<dyn Builds>,
}

/// Where a call hands a line its program wrote for the log of the build: `(step, line)`.
type LineSink<'a> = &'a dyn Fn(&str, &str);

/// What a call does with what comes while it waits: its node's cancelling, the log of its build.
#[derive(Default)]
struct Call<'a> {
    token: Option<&'a CancelToken>,
    log: Option<LineSink<'a>>,
    hide: Option<&'a dyn Fn(&str)>,
}

/// How the core reaches the program of a plugin.
struct Link {
    name: String,
    version: Version,
    command: String,
    args: Vec<String>,
    folder: PathBuf,
    timeout: Duration,
    idle: Duration,
    process: Mutex<Option<Process>>,
    pending: Mutex<HashMap<u64, Pending>>,
    host: Mutex<Option<Arc<dyn Host>>>,
    watches: Mutex<BTreeMap<(String, String), Watch>>,
    revivals: Mutex<Vec<Instant>>,
    next_id: AtomicU64,
    starts: AtomicU64,
}

impl Drop for Link {
    fn drop(&mut self) {
        self.shut_down();
    }
}

/// Why a call did not get an answer.
enum Broken {
    /// The plugin answered with an error: it stays up.
    Answered(CiError),
    /// The program has to go: it timed out, died, or broke the protocol.
    Gone(String),
    /// The node was cancelled.
    Cancelled(String),
}

impl Link {
    fn slot(&self) -> MutexGuard<'_, Option<Process>> {
        lock(&self.process)
    }

    fn host(&self) -> Option<Arc<dyn Host>> {
        lock(&self.host).clone()
    }

    fn log(&self, line: &str) {
        if let Some(host) = self.host() {
            host.log(line);
        }
    }

    /// Calls `method`, starting the program when it is not running.
    fn call(self: &Arc<Self>, method: &str, params: Value, call: &Call<'_>) -> Result<Value, CiError> {
        let (control, generation) = self.running().map_err(CiError::failed)?;
        let result = self.request(&control, generation, method, params, self.timeout, call);
        self.touch(generation);
        match result {
            Ok(value) => Ok(value),
            Err(Broken::Answered(error)) => Err(error),
            Err(Broken::Gone(why)) => {
                self.kill(generation);
                self.log(&why);
                Err(CiError::failed(why))
            }
            Err(Broken::Cancelled(why)) => {
                self.kill(generation);
                Err(CiError::failed(why))
            }
        }
    }

    /// The program that runs now, started when there is none, and told again every job its
    /// triggers watched.
    fn running(self: &Arc<Self>) -> Result<(Control, u64), String> {
        let mut slot = self.slot();
        if let Some(process) = slot.as_ref() {
            return Ok((process.control.clone(), process.generation));
        }
        let process = self.spawn()?;
        let running = (process.control.clone(), process.generation);
        *slot = Some(process);
        drop(slot);
        self.rewatch(&running.0, running.1);
        Ok(running)
    }

    /// Tells the program of `generation` every job its triggers watch.
    fn rewatch(&self, control: &Control, generation: u64) {
        let watches: Vec<(String, String, Value)> = lock(&self.watches)
            .iter()
            .map(|((trigger, job), watch)| (trigger.clone(), job.clone(), watch.settings.clone()))
            .collect();
        for (trigger, job, settings) in watches {
            let params = json!({ "id": trigger, "job": job, "settings": settings });
            match self.request(
                control,
                generation,
                method::TRIGGER_WATCH,
                params,
                self.timeout,
                &Call::default(),
            ) {
                Ok(_) => self.log(&format!("o gatilho {trigger} volta a vigiar {job}")),
                Err(broken) => {
                    let why = match broken {
                        Broken::Answered(e) => e.to_string(),
                        Broken::Gone(why) | Broken::Cancelled(why) => why,
                    };
                    self.log(&format!("o gatilho {trigger} não voltou a vigiar {job}: {why}"));
                }
            }
        }
    }

    /// Starts the program, with a thread that reads it, asks `initialize` and checks who
    /// answered; then leaves a watcher that stops it after `idle` with no call.
    fn spawn(self: &Arc<Self>) -> Result<Process, String> {
        let program = if Path::new(&self.command).components().count() > 1 {
            self.folder.join(&self.command).into_os_string()
        } else {
            self.command.clone().into()
        };
        let running = Spec::new(program)
            .args(&self.args)
            .dir(&self.folder)
            .env("RAVEN_CI_PLUGIN", &self.name)
            .stdin()
            .line_limit(LINE_LIMIT)
            .spawn()
            .map_err(|e| format!("o plugin {} não subiu ({}): {e}", self.name, self.command))?;
        let generation = self.starts.fetch_add(1, Ordering::SeqCst) + 1;
        let control = running.control();
        self.listen(running, control.clone(), generation);
        let hello =
            json!({ "server": "raven-ci", "plugin": self.name, "protocol": PROTOCOL, "api": API_VERSION });
        let said = match self.request(
            &control,
            generation,
            method::INITIALIZE,
            hello,
            self.timeout,
            &Call::default(),
        ) {
            Ok(said) => said,
            Err(broken) => {
                control.stop();
                let why = match broken {
                    Broken::Answered(e) => e.to_string(),
                    Broken::Gone(why) | Broken::Cancelled(why) => why,
                };
                return Err(format!("o plugin {} não respondeu ao initialize: {why}", self.name));
            }
        };
        let name = said.get("name").and_then(Value::as_str).unwrap_or_default();
        let version = said.get("version").and_then(Value::as_str).and_then(|v| Version::parse(v).ok());
        if name != self.name || version != Some(self.version) {
            control.stop();
            return Err(format!(
                "o programa diz ser {name} {}, e o plugin.toml diz {} {}",
                said.get("version").and_then(Value::as_str).unwrap_or("sem versão"),
                self.name,
                self.version
            ));
        }
        if let Some(api) = said.get("api").and_then(Value::as_str)
            && let Err(why) = api_accepts(api)
        {
            control.stop();
            return Err(format!("o plugin {}: {why}", self.name));
        }
        self.log(&format!("subiu (processo {})", control.pid()));
        self.watch_idle(generation);
        Ok(Process { control, generation, last_call: Instant::now() })
    }

    /// Reads the program of `generation` until it ends: answers go to their calls, and what it
    /// asks the host is answered on a thread of its own, so a host that asks the plugin back never
    /// waits on this reader.
    fn listen(self: &Arc<Self>, running: Running, control: Control, generation: u64) {
        let link: Weak<Self> = Arc::downgrade(self);
        let _ = std::thread::Builder::new().name(format!("raven-ci-leitor-{}", self.name)).spawn(move || {
            for event in running.events() {
                let Some(link) = link.upgrade() else {
                    control.stop();
                    return;
                };
                match event {
                    Event::Out(line) => link.incoming(&line, &control),
                    Event::Err(line) => link.log(&line),
                    Event::Ended(end) => link.ended(generation, end),
                }
            }
        });
    }

    /// One line the program wrote.
    fn incoming(self: &Arc<Self>, line: &str, control: &Control) {
        let Ok(message) = serde_json::from_str::<Value>(line) else {
            self.log(&format!("linha que não é JSON-RPC: {line}"));
            return;
        };
        let params = message.get("params").cloned().unwrap_or(Value::Null);
        let text = |key: &str| params.get(key).and_then(Value::as_str).unwrap_or_default().to_owned();
        let to_call = |incoming: Incoming| {
            if let Some(call) = params.get("call").and_then(Value::as_u64)
                && let Some(pending) = lock(&self.pending).get(&call)
            {
                let _ = pending.tx.send(incoming);
            }
        };
        match message.get("method").and_then(Value::as_str) {
            Some(method::HOST_LOG) if message.get("id").is_none() => {
                if let Some(host) = self.host() {
                    host.log(&text("line"));
                }
            }
            Some(method::HOST_BUILD_LOG) => to_call(Incoming::Log { step: text("step"), line: text("line") }),
            Some(method::HOST_HIDE) => to_call(Incoming::Hide(text("value"))),
            Some(asked) => {
                let Some(their_id) = message.get("id").cloned() else { return };
                let asked = asked.to_owned();
                let link = self.clone();
                let control = control.clone();
                let _ = std::thread::Builder::new().name(format!("raven-ci-host-{}", self.name)).spawn(
                    move || {
                        let reply = match link.serve(&asked, &params) {
                            Ok(result) => json!({ "jsonrpc": "2.0", "id": their_id, "result": result }),
                            Err(e) => json!({ "jsonrpc": "2.0", "id": their_id, "error": error_to_wire(&e) }),
                        };
                        control.send(&reply.to_string());
                    },
                );
            }
            None => {
                let Some(id) = message.get("id").and_then(Value::as_u64) else { return };
                let incoming = match message.get("error") {
                    Some(error) => Incoming::Refused(error.clone()),
                    None => Incoming::Answer(message.get("result").cloned().unwrap_or(Value::Null)),
                };
                if let Some(pending) = lock(&self.pending).get(&id) {
                    let _ = pending.tx.send(incoming);
                }
            }
        }
    }

    /// The program of `generation` ended: its calls fail, and a plugin whose triggers watch jobs
    /// is started again.
    fn ended(self: &Arc<Self>, generation: u64, end: End) {
        for pending in lock(&self.pending).values().filter(|p| p.generation == generation) {
            let _ = pending.tx.send(Incoming::Ended(end));
        }
        let fell = {
            let mut slot = self.slot();
            let fell = slot.as_ref().is_some_and(|process| process.generation == generation);
            if fell {
                *slot = None;
            }
            fell
        };
        if fell {
            self.log(&format!("o programa terminou sozinho, {}", how(end)));
        }
        self.revive();
    }

    /// Starts the program again, in a moment, when its triggers watch jobs and the plugin is on;
    /// at most [`REVIVALS_PER_MINUTE`] times a minute, after which the next call starts it.
    fn revive(self: &Arc<Self>) {
        if lock(&self.watches).is_empty() || self.host().is_none() || self.slot().is_some() {
            return;
        }
        {
            let mut revivals = lock(&self.revivals);
            revivals.retain(|at| at.elapsed() < Duration::from_secs(60));
            if revivals.len() >= REVIVALS_PER_MINUTE {
                drop(revivals);
                self.log("caiu vezes demais em um minuto: volta na próxima chamada");
                return;
            }
            revivals.push(Instant::now());
        }
        let link = Arc::downgrade(self);
        let _ = std::thread::Builder::new().name(format!("raven-ci-volta-{}", self.name)).spawn(move || {
            std::thread::sleep(Duration::from_millis(200));
            let Some(link) = link.upgrade() else { return };
            if lock(&link.watches).is_empty() || link.host().is_none() {
                return;
            }
            if let Err(why) = link.running() {
                link.log(&format!("não voltou: {why}"));
            }
        });
    }

    /// Notes that the program of `generation` was just called.
    fn touch(&self, generation: u64) {
        if let Some(process) = self.slot().as_mut()
            && process.generation == generation
        {
            process.last_call = Instant::now();
        }
    }

    /// Stops the program of `generation`, if it is still the one running.
    fn kill(&self, generation: u64) {
        let process = {
            let mut slot = self.slot();
            if slot.as_ref().is_some_and(|process| process.generation == generation) {
                slot.take()
            } else {
                None
            }
        };
        if let Some(process) = process {
            process.control.stop();
        }
    }

    /// Stops the program of `generation` once it sits with no call for `idle`, unless a call waits
    /// or a trigger of it watches a job.
    fn watch_idle(self: &Arc<Self>, generation: u64) {
        let link: Weak<Self> = Arc::downgrade(self);
        let tick = (self.idle / 4).clamp(Duration::from_millis(10), Duration::from_secs(1));
        let _ = std::thread::Builder::new().name(format!("raven-ci-ocioso-{}", self.name)).spawn(move || {
            loop {
                std::thread::sleep(tick);
                let Some(link) = link.upgrade() else { return };
                if !lock(&link.watches).is_empty()
                    || lock(&link.pending).values().any(|pending| pending.generation == generation)
                {
                    continue;
                }
                let mut slot = link.slot();
                let Some(process) = slot.as_ref() else { return };
                if process.generation != generation {
                    return;
                }
                if process.last_call.elapsed() >= link.idle {
                    let _ = process
                        .control
                        .send(&json!({ "jsonrpc": "2.0", "method": method::SHUTDOWN }).to_string());
                    process.control.stop();
                    *slot = None;
                    drop(slot);
                    link.log("parado por ficar ocioso");
                    return;
                }
            }
        });
    }

    /// Asks the program to end and stops it.
    fn shut_down(&self) {
        if let Some(process) = self.slot().take() {
            let _ =
                process.control.send(&json!({ "jsonrpc": "2.0", "method": method::SHUTDOWN }).to_string());
            process.control.stop();
        }
    }

    /// Sends one request to the program of `generation` and waits for its answer, handing what
    /// comes for the call (lines of the build's log) to `call`.
    fn request(
        &self,
        control: &Control,
        generation: u64,
        method: &str,
        params: Value,
        limit: Duration,
        call: &Call<'_>,
    ) -> Result<Value, Broken> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = channel();
        lock(&self.pending).insert(id, Pending { generation, tx });
        let result = self.wait(control, id, &rx, method, params, limit, call);
        lock(&self.pending).remove(&id);
        result
    }

    #[allow(clippy::too_many_arguments, reason = "the parts of one call, kept apart from its bookkeeping")]
    fn wait(
        &self,
        control: &Control,
        id: u64,
        rx: &std::sync::mpsc::Receiver<Incoming>,
        method: &str,
        params: Value,
        limit: Duration,
        call: &Call<'_>,
    ) -> Result<Value, Broken> {
        let request = json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params });
        if !control.send(&request.to_string()) {
            return Err(Broken::Gone(format!("o plugin {} fechou a entrada", self.name)));
        }
        let deadline = Instant::now() + limit;
        loop {
            if let Some(token) = call.token
                && token.is_cancelled()
            {
                return Err(Broken::Cancelled(token.reason().unwrap_or_else(|| "cancelado".to_owned())));
            }
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return Err(Broken::Gone(format!(
                    "o plugin {} não respondeu a {method} em {} ms",
                    self.name,
                    limit.as_millis()
                )));
            }
            match rx.recv_timeout(left.min(Duration::from_millis(50))) {
                Ok(Incoming::Answer(value)) => return Ok(value),
                Ok(Incoming::Refused(error)) => return Err(Broken::Answered(self.refusal(&error))),
                Ok(Incoming::Ended(end)) => {
                    return Err(Broken::Gone(format!(
                        "o plugin {} terminou no meio de {method}, {}",
                        self.name,
                        how(end)
                    )));
                }
                Ok(Incoming::Log { step, line }) => match call.log {
                    Some(log) => log(&step, &line),
                    None => self.log(&line),
                },
                Ok(Incoming::Hide(value)) => {
                    if let Some(hide) = call.hide {
                        hide(&value);
                    }
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => {
                    return Err(Broken::Gone(format!("o plugin {} parou de responder", self.name)));
                }
            }
        }
    }

    /// The error the plugin answered, as the core says it: a refusal of the host or a setting
    /// stays what it is, anything else is said as the plugin's.
    fn refusal(&self, error: &Value) -> CiError {
        match error_from_wire(error) {
            CiError::Failed(said) => CiError::failed(format!("o plugin {}: {said}", self.name)),
            CiError::Unavailable(said) => CiError::unavailable(format!("o plugin {}: {said}", self.name)),
            other => other,
        }
    }

    /// The build a trigger of the plugin asks for, or the scan, through the [`Builds`] of the job
    /// it watches.
    fn builds_of(&self, params: &Value) -> Result<Arc<dyn Builds>, CiError> {
        let trigger = params.get("trigger").and_then(Value::as_str).unwrap_or_default().to_owned();
        let job = params.get("watched").and_then(Value::as_str).unwrap_or_default().to_owned();
        lock(&self.watches)
            .get(&(trigger.clone(), job.clone()))
            .map(|watch| watch.builds.clone())
            .ok_or_else(|| CiError::unavailable(format!("o gatilho {trigger} não vigia o job {job}")))
    }

    /// Answers what the program asks the host, checking the permission of each service.
    fn serve(&self, method: &str, params: &Value) -> Result<Value, CiError> {
        let Some(host) = self.host() else { return Err(CiError::unavailable("o plugin está desligado")) };
        let text = |key: &str| params.get(key).and_then(Value::as_str).unwrap_or_default().to_owned();
        match method {
            method::HOST_LOG => {
                host.log(&text("line"));
                Ok(Value::Null)
            }
            method::HOST_SETTINGS => Ok(host.settings()),
            method::HOST_SECRET => host.secret(&text("name")).map(Value::String),
            method::HOST_DATA_DIR => host.data_dir().map(|dir| json!(dir.display().to_string())),
            method::HOST_DATABASE => host.database().map(|file| json!(file.display().to_string())),
            method::HOST_GRANTED => Ok(json!(host.granted().iter().map(|p| p.name()).collect::<Vec<_>>())),
            method::HOST_REQUIRE => {
                let permission = Permission::parse(&text("permission")).ok_or_else(|| {
                    CiError::failed(format!("a permissão {:?} não existe", text("permission")))
                })?;
                host.require(permission, &text("what")).map(|()| Value::Null)
            }
            method::HOST_REQUEST_BUILD => {
                let request = serde_json::from_value(params.get("request").cloned().unwrap_or(Value::Null))
                    .map_err(|e| CiError::failed(format!("o pedido de build: {e}")))?;
                self.builds_of(params)?.request(request).map(|number| json!(number))
            }
            method::HOST_SCAN => self.builds_of(params)?.scan(&text("job")).map(|said| json!(said)),
            method::HOST_ATTACHMENTS => {
                let number = params.get("number").and_then(Value::as_u64);
                host.attachments(&text("job"), number, &text("kind")).map(|list| json!(list))
            }
            method::HOST_ATTACHMENT => {
                let id = params.get("id").and_then(Value::as_i64).unwrap_or_default();
                host.attachment(id)
                    .map(|(info, bytes)| json!({ "info": info, "base64": STANDARD.encode(bytes) }))
            }
            other => Err(CiError::failed(format!("o host não tem o método {other}"))),
        }
    }
}

/// How a program ended, in words.
fn how(end: End) -> String {
    match end {
        End::Code(code) => format!("com o código {code}"),
        End::Signal(signal) => format!("pelo sinal {signal}"),
        End::TimedOut | End::Stopped => "parado".to_owned(),
    }
}

/// `value` read as `T`, or a failure saying what the plugin of `link` answered wrong.
fn parse<T: DeserializeOwned>(link: &Link, method: &str, value: Value) -> Result<T, CiError> {
    serde_json::from_value(value).map_err(|e| {
        CiError::failed(format!("o plugin {} respondeu {method} num formato errado: {e}", link.name))
    })
}

/// A point of an external plugin known by its id: each method of its trait is a call.
struct ExternalPoint {
    id: String,
    link: Arc<Link>,
}

impl Scm for ExternalPoint {
    fn id(&self) -> &str {
        &self.id
    }

    fn checkout(&self, request: &CheckoutRequest, log: &dyn Fn(&str)) -> Result<Checkout, CiError> {
        let to_log = |_: &str, line: &str| log(line);
        let call = Call { log: Some(&to_log), ..Call::default() };
        let answer =
            self.link.call(method::SCM_CHECKOUT, json!({ "id": self.id, "request": request }), &call)?;
        parse(&self.link, method::SCM_CHECKOUT, answer)
    }
}

impl BranchSource for ExternalPoint {
    fn id(&self) -> &str {
        &self.id
    }

    fn discover(&self, settings: &Value) -> Result<Vec<Head>, CiError> {
        let answer = self.link.call(
            method::BRANCH_DISCOVER,
            json!({ "id": self.id, "settings": settings }),
            &Call::default(),
        )?;
        parse(&self.link, method::BRANCH_DISCOVER, answer)
    }

    fn file(&self, settings: &Value, head: &Head, path: &str) -> Result<Option<String>, CiError> {
        let answer = self.link.call(
            method::BRANCH_FILE,
            json!({ "id": self.id, "settings": settings, "head": head, "path": path }),
            &Call::default(),
        )?;
        parse(&self.link, method::BRANCH_FILE, answer)
    }
}

impl Trigger for ExternalPoint {
    fn id(&self) -> &str {
        &self.id
    }

    fn watch(&self, job: &str, settings: &Value, builds: Arc<dyn Builds>) -> Result<(), CiError> {
        self.link.call(
            method::TRIGGER_WATCH,
            json!({ "id": self.id, "job": job, "settings": settings }),
            &Call::default(),
        )?;
        lock(&self.link.watches)
            .insert((self.id.clone(), job.to_owned()), Watch { settings: settings.clone(), builds });
        Ok(())
    }

    fn unwatch(&self, job: &str) {
        let watched = lock(&self.link.watches).remove(&(self.id.clone(), job.to_owned())).is_some();
        // A program that is not running watches nothing: it is not started only to be told.
        if watched && self.link.slot().is_some() {
            let _ = self.link.call(
                method::TRIGGER_UNWATCH,
                json!({ "id": self.id, "job": job }),
                &Call::default(),
            );
        }
    }
}

impl Publisher for ExternalPoint {
    fn id(&self) -> &str {
        &self.id
    }

    fn publish(
        &self,
        build: &BuildInfo,
        settings: &Value,
        workspace: &Path,
    ) -> Result<Vec<Attachment>, CiError> {
        let answer = self.link.call(
            method::PUBLISH,
            json!({ "id": self.id, "build": build, "settings": settings, "workspace": workspace }),
            &Call::default(),
        )?;
        let wires: Vec<AttachmentWire> = parse(&self.link, method::PUBLISH, answer)?;
        wires.into_iter().map(|wire| wire.into_attachment(workspace).map_err(CiError::failed)).collect()
    }
}

impl StatusReporter for ExternalPoint {
    fn id(&self) -> &str {
        &self.id
    }

    fn report(&self, status: &CommitStatus, settings: &Value) -> Result<(), CiError> {
        self.link
            .call(
                method::STATUS_REPORT,
                json!({ "id": self.id, "status": status, "settings": settings }),
                &Call::default(),
            )
            .map(|_| ())
    }
}

impl Notifier for ExternalPoint {
    fn id(&self) -> &str {
        &self.id
    }

    fn notify(&self, event: &BuildEvent, settings: &Value) -> Result<(), CiError> {
        self.link
            .call(
                method::NOTIFY,
                json!({ "id": self.id, "event": event, "settings": settings }),
                &Call::default(),
            )
            .map(|_| ())
    }
}

impl BuildHook for ExternalPoint {
    fn id(&self) -> &str {
        &self.id
    }

    fn before(&self, build: &BuildContext, settings: &Value) -> Result<(), CiError> {
        self.link
            .call(
                method::HOOK_BEFORE,
                json!({ "id": self.id, "build": build, "settings": settings }),
                &Call::default(),
            )
            .map(|_| ())
    }

    fn after(&self, build: &BuildContext, status: BuildStatus, settings: &Value) -> Result<(), CiError> {
        self.link
            .call(
                method::HOOK_AFTER,
                json!({ "id": self.id, "build": build, "status": status, "settings": settings }),
                &Call::default(),
            )
            .map(|_| ())
    }
}

impl AgentLauncher for ExternalPoint {
    fn id(&self) -> &str {
        &self.id
    }

    fn launch(&self, request: &LaunchRequest) -> Result<Box<dyn LaunchedAgent>, CiError> {
        let answer = self.link.call(
            method::AGENT_LAUNCH,
            json!({ "id": self.id, "request": request }),
            &Call::default(),
        )?;
        let handle = answer.get("agent").and_then(Value::as_str).unwrap_or_default().to_owned();
        let name = answer.get("name").and_then(Value::as_str).unwrap_or(&handle).to_owned();
        Ok(Box::new(ExternalAgent { launcher: self.id.clone(), handle, name, link: self.link.clone() }))
    }
}

/// An agent an external launcher brought up, known to the program by its handle.
struct ExternalAgent {
    launcher: String,
    handle: String,
    name: String,
    link: Arc<Link>,
}

impl LaunchedAgent for ExternalAgent {
    fn name(&self) -> String {
        self.name.clone()
    }

    fn alive(&self) -> bool {
        self.link
            .call(method::AGENT_ALIVE, json!({ "id": self.launcher, "agent": self.handle }), &Call::default())
            .ok()
            .and_then(|answer| answer.as_bool())
            .unwrap_or(false)
    }

    fn stop(&self) {
        let _ = self.link.call(
            method::AGENT_STOP,
            json!({ "id": self.launcher, "agent": self.handle }),
            &Call::default(),
        );
    }
}

/// A kind of credential of an external plugin: its form from the `plugin.toml`, what it hides
/// and gives asked of the program.
/// Its form is made again each time it is asked: an entity of `ravenrs-app` is not `Send`.
struct ExternalCredential {
    decl: CredentialDecl,
    link: Arc<Link>,
}

impl ExternalCredential {
    /// The values of the secret fields: what a log hides when the program cannot say.
    fn secret_values(&self, credential: &Value) -> Vec<String> {
        self.decl
            .fields
            .iter()
            .filter(|field| field.secret)
            .filter_map(|field| credential.get(&field.name).and_then(Value::as_str).map(str::to_owned))
            .collect()
    }
}

impl CredentialKind for ExternalCredential {
    fn id(&self) -> &str {
        &self.decl.id
    }

    fn caption(&self) -> String {
        self.decl.caption.clone()
    }

    fn fields(&self) -> EntityInfo {
        entity_of(&self.decl.id, &self.decl.fields).unwrap_or_else(|_| EntityInfo::new(&self.decl.id))
    }

    fn hidden(&self, credential: &Value) -> Vec<String> {
        let asked = self.link.call(
            method::CREDENTIAL_HIDDEN,
            json!({ "id": self.decl.id, "credential": credential }),
            &Call::default(),
        );
        let mut hidden: Vec<String> =
            asked.ok().and_then(|answer| serde_json::from_value(answer).ok()).unwrap_or_default();
        // What the program says comes on top of the secret fields, never instead of them.
        for value in self.secret_values(credential) {
            if !hidden.contains(&value) {
                hidden.push(value);
            }
        }
        hidden
    }

    fn secret(&self, credential: &Value) -> String {
        let asked = self.link.call(
            method::CREDENTIAL_SECRET,
            json!({ "id": self.decl.id, "credential": credential }),
            &Call::default(),
        );
        match asked.ok().and_then(|answer| answer.as_str().map(str::to_owned)) {
            Some(secret) => secret,
            None => self.secret_values(credential).into_iter().next().unwrap_or_default(),
        }
    }
}

/// A pipeline format of an external plugin: `format/read`.
struct ExternalFormat {
    decl: FormatDecl,
    link: Arc<Link>,
}

impl PipelineFormat for ExternalFormat {
    fn id(&self) -> &str {
        &self.decl.id
    }

    fn file_names(&self) -> Vec<String> {
        self.decl.file_names.clone()
    }

    fn read(&self, job: &str, text: &str) -> Result<Pipeline, Vec<FormatError>> {
        let answer = self
            .link
            .call(
                method::FORMAT_READ,
                json!({ "id": self.decl.id, "job": job, "text": text }),
                &Call::default(),
            )
            .map_err(|e| vec![FormatError::whole(e.to_string())])?;
        if let Some(errors) = answer.get("errors") {
            return Err(format_errors_from_wire(errors));
        }
        let wire: PipelineWire =
            serde_json::from_value(answer.get("pipeline").cloned().unwrap_or(Value::Null)).map_err(|e| {
                vec![FormatError::whole(format!(
                    "o plugin {} respondeu um pipeline errado: {e}",
                    self.link.name
                ))]
            })?;
        wire.into_pipeline().map_err(|why| vec![FormatError::whole(why)])
    }
}

/// A route of an external plugin: each request is a `route/handle` call.
fn route(decl: &RouteDecl, link: &Arc<Link>) -> Route {
    let link = link.clone();
    let pattern = decl.path.clone();
    let route = Route::new(&decl.method, &decl.path, move |request| {
        let params = json!({ "route": pattern, "request": http_request_to_wire(request) });
        match link.call(method::ROUTE_HANDLE, params, &Call::default()) {
            Ok(answer) => http_response_from_wire(&answer)
                .unwrap_or_else(|why| HttpResponse::text(502, format!("o plugin {}: {why}", link.name))),
            Err(e) => HttpResponse::text(502, e.to_string()),
        }
    });
    if decl.public { route.public() } else { route }
}

/// A kind of step whose code is in an external plugin: each node is a `step/execute` call, with
/// the build it runs in, whose log gets what the program writes for it.
struct ExternalStep {
    decl: StepDecl,
    link: Arc<Link>,
}

impl NodeKind for ExternalStep {
    fn spec(&self) -> NodeSpec {
        let mut spec = NodeSpec::new(&self.decl.kind, self.decl.version).input("main").output("main");
        if let Some(title) = &self.decl.title {
            spec = spec.title(title);
        }
        for name in &self.decl.required {
            spec = spec.requires(name);
        }
        spec
    }

    fn execute(&self, cx: &NodeContext<'_>) -> Result<Ports, NodeError> {
        let inputs: serde_json::Map<String, Value> = cx
            .inputs()
            .iter()
            .map(|(port, items)| {
                (port.clone(), Value::Array(items.iter().map(|item| item.json.clone()).collect()))
            })
            .collect();
        let scope = cx.run_id().and_then(BuildScope::find);
        let build = scope
            .as_ref()
            .map(|scope| json!({ "context": scope.context(), "environment": scope.environment() }));
        let params = json!({
            "kind": self.decl.kind,
            "version": self.decl.version,
            "node": cx.node().id,
            "parameters": cx.parameters(),
            "inputs": inputs,
            "build": build,
        });
        let node = cx.node().id.clone();
        let log = |step: &str, line: &str| {
            if let Some(scope) = &scope {
                scope.log(if step.is_empty() { &node } else { step }, line);
            }
        };
        let hide = |value: &str| {
            if let Some(scope) = &scope {
                scope.hide(value);
            }
        };
        let call = Call { token: Some(cx.token()), log: Some(&log), hide: Some(&hide) };
        let answer = self.link.call(method::STEP_EXECUTE, params, &call).map_err(|e| {
            if cx.cancelled() { NodeError::cancelled(e.to_string()) } else { NodeError::new(e.to_string()) }
        })?;
        let mut ports = Ports::new();
        if let Some(outputs) = answer.get("outputs").and_then(Value::as_object) {
            let first = cx.input("main").first();
            for (port, items) in outputs {
                let items = items
                    .as_array()
                    .map(|items| {
                        items
                            .iter()
                            .map(|json| match first {
                                Some(parent) => parent.derive(json.clone()),
                                None => Item::new(json.clone()),
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                ports.insert(port.clone(), items);
            }
        }
        Ok(ports)
    }
}

/// A subcommand whose code is in an external plugin: a `cli/run` call.
struct ExternalCommand {
    decl: CommandDecl,
    link: Arc<Link>,
}

impl CliCommand for ExternalCommand {
    fn name(&self) -> &str {
        &self.decl.name
    }

    fn summary(&self) -> String {
        self.decl.summary.clone()
    }

    fn run(&self, args: &[String], out: &mut dyn Write) -> Result<i32, CiError> {
        let answer = self.link.call(
            method::CLI_RUN,
            json!({ "command": self.decl.name, "args": args }),
            &Call::default(),
        )?;
        if let Some(text) = answer.get("output").and_then(Value::as_str) {
            out.write_all(text.as_bytes()).map_err(|e| CiError::failed(e.to_string()))?;
        }
        Ok(answer.get("code").and_then(Value::as_i64).unwrap_or(0) as i32)
    }
}
