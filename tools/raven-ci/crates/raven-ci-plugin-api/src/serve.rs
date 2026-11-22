//! The `main` of an external plugin written in Rust: [`serve`].
//!
//! # The role
//!
//! A plugin compiled into `raven-ci` implements [`CiPlugin`] and its points and runs in the
//! server's process. The same plugin, unchanged, can be its own program instead: its `main` calls
//! [`serve`], and `raven-ci plugins compilar PASTA` builds it, writes its `plugin.toml` and installs
//! it, with no new `raven-ci`.
//!
//! # How it works
//!
//! - A thread reads the standard input, one JSON-RPC message per line. An answer to something the
//!   plugin asked goes to the call that waits for it; each call of the core runs on a thread of its
//!   own, so a call that asks the host (and the host that asks the plugin back) never waits on
//!   itself.
//! - `initialize` starts the plugin with a [`Host`] that is a client of `host/*`: each service is
//!   asked of the server, which checks the permission and keeps the audit, as for a plugin
//!   compiled in.
//! - Each point becomes the method of the [`protocol`](crate::protocol): `step/execute` runs the
//!   node with a [`BuildScope`] whose log goes to the build's log, `publisher/publish` sends the
//!   attachments, `trigger/watch` gets a [`Builds`] whose requests are `host/request_build` and
//!   `host/scan`.
//! - With `--manifesto`, it writes the `plugin.toml` of the plugin (its manifest and the ids of its
//!   points) to the standard output and ends. The points are asked without `start`, so a plugin
//!   returns them from its constructor.
//!
//! # How to use it
//!
//! ```no_run
//! # use raven_ci_plugin_api::CiPlugin;
//! # struct Mine;
//! # impl ravenrs_app::Module for Mine { fn name(&self) -> &str { "meu" } }
//! # impl CiPlugin for Mine {}
//! fn main() {
//!     raven_ci_plugin_api::serve(Mine);
//! }
//! ```

use std::collections::{BTreeMap, HashMap};
use std::io::{BufRead, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Sender, channel};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::time::Duration;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use ravenrs_flows::{Catalog, Item, Mode, NodeContext, NodeDef, Ports};
use serde::de::DeserializeOwned;
use serde_json::{Map, Value, json};

use crate::protocol::{
    API_VERSION, AttachmentWire, CommandDecl, CredentialDecl, FormatDecl, IdDecl, PROTOCOL, PipelineWire,
    PluginToml, RouteDecl, StepDecl, error_from_wire, error_to_wire, fields_of, format_errors_to_wire,
    http_request_from_wire, http_response_to_wire, method,
};
use crate::{
    AgentLauncher, AttachmentInfo, BranchSource, BuildContext, BuildHook, BuildLog, BuildRequest, BuildScope,
    Builds, CiError, CiPlugin, CliCommand, CredentialKind, Host, LaunchedAgent, Notifier, Permission,
    PipelineFormat, Publisher, Route, Scm, StatusReporter, Trigger, enter,
};

/// The longest the plugin waits for the server to answer something it asked.
const HOST_LIMIT: Duration = Duration::from_secs(300);

/// Serves `plugin` on the standard input and output until the server says `shutdown` or closes
/// the input; with `--manifesto` among the arguments, writes its `plugin.toml` instead. Ends the
/// process with 1 when the manifest cannot be written.
#[allow(clippy::print_stderr, clippy::exit, reason = "the main of a program")]
pub fn serve(plugin: impl CiPlugin + 'static) {
    let plugin: Arc<dyn CiPlugin> = Arc::new(plugin);
    if std::env::args().any(|arg| arg == "--manifesto") {
        let command = std::env::current_exe()
            .ok()
            .and_then(|exe| exe.file_name().map(|name| format!("./{}", name.to_string_lossy())))
            .unwrap_or_else(|| format!("./{}", plugin.name()));
        match manifest(plugin.as_ref(), &command)
            .and_then(|file| toml::to_string(&file).map_err(|e| format!("o plugin.toml não se escreve: {e}")))
        {
            Ok(text) => {
                let mut out = std::io::stdout();
                let _ = out.write_all(text.as_bytes()).and_then(|()| out.flush());
            }
            Err(why) => {
                eprintln!("{why}");
                std::process::exit(1);
            }
        }
        return;
    }
    let stdin = std::io::stdin();
    serve_with(plugin, stdin.lock(), std::io::stdout());
}

