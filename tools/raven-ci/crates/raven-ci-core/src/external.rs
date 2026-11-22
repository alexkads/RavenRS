//! Plugins written in any language: a program that speaks JSON-RPC 2.0 on its standard input and
//! output, one message per line, as MCP servers do.
//!
//! Its folder has a `plugin.toml` that says who it is without starting it:
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
//! ```
//!
//! The program starts at the first call, in its folder, with only the basic variables of the
//! environment. It is asked `initialize` first and must answer its name and version as the
//! manifest says. Each call has a time limit; past it, or when the program dies, the program is
//! stopped and the call fails with the reason, and the next call starts it again. With no call
//! for `idle_ms`, it is stopped. The server never goes down with it.

use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, Weak};
use std::time::{Duration, Instant};

use raven_ci_plugin_api::{CiError, CiPlugin, CliCommand, Host, Permission, Point};
use ravenrs_app::{Module, ModuleManifest, Target, Version, VersionRange};
use ravenrs_flows::{CancelToken, Catalog, Item, NodeContext, NodeError, NodeKind, NodeSpec, Ports};
use ravenrs_process::{Control, End, Event, Running, Spec};
use serde::Deserialize;
use serde_json::{Value, json};

/// The name of the file that says who an external plugin is.
pub const MANIFEST_FILE: &str = "plugin.toml";

/// How long a call may take when the manifest does not say.
const DEFAULT_TIMEOUT_MS: u64 = 30_000;

/// How long a plugin may sit with no call when the manifest does not say.
const DEFAULT_IDLE_MS: u64 = 60_000;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ManifestFile {
    name: String,
    version: String,
    #[serde(default)]
    summary: String,
    command: String,
    #[serde(default)]
    windows_command: Option<String>,
    #[serde(default)]
    args: Vec<String>,
    #[serde(default)]
    permissions: Vec<String>,
    #[serde(default)]
    requires: BTreeMap<String, String>,
    #[serde(default)]
    after: Vec<String>,
    #[serde(default)]
    timeout_ms: Option<u64>,
    #[serde(default)]
    idle_ms: Option<u64>,
    #[serde(default)]
    steps: Vec<StepDecl>,
    #[serde(default)]
    commands: Vec<CommandDecl>,
}

/// A kind of step an external plugin brings.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StepDecl {
    /// What a pipeline names it by.
    pub kind: String,
    /// Its version.
    #[serde(default = "first_version")]
    pub version: u32,
    /// What it is called.
    #[serde(default)]
    pub title: Option<String>,
    /// The parameters a step of this kind must have.
    #[serde(default)]
    pub required: Vec<String>,
}

fn first_version() -> u32 {
    1
}

/// A subcommand an external plugin brings.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandDecl {
    /// What follows `raven-ci`.
    pub name: String,
    /// One line for the list of commands.
    #[serde(default)]
    pub summary: String,
}

/// What the `plugin.toml` of an external plugin says, checked.
#[derive(Clone, Debug, PartialEq, Eq)]
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
}