/// The `plugin.toml` of `plugin`, whose program is `command`: its name, version, permissions and
/// requirements from its manifest, and the ids of its points. Its points are asked without
/// `start`.
pub fn manifest(plugin: &dyn CiPlugin, command: &str) -> Result<PluginToml, String> {
    let module = plugin.manifest();
    let version = module.version.ok_or_else(|| {
        format!("o plugin {} não diz a versão: ModuleManifest::new().version(\"0.1.0\")", plugin.name())
    })?;
    let mut catalog = Catalog::new();
    plugin.steps(&mut catalog);
    let ids = |list: Vec<String>| list.into_iter().map(IdDecl::new).collect::<Vec<_>>();
    let config = match plugin.config() {
        Some(entity) => fields_of(&entity)?,
        None => Vec::new(),
    };
    let credential_kinds = plugin
        .credential_kinds()
        .iter()
        .map(|kind| {
            Ok(CredentialDecl {
                id: kind.id().to_owned(),
                caption: kind.caption(),
                fields: fields_of(&kind.fields())?,
            })
        })
        .collect::<Result<_, String>>()?;
    let file = PluginToml {
        name: plugin.name().to_owned(),
        version: version.to_string(),
        summary: plugin.summary(),
        command: command.to_owned(),
        windows_command: None,
        args: Vec::new(),
        permissions: Permission::declared(&module).iter().map(|p| p.name().to_owned()).collect(),
        after: plugin.dependencies(),
        timeout_ms: None,
        idle_ms: None,
        api: Some(API_VERSION.to_owned()),
        requires: module
            .requires
            .iter()
            .map(|(name, range)| (name.clone(), format!("^{}", range.from)))
            .collect(),
        config,
        steps: catalog
            .specs()
            .into_iter()
            .map(|spec| StepDecl {
                title: (spec.title != spec.kind).then(|| spec.title.clone()),
                kind: spec.kind,
                version: spec.version,
                required: spec.required,
            })
            .collect(),
        commands: plugin
            .cli_commands()
            .iter()
            .map(|c| CommandDecl { name: c.name().to_owned(), summary: c.summary() })
            .collect(),
        scms: ids(plugin.scms().iter().map(|p| p.id().to_owned()).collect()),
        branch_sources: ids(plugin.branch_sources().iter().map(|p| p.id().to_owned()).collect()),
        triggers: ids(plugin.triggers().iter().map(|p| p.id().to_owned()).collect()),
        publishers: ids(plugin.publishers().iter().map(|p| p.id().to_owned()).collect()),
        status_reporters: ids(plugin.status_reporters().iter().map(|p| p.id().to_owned()).collect()),
        notifiers: ids(plugin.notifiers().iter().map(|p| p.id().to_owned()).collect()),
        agent_launchers: ids(plugin.agent_launchers().iter().map(|p| p.id().to_owned()).collect()),
        build_hooks: ids(plugin.build_hooks().iter().map(|p| p.id().to_owned()).collect()),
        pipeline_formats: plugin
            .pipeline_formats()
            .iter()
            .map(|f| FormatDecl { id: f.id().to_owned(), file_names: f.file_names() })
            .collect(),
        credential_kinds,
        routes: plugin
            .routes()
            .iter()
            .map(|r| RouteDecl { method: r.method.clone(), path: r.path.clone(), public: r.public })
            .collect(),
    };
    let problems = file.check();
    if problems.is_empty() { Ok(file) } else { Err(problems.join("; ")) }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Where the lines go, one message at a time.
struct Wire {
    out: Mutex<Box<dyn Write + Send>>,
    asked: Mutex<HashMap<u64, Sender<Result<Value, Value>>>>,
    next: AtomicU64,
}

impl Wire {
    fn send(&self, message: &Value) {
        let mut out = lock(&self.out);
        let _ = writeln!(out, "{message}").and_then(|()| out.flush());
    }

    fn notify(&self, method: &str, params: Value) {
        self.send(&json!({ "jsonrpc": "2.0", "method": method, "params": params }));
    }

    /// Asks the server `method` and waits for its answer.
    fn ask(&self, method: &str, params: Value) -> Result<Value, CiError> {
        let id = self.next.fetch_add(1, Ordering::SeqCst) + 1;
        let (tx, rx) = channel();
        lock(&self.asked).insert(id, tx);
        self.send(&json!({ "jsonrpc": "2.0", "id": format!("p{id}"), "method": method, "params": params }));
        let answer = rx.recv_timeout(HOST_LIMIT);
        lock(&self.asked).remove(&id);
        match answer {
            Ok(Ok(value)) => Ok(value),
            Ok(Err(error)) => Err(error_from_wire(&error)),
            Err(_) => Err(CiError::unavailable(format!("o servidor não respondeu a {method}"))),
        }
    }

    /// Hands an answer of the server to the call that asked.
    fn answered(&self, message: &Value) {
        let Some(id) = message.get("id").and_then(Value::as_str).and_then(|id| id.strip_prefix('p')) else {
            return;
        };
        let Ok(id) = id.parse::<u64>() else { return };
        if let Some(tx) = lock(&self.asked).remove(&id) {
            let answer = match message.get("error") {
                Some(error) => Err(error.clone()),
                None => Ok(message.get("result").cloned().unwrap_or(Value::Null)),
            };
            let _ = tx.send(answer);
        }
    }
}

/// The host of the plugin, over the wire.
struct WireHost {
    name: String,
    wire: Arc<Wire>,
}

impl Host for WireHost {
    fn plugin(&self) -> &str {
        &self.name
    }

    fn granted(&self) -> Vec<Permission> {
        self.wire
            .ask(method::HOST_GRANTED, Value::Null)
            .ok()
            .and_then(|v| v.as_array().cloned())
            .unwrap_or_default()
            .iter()
            .filter_map(|p| p.as_str().and_then(Permission::parse))
            .collect()
    }

    fn require(&self, permission: Permission, what: &str) -> Result<(), CiError> {
        self.wire
            .ask(method::HOST_REQUIRE, json!({ "permission": permission.name(), "what": what }))
            .map(|_| ())
    }

    fn settings(&self) -> Value {
        self.wire.ask(method::HOST_SETTINGS, Value::Null).unwrap_or_else(|_| json!({}))
    }

    fn data_dir(&self) -> Result<PathBuf, CiError> {
        self.wire
            .ask(method::HOST_DATA_DIR, Value::Null)
            .map(|v| PathBuf::from(v.as_str().unwrap_or_default()))
    }

    fn database(&self) -> Result<PathBuf, CiError> {
        self.wire
            .ask(method::HOST_DATABASE, Value::Null)
            .map(|v| PathBuf::from(v.as_str().unwrap_or_default()))
    }

    fn secret(&self, name: &str) -> Result<String, CiError> {
        self.wire
            .ask(method::HOST_SECRET, json!({ "name": name }))
            .map(|v| v.as_str().unwrap_or_default().to_owned())
    }

    fn log(&self, line: &str) {
        self.wire.notify(method::HOST_LOG, json!({ "line": line }));
    }

    fn attachments(
        &self,
        job: &str,
        number: Option<u64>,
        kind: &str,
    ) -> Result<Vec<AttachmentInfo>, CiError> {
        let answer =
            self.wire.ask(method::HOST_ATTACHMENTS, json!({ "job": job, "number": number, "kind": kind }))?;
        serde_json::from_value(answer)
            .map_err(|e| CiError::failed(format!("os anexos que o servidor deu: {e}")))
    }

    fn attachment(&self, id: i64) -> Result<(AttachmentInfo, Vec<u8>), CiError> {
        let answer = self.wire.ask(method::HOST_ATTACHMENT, json!({ "id": id }))?;
        let info = serde_json::from_value(answer.get("info").cloned().unwrap_or(Value::Null))
            .map_err(|e| CiError::failed(format!("o anexo {id} que o servidor deu: {e}")))?;
        let text = answer.get("base64").and_then(Value::as_str).unwrap_or_default();
        let bytes = STANDARD
            .decode(text.as_bytes())
            .map_err(|e| CiError::failed(format!("o anexo {id}: base64: {e}")))?;
        Ok((info, bytes))
    }
}

/// The builds a trigger of the plugin asks for, over the wire, for the job it watches.
struct WireBuilds {
    trigger: String,
    job: String,
    wire: Arc<Wire>,
}

impl Builds for WireBuilds {
    fn request(&self, request: BuildRequest) -> Result<u64, CiError> {
        let answer = self.wire.ask(
            method::HOST_REQUEST_BUILD,
            json!({ "trigger": self.trigger, "watched": self.job, "request": request }),
        )?;
        answer.as_u64().ok_or_else(|| CiError::failed("o servidor não disse o número do build"))
    }

    fn scan(&self, job: &str) -> Result<Vec<String>, CiError> {
        let answer = self
            .wire
            .ask(method::HOST_SCAN, json!({ "trigger": self.trigger, "watched": self.job, "job": job }))?;
        Ok(serde_json::from_value(answer).unwrap_or_default())
    }
}

/// The log of the build of one call, over the wire.
struct WireLog {
    call: Value,
    wire: Arc<Wire>,
}

impl BuildLog for WireLog {
    fn line(&self, step: &str, text: &str) {
        self.wire.notify(method::HOST_BUILD_LOG, json!({ "call": self.call, "step": step, "line": text }));
    }

    fn hide(&self, value: &str) {
        self.wire.notify(method::HOST_HIDE, json!({ "call": self.call, "value": value }));
    }
}

/// The points of the plugin, asked once it started.
struct Points {
    catalog: Catalog,
    commands: Vec<Arc<dyn CliCommand>>,
    scms: Vec<Arc<dyn Scm>>,
    branch_sources: Vec<Arc<dyn BranchSource>>,
    triggers: Vec<Arc<dyn Trigger>>,
    publishers: Vec<Arc<dyn Publisher>>,
    status_reporters: Vec<Arc<dyn StatusReporter>>,
    notifiers: Vec<Arc<dyn Notifier>>,
    credential_kinds: Vec<Arc<dyn CredentialKind>>,
    agent_launchers: Vec<Arc<dyn AgentLauncher>>,
    pipeline_formats: Vec<Arc<dyn PipelineFormat>>,
    routes: Vec<Route>,
    build_hooks: Vec<Arc<dyn BuildHook>>,
}

impl Points {
    fn of(plugin: &dyn CiPlugin) -> Self {
        let mut catalog = Catalog::new();
        plugin.steps(&mut catalog);
        Self {
            catalog,
            commands: plugin.cli_commands(),
            scms: plugin.scms(),
            branch_sources: plugin.branch_sources(),
            triggers: plugin.triggers(),
            publishers: plugin.publishers(),
            status_reporters: plugin.status_reporters(),
            notifiers: plugin.notifiers(),
            credential_kinds: plugin.credential_kinds(),
            agent_launchers: plugin.agent_launchers(),
            pipeline_formats: plugin.pipeline_formats(),
            routes: plugin.routes(),
            build_hooks: plugin.build_hooks(),
        }
    }
}

/// One conversation with the server.
struct Server {
    plugin: Arc<dyn CiPlugin>,
    wire: Arc<Wire>,
    points: OnceLock<Points>,
    agents: Mutex<BTreeMap<String, Box<dyn LaunchedAgent>>>,
    next_agent: AtomicU64,
}

/// Serves `plugin` reading `input` and writing `output`, until `shutdown` or the end of `input`;
/// the plugin is stopped then. What [`serve`] does on the standard input and output, for tests.
pub fn serve_with(plugin: Arc<dyn CiPlugin>, input: impl BufRead, output: impl Write + Send + 'static) {
    let server = Arc::new(Server {
        plugin,
        wire: Arc::new(Wire {
            out: Mutex::new(Box::new(output)),
            asked: Mutex::default(),
            next: AtomicU64::new(0),
        }),
        points: OnceLock::new(),
        agents: Mutex::default(),
        next_agent: AtomicU64::new(0),
    });
    for line in input.lines() {
        let Ok(line) = line else { break };
        let Ok(message) = serde_json::from_str::<Value>(&line) else { continue };
        let Some(asked) = message.get("method").and_then(Value::as_str) else {
            server.wire.answered(&message);
            continue;
        };
        if asked == method::SHUTDOWN {
            break;
        }
        let Some(id) = message.get("id").cloned() else { continue };
        let asked = asked.to_owned();
        let params = message.get("params").cloned().unwrap_or(Value::Null);
        let server = server.clone();
        let _ = std::thread::Builder::new().name(format!("raven-ci-{asked}")).spawn(move || {
            let reply = match server.answer(&asked, &params, &id) {
                Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
                Err(error) => json!({ "jsonrpc": "2.0", "id": id, "error": error_to_wire(&error) }),
            };
            server.wire.send(&reply);
        });
    }
    if server.points.get().is_some() {
        for launched in std::mem::take(&mut *lock(&server.agents)).into_values() {
            launched.stop();
        }
        server.plugin.stop();
    }
}

/// `params[key]` read as `T`, or a failure naming it.
fn field<T: DeserializeOwned>(params: &Value, key: &str) -> Result<T, CiError> {
    serde_json::from_value(params.get(key).cloned().unwrap_or(Value::Null))
        .map_err(|e| CiError::failed(format!("o pedido não trouxe {key} certo: {e}")))
}

fn to_json(value: impl serde::Serialize) -> Result<Value, CiError> {
    serde_json::to_value(value).map_err(|e| CiError::failed(e.to_string()))
}

/// The point whose id is `params.id` among `list`.
fn find<'a, T: ?Sized>(
    list: &'a [Arc<T>],
    params: &Value,
    id_of: impl Fn(&T) -> &str,
    what: &str,
) -> Result<&'a Arc<T>, CiError> {
    let id = params.get("id").and_then(Value::as_str).unwrap_or_default();
    list.iter()
        .find(|point| id_of(point) == id)
        .ok_or_else(|| CiError::unavailable(format!("este plugin não tem {what} {id}")))
}

impl Server {
    fn points(&self) -> Result<&Points, CiError> {
        self.points.get().ok_or_else(|| CiError::unavailable("o plugin ainda não recebeu o initialize"))
    }

    fn answer(&self, asked: &str, params: &Value, call: &Value) -> Result<Value, CiError> {
        if asked == method::INITIALIZE {
            if self.points.get().is_none() {
                let host =
                    Arc::new(WireHost { name: self.plugin.name().to_owned(), wire: self.wire.clone() });
                self.plugin.start(host)?;
                let _ = self.points.set(Points::of(self.plugin.as_ref()));
            }
            let version = self.plugin.manifest().version.map(|v| v.to_string()).unwrap_or_default();
            return Ok(
                json!({ "name": self.plugin.name(), "version": version, "api": API_VERSION, "protocol": PROTOCOL }),
            );
        }
        let points = self.points()?;
        match asked {
            method::STEP_EXECUTE => self.step(points, params, call),
            method::CLI_RUN => {
                let name = params.get("command").and_then(Value::as_str).unwrap_or_default();
                let command =
                    points.commands.iter().find(|c| c.name() == name).ok_or_else(|| {
                        CiError::unavailable(format!("este plugin não tem o comando {name}"))
                    })?;
                let args: Vec<String> = field(params, "args").unwrap_or_default();
                let mut out = Vec::new();
                let code = command.run(&args, &mut out)?;
                Ok(json!({ "code": code, "output": String::from_utf8_lossy(&out) }))
            }
            method::SCM_CHECKOUT => {
                let scm = find(&points.scms, params, |p| p.id(), "o scm")?;
                let request = field(params, "request")?;
                let log = WireLog { call: call.clone(), wire: self.wire.clone() };
                to_json(scm.checkout(&request, &|line| log.line("", line))?)
            }
            method::BRANCH_DISCOVER => {
                let source = find(&points.branch_sources, params, |p| p.id(), "a fonte de branches")?;
                to_json(source.discover(params.get("settings").unwrap_or(&Value::Null))?)
            }
            method::BRANCH_FILE => {
                let source = find(&points.branch_sources, params, |p| p.id(), "a fonte de branches")?;
                let head = field(params, "head")?;
                let path: String = field(params, "path")?;
                to_json(source.file(params.get("settings").unwrap_or(&Value::Null), &head, &path)?)
            }
            method::PUBLISH => {
                let publisher = find(&points.publishers, params, |p| p.id(), "o publicador")?;
                let build = field(params, "build")?;
                let workspace: PathBuf = field(params, "workspace")?;
                let attachments =
                    publisher.publish(&build, params.get("settings").unwrap_or(&Value::Null), &workspace)?;
                let wires = attachments
                    .into_iter()
                    .map(|a| AttachmentWire::from_attachment(a, &workspace))
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(CiError::failed)?;
                to_json(wires)
            }
            method::STATUS_REPORT => {
                let reporter = find(&points.status_reporters, params, |p| p.id(), "o relator de status")?;
                reporter.report(&field(params, "status")?, params.get("settings").unwrap_or(&Value::Null))?;
                Ok(Value::Null)
            }
            method::NOTIFY => {
                let notifier = find(&points.notifiers, params, |p| p.id(), "o notificador")?;
                notifier.notify(&field(params, "event")?, params.get("settings").unwrap_or(&Value::Null))?;
                Ok(Value::Null)
            }
            method::CREDENTIAL_HIDDEN => {
                let kind = find(&points.credential_kinds, params, |p| p.id(), "o tipo de credencial")?;
                to_json(kind.hidden(params.get("credential").unwrap_or(&Value::Null)))
            }
            method::CREDENTIAL_SECRET => {
                let kind = find(&points.credential_kinds, params, |p| p.id(), "o tipo de credencial")?;
                Ok(Value::String(kind.secret(params.get("credential").unwrap_or(&Value::Null))))
            }
            method::HOOK_BEFORE | method::HOOK_AFTER => {
                let hook = find(&points.build_hooks, params, |p| p.id(), "o gancho")?;
                let build: BuildContext = field(params, "build")?;
                let settings = params.get("settings").unwrap_or(&Value::Null);
                if asked == method::HOOK_BEFORE {
                    hook.before(&build, settings)?;
                } else {
                    hook.after(&build, field(params, "status")?, settings)?;
                }
                Ok(Value::Null)
            }
            method::ROUTE_HANDLE => {
                let request = http_request_from_wire(params.get("request").unwrap_or(&Value::Null))
                    .map_err(CiError::failed)?;
                let declared = params.get("route").and_then(Value::as_str).unwrap_or_default();
                let route = points
                    .routes
                    .iter()
                    .find(|r| r.method.eq_ignore_ascii_case(&request.method) && r.path == declared)
                    .or_else(|| points.routes.iter().find(|r| r.matches(&request.method, &request.path)))
                    .ok_or_else(|| CiError::unavailable(format!("este plugin não tem a rota {declared}")))?;
                Ok(http_response_to_wire(&(route.handler)(&request)))
            }
            method::TRIGGER_WATCH => {
                let trigger = find(&points.triggers, params, |p| p.id(), "o gatilho")?;
                let job: String = field(params, "job")?;
                let builds = Arc::new(WireBuilds {
                    trigger: trigger.id().to_owned(),
                    job: job.clone(),
                    wire: self.wire.clone(),
                });
                trigger.watch(&job, params.get("settings").unwrap_or(&Value::Null), builds)?;
                Ok(Value::Null)
            }
            method::TRIGGER_UNWATCH => {
                let trigger = find(&points.triggers, params, |p| p.id(), "o gatilho")?;
                trigger.unwatch(&field::<String>(params, "job")?);
                Ok(Value::Null)
            }
            method::FORMAT_READ => {
                let format = find(&points.pipeline_formats, params, |p| p.id(), "o formato")?;
                let job: String = field(params, "job")?;
                let text: String = field(params, "text")?;
                match format.read(&job, &text) {
                    Ok(pipeline) => Ok(
                        json!({ "pipeline": PipelineWire::from_pipeline(pipeline).map_err(CiError::failed)? }),
                    ),
                    Err(errors) => Ok(json!({ "errors": format_errors_to_wire(&errors) })),
                }
            }
            method::AGENT_LAUNCH => {
                let launcher = find(&points.agent_launchers, params, |p| p.id(), "o lançador de agentes")?;
                let launched = launcher.launch(&field(params, "request")?)?;
                let handle = format!("a{}", self.next_agent.fetch_add(1, Ordering::SeqCst) + 1);
                let name = launched.name();
                lock(&self.agents).insert(handle.clone(), launched);
                Ok(json!({ "agent": handle, "name": name }))
            }
            method::AGENT_ALIVE => {
                let handle: String = field(params, "agent")?;
                Ok(Value::Bool(lock(&self.agents).get(&handle).is_some_and(|a| a.alive())))
            }
            method::AGENT_STOP => {
                let handle: String = field(params, "agent")?;
                let launched = lock(&self.agents).remove(&handle);
                if let Some(launched) = launched {
                    launched.stop();
                }
                Ok(Value::Null)
            }
            other => Err(CiError::failed(format!("o plugin não conhece o método {other}"))),
        }
    }