/// Reads and checks the `plugin.toml` in `folder`, saying every problem it finds.
pub fn read_manifest(folder: &Path) -> Result<ExternalManifest, String> {
    let path = folder.join(MANIFEST_FILE);
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let file: ManifestFile = toml::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut problems = Vec::new();
    if file.name.is_empty()
        || !file.name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        problems.push(format!("o nome {:?} tem que ter só letras minúsculas, números e '-'", file.name));
    }
    let version = Version::parse(&file.version).unwrap_or_else(|e| {
        problems.push(format!("a versão: {e}"));
        Version::default()
    });
    let mut permissions = Vec::new();
    for name in &file.permissions {
        match Permission::parse(name) {
            Some(p) => permissions.push(p),
            None => problems.push(format!(
                "a permissão {name:?} não existe; as que há são {}",
                Permission::ALL.map(Permission::name).join(", ")
            )),
        }
    }
    let mut requires = Vec::new();
    for (plugin, range) in &file.requires {
        match VersionRange::compatible(range.trim_start_matches('^')) {
            Ok(range) => requires.push((plugin.clone(), range)),
            Err(e) => problems.push(format!("requires.{plugin}: {e}")),
        }
    }
    if file.steps.is_empty() && file.commands.is_empty() {
        problems.push("não oferece nenhum passo ([[steps]]) nem comando ([[commands]])".to_owned());
    }
    if !problems.is_empty() {
        return Err(format!("{}: {}", path.display(), problems.join("; ")));
    }
    let command = match (&file.windows_command, cfg!(windows)) {
        (Some(command), true) => command.clone(),
        _ => file.command,
    };
    Ok(ExternalManifest {
        name: file.name,
        version,
        summary: file.summary,
        command,
        args: file.args,
        permissions,
        requires,
        after: file.after,
        timeout: Duration::from_millis(file.timeout_ms.unwrap_or(DEFAULT_TIMEOUT_MS).max(1)),
        idle: Duration::from_millis(file.idle_ms.unwrap_or(DEFAULT_IDLE_MS).max(1)),
        steps: file.steps,
        commands: file.commands,
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
            host: Mutex::default(),
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

    /// The points its manifest offers.
    pub fn declared_points(&self) -> Vec<Point> {
        let mut points = Vec::new();
        if !self.manifest.steps.is_empty() {
            points.push(Point::Step);
        }
        if !self.manifest.commands.is_empty() {
            points.push(Point::CliCommand);
        }
        points
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

    /// Calls `method` with `params`, starting the program when it is not running.
    pub fn call(&self, method: &str, params: Value) -> Result<Value, CiError> {
        self.link.call(method, params, None)
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

    fn start(&self, host: Arc<dyn Host>) -> Result<(), CiError> {
        *lock(&self.link.host) = Some(host);
        Ok(())
    }

    fn stop(&self) {
        self.link.shut_down();
        *lock(&self.link.host) = None;
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
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// The program of a plugin, running.
struct Process {
    running: Running,
    control: Control,
    last_call: Instant,
    generation: u64,
}

/// How the core reaches the program of a plugin: one conversation at a time.
struct Link {
    name: String,
    version: Version,
    command: String,
    args: Vec<String>,
    folder: PathBuf,
    timeout: Duration,
    idle: Duration,
    process: Mutex<Option<Process>>,
    host: Mutex<Option<Arc<dyn Host>>>,
    next_id: AtomicU64,
    starts: AtomicU64,
}

/// Why a call did not get an answer.
enum Broken {
    /// The plugin answered with an error: it stays up.
    Answered(String),
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

    /// Calls `method`, starting the program when it is not running. Only one call goes at a time.
    fn call(
        self: &Arc<Self>,
        method: &str,
        params: Value,
        token: Option<&CancelToken>,
    ) -> Result<Value, CiError> {
        let mut slot = self.slot();
        if slot.is_none() {
            *slot = Some(self.spawn().map_err(CiError::failed)?);
        }
        let Some(process) = slot.as_mut() else { return Err(CiError::failed("o plugin não subiu")) };
        let result = self.exchange(process, method, params, self.timeout, token);
        process.last_call = Instant::now();
        match result {
            Ok(value) => Ok(value),
            Err(Broken::Answered(why)) => Err(CiError::failed(why)),
            Err(Broken::Gone(why)) => {
                process.control.stop();
                *slot = None;
                self.log(&why);
                Err(CiError::failed(why))
            }
            Err(Broken::Cancelled(why)) => {
                process.control.stop();
                *slot = None;
                Err(CiError::failed(why))
            }
        }
    }

    /// Starts the program, asks `initialize` and checks who answered; then leaves a watcher that
    /// stops it after `idle` with no call.
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
            .spawn()
            .map_err(|e| format!("o plugin {} não subiu ({}): {e}", self.name, self.command))?;
        let generation = self.starts.fetch_add(1, Ordering::SeqCst) + 1;
        let control = running.control();
        let mut process = Process { running, control, last_call: Instant::now(), generation };
        let hello = json!({ "server": "raven-ci", "plugin": self.name, "protocol": 1 });
        let said = match self.exchange(&mut process, "initialize", hello, self.timeout, None) {
            Ok(said) => said,
            Err(Broken::Answered(why) | Broken::Gone(why) | Broken::Cancelled(why)) => {
                process.control.stop();
                return Err(format!("o plugin {} não respondeu ao initialize: {why}", self.name));
            }
        };
        let name = said.get("name").and_then(Value::as_str).unwrap_or_default();
        let version = said.get("version").and_then(Value::as_str).and_then(|v| Version::parse(v).ok());
        if name != self.name || version != Some(self.version) {
            process.control.stop();
            return Err(format!(
                "o programa diz ser {name} {}, e o plugin.toml diz {} {}",
                said.get("version").and_then(Value::as_str).unwrap_or("sem versão"),
                self.name,
                self.version
            ));
        }
        self.log(&format!("subiu (processo {})", process.control.pid()));
        self.watch_idle(generation);
        Ok(process)
    }

    /// Stops the program of `generation` once it sits with no call for `idle`.
    fn watch_idle(self: &Arc<Self>, generation: u64) {
        let link: Weak<Self> = Arc::downgrade(self);
        let tick = (self.idle / 4).clamp(Duration::from_millis(10), Duration::from_secs(1));
        let _ = std::thread::Builder::new().name(format!("raven-ci-ocioso-{}", self.name)).spawn(move || {
            loop {
                std::thread::sleep(tick);
                let Some(link) = link.upgrade() else { return };
                // A call in progress holds the slot: wait for it rather than stop under it.
                let Ok(mut slot) = link.process.try_lock() else { continue };
                let Some(process) = slot.as_ref() else { return };
                if process.generation != generation {
                    return;
                }
                if process.last_call.elapsed() >= link.idle {
                    let _ =
                        process.control.send(&json!({ "jsonrpc": "2.0", "method": "shutdown" }).to_string());
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
            let _ = process.control.send(&json!({ "jsonrpc": "2.0", "method": "shutdown" }).to_string());
            process.control.stop();
        }
    }

    /// Sends one request and waits for its answer, serving what the program asks the host in
    /// the meantime.
    fn exchange(
        &self,
        process: &mut Process,
        method: &str,
        params: Value,
        limit: Duration,
        token: Option<&CancelToken>,
    ) -> Result<Value, Broken> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let request = json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params });
        if !process.control.send(&request.to_string()) {
            return Err(Broken::Gone(format!("o plugin {} fechou a entrada", self.name)));
        }
        let deadline = Instant::now() + limit;
        loop {
            if let Some(token) = token
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
            let Some(event) = process.running.next_within(left.min(Duration::from_millis(50))) else {
                continue;
            };
            let line = match event {
                Event::Out(line) => line,
                Event::Err(line) => {
                    self.log(&line);
                    continue;
                }
                Event::Ended(end) => {
                    let how = match end {
                        End::Code(code) => format!("com o código {code}"),
                        End::Signal(signal) => format!("pelo sinal {signal}"),
                        End::TimedOut | End::Stopped => "parado".to_owned(),
                    };
                    return Err(Broken::Gone(format!(
                        "o plugin {} terminou no meio de {method}, {how}",
                        self.name
                    )));
                }
            };
            let Ok(message) = serde_json::from_str::<Value>(&line) else {
                self.log(&format!("linha que não é JSON-RPC: {line}"));
                continue;
            };
            if let Some(asked) = message.get("method").and_then(Value::as_str) {
                let answer = self.serve(asked, message.get("params").unwrap_or(&Value::Null));
                if let Some(their_id) = message.get("id") {
                    let reply = match answer {
                        Ok(result) => json!({ "jsonrpc": "2.0", "id": their_id, "result": result }),
                        Err(e) => {
                            json!({ "jsonrpc": "2.0", "id": their_id, "error": { "code": -32000, "message": e.to_string() } })
                        }
                    };
                    process.control.send(&reply.to_string());
                }
                continue;
            }
            if message.get("id").and_then(Value::as_u64) != Some(id) {
                continue;
            }
            if let Some(error) = message.get("error") {
                let said = error.get("message").and_then(Value::as_str).unwrap_or("erro sem mensagem");
                return Err(Broken::Answered(format!("o plugin {}: {said}", self.name)));
            }
            return Ok(message.get("result").cloned().unwrap_or(Value::Null));
        }
    }

    /// Answers what the program asks the host, checking the permission of each service.
    fn serve(&self, method: &str, params: &Value) -> Result<Value, CiError> {
        let Some(host) = self.host() else { return Err(CiError::unavailable("o plugin está desligado")) };
        match method {
            "host/log" => {
                host.log(params.get("line").and_then(Value::as_str).unwrap_or_default());
                Ok(Value::Null)
            }
            "host/settings" => Ok(host.settings()),
            "host/secret" => {
                let name = params.get("name").and_then(Value::as_str).unwrap_or_default();
                host.secret(name).map(Value::String)
            }
            "host/data_dir" => host.data_dir().map(|dir| json!(dir.display().to_string())),
            other => Err(CiError::failed(format!("o host não tem o método {other}"))),
        }
    }
}

/// A kind of step whose code is in an external plugin: each node is a `step/execute` call.
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
        let params = json!({
            "kind": self.decl.kind,
            "node": cx.node().id,
            "parameters": cx.parameters(),
            "inputs": inputs,
        });
        let answer = self.link.call("step/execute", params, Some(cx.token())).map_err(|e| {
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
        let answer = self.link.call("cli/run", json!({ "command": self.decl.name, "args": args }), None)?;
        if let Some(text) = answer.get("output").and_then(Value::as_str) {
            out.write_all(text.as_bytes()).map_err(|e| CiError::failed(e.to_string()))?;
        }
        Ok(answer.get("code").and_then(Value::as_i64).unwrap_or(0) as i32)
    }
}