    /// Runs a node, with the scope of its build when the server sent one.
    fn step(&self, points: &Points, params: &Value, call: &Value) -> Result<Value, CiError> {
        let kind = params.get("kind").and_then(Value::as_str).unwrap_or_default();
        let version = params.get("version").and_then(Value::as_u64).and_then(|v| u32::try_from(v).ok());
        let spec = points
            .catalog
            .specs()
            .into_iter()
            .filter(|spec| spec.kind == kind && version.is_none_or(|v| v == spec.version))
            .max_by_key(|spec| spec.version)
            .ok_or_else(|| CiError::unavailable(format!("este plugin não tem o passo {kind}")))?;
        let node_kind = points
            .catalog
            .get(kind, spec.version)
            .ok_or_else(|| CiError::unavailable(format!("este plugin não tem o passo {kind}")))?;
        let mut node = NodeDef::new(params.get("node").and_then(Value::as_str).unwrap_or(kind), kind);
        let parameters: Map<String, Value> = field(params, "parameters").unwrap_or_default();
        for (name, value) in parameters {
            node = node.parameter(name, value);
        }
        let inputs: BTreeMap<String, Vec<Value>> = field(params, "inputs").unwrap_or_default();
        let inputs: Ports = inputs
            .into_iter()
            .map(|(port, items)| (port, items.into_iter().map(Item::new).collect()))
            .collect();
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX));
        let mut cx = NodeContext::new(&node, &inputs, now_ms, Mode::Active);
        let mut _guard = None;
        if let Some(build) = params.get("build").filter(|b| !b.is_null()) {
            let context: BuildContext = field(build, "context")?;
            let environment: BTreeMap<String, String> = field(build, "environment").unwrap_or_default();
            let run: Arc<str> = Arc::from(context.run.as_str());
            let log = Arc::new(WireLog { call: call.clone(), wire: self.wire.clone() });
            _guard = Some(enter(BuildScope::new(context, log, environment)));
            cx = cx.in_run(Some(run));
        }
        let ports = node_kind.execute(&cx).map_err(|e| CiError::failed(e.to_string()))?;
        let outputs: Map<String, Value> = ports
            .into_iter()
            .map(|(port, items)| (port, Value::Array(items.into_iter().map(|item| item.json).collect())))
            .collect();
        Ok(json!({ "outputs": outputs }))
    }
}

#[cfg(test)]
mod tests {
    use std::io::BufReader;
    use std::sync::mpsc::Receiver;

    use ravenrs_app::{Module, ModuleManifest};

    use super::*;

    /// A plugin with one command, `anexos`, that lists through its host what builds left.
    #[derive(Default)]
    struct Reader {
        host: Arc<Mutex<Option<Arc<dyn Host>>>>,
    }

    impl Module for Reader {
        fn name(&self) -> &str {
            "leitor"
        }
        fn manifest(&self) -> ModuleManifest {
            ModuleManifest::new().version("0.2.0").unwrap_or_default()
        }
    }

    impl CiPlugin for Reader {
        fn start(&self, host: Arc<dyn Host>) -> Result<(), CiError> {
            *lock(&self.host) = Some(host);
            Ok(())
        }
        fn cli_commands(&self) -> Vec<Arc<dyn CliCommand>> {
            vec![Arc::new(Anexos(self.host.clone()))]
        }
    }

    struct Anexos(Arc<Mutex<Option<Arc<dyn Host>>>>);

    impl CliCommand for Anexos {
        fn name(&self) -> &str {
            "anexos"
        }
        fn summary(&self) -> String {
            String::new()
        }
        fn run(&self, _: &[String], out: &mut dyn Write) -> Result<i32, CiError> {
            let host = lock(&self.0).clone().ok_or_else(|| CiError::unavailable("sem host"))?;
            let listed = host.attachments("site", Some(3), "junit")?;
            let (_, bytes) = host.attachment(listed[0].id)?;
            writeln!(out, "{} {}", listed[0].name, String::from_utf8_lossy(&bytes))
                .map_err(|e| CiError::failed(e.to_string()))?;
            Ok(0)
        }
    }

    /// What the plugin writes, one message per line.
    struct Lines(Sender<Value>, Vec<u8>);

    impl Write for Lines {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.1.extend_from_slice(buf);
            while let Some(at) = self.1.iter().position(|b| *b == b'\n') {
                let line: Vec<u8> = self.1.drain(..=at).collect();
                if let Ok(message) = serde_json::from_slice(&line) {
                    let _ = self.0.send(message);
                }
            }
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    fn next(said: &Receiver<Value>) -> Value {
        said.recv_timeout(Duration::from_secs(10)).expect("the plugin said something")
    }

    #[test]
    fn the_host_of_a_served_plugin_asks_the_server_and_waits_for_its_answer() {
        let (input, mut to_plugin) = std::io::pipe().expect("a pipe");
        let (tx, said) = channel();
        let served = std::thread::spawn(move || {
            serve_with(Arc::new(Reader::default()), BufReader::new(input), Lines(tx, Vec::new()));
        });
        let mut send = |message: Value| writeln!(to_plugin, "{message}").expect("written");
        send(json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {} }));
        let hello = next(&said);
        assert_eq!(
            (hello["result"]["name"].as_str(), hello["result"]["version"].as_str()),
            (Some("leitor"), Some("0.2.0"))
        );
        assert_eq!(hello["result"]["api"], API_VERSION);

        send(
            json!({ "jsonrpc": "2.0", "id": 2, "method": "cli/run", "params": { "command": "anexos", "args": [] } }),
        );
        let asked = next(&said);
        assert_eq!(asked["method"], "host/attachments");
        assert_eq!(asked["params"], json!({ "job": "site", "number": 3, "kind": "junit" }));
        let info = json!({ "id": 9, "job": "site", "number": 3, "kind": "junit", "name": "junit.xml", "mime": "text/xml", "size": 2, "summary": {} });
        send(json!({ "jsonrpc": "2.0", "id": asked["id"], "result": [info.clone()] }));
        let asked = next(&said);
        assert_eq!(
            (asked["method"].as_str(), &asked["params"]),
            (Some("host/attachment"), &json!({ "id": 9 }))
        );
        send(
            json!({ "jsonrpc": "2.0", "id": asked["id"], "result": { "info": info, "base64": STANDARD.encode("ok") } }),
        );
        let done = next(&said);
        assert_eq!(done["id"], 2);
        assert_eq!(done["result"], json!({ "code": 0, "output": "junit.xml ok\n" }));

        send(
            json!({ "jsonrpc": "2.0", "id": 3, "method": "cli/run", "params": { "command": "nenhum", "args": [] } }),
        );
        let refused = next(&said);
        assert_eq!(refused["error"]["data"]["kind"], "unavailable", "an error crosses whole: {refused}");

        send(json!({ "jsonrpc": "2.0", "method": "shutdown" }));
        served.join().expect("it ends on shutdown");
    }
}
