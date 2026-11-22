//! A fake plugin for each extension point, and a fake host: what the tests of the core and the
//! self-test of `raven-ci` put together to prove every point is called as its documentation says.
//!
//! Each [`Fake`] offers one point and writes what happened to it in its [`Probe`], which a test
//! keeps before handing the plugin to the core.
//!
//! ```
//! use raven_ci_plugin_api::testing::{Fake, FakeHost};
//! use raven_ci_plugin_api::{CiPlugin, Permission, Point};
//! use std::sync::Arc;
//!
//! let scm = Fake::new(Point::Scm);
//! let probe = scm.probe();
//! scm.start(Arc::new(FakeHost::new("fake-scm", &[Permission::Disk]))).expect("starts");
//! assert_eq!(probe.started(), 1);
//! assert_eq!(scm.scms()[0].id(), "fake-scm");
//! ```

use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use ravenrs_app::{EntityInfo, FieldInfo, FieldKind, Module, ModuleManifest, Rule, VersionRange};
use ravenrs_flows::{Catalog, FlowDefinition, NodeContext, NodeDef, NodeError, NodeKind, NodeSpec, Ports};
use serde_json::{Value, json};

use crate::{
    AgentLauncher, Attachment, BranchSource, BuildEvent, BuildInfo, BuildRequest, Builds, Checkout,
    CheckoutRequest, CiError, CiPlugin, CliCommand, CommitStatus, CredentialKind, FormatError, Head,
    HeadKind, Host, HttpResponse, LaunchRequest, LaunchedAgent, Migration, Notifier, Permission, Pipeline,
    PipelineFormat, Point, Publisher, Route, STAGE_KEY, Scm, StatusReporter, Trigger,
};

/// The kind of step the fake of [`Point::Step`] brings.
pub const ECHO: &str = "fake-echo";

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// A host that grants what it is told, keeps the audit and the log in memory, and gives a
/// folder of its own under the system's temporary folder.
pub struct FakeHost {
    plugin: String,
    granted: Vec<Permission>,
    settings: Value,
    secrets: BTreeMap<String, String>,
    dir: PathBuf,
    audit: Mutex<Vec<(Permission, String, bool)>>,
    lines: Mutex<Vec<String>>,
}

static NEXT_DIR: AtomicU64 = AtomicU64::new(0);

impl FakeHost {
    /// The host of `plugin`, granting `granted`.
    pub fn new(plugin: &str, granted: &[Permission]) -> Self {
        let n = NEXT_DIR.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("raven-ci-fake-{plugin}-{}-{n}", std::process::id()));
        Self {
            plugin: plugin.to_owned(),
            granted: granted.to_vec(),
            settings: json!({}),
            secrets: BTreeMap::new(),
            dir,
            audit: Mutex::default(),
            lines: Mutex::default(),
        }
    }

    /// With these settings.
    pub fn settings(mut self, settings: Value) -> Self {
        self.settings = settings;
        self
    }

    /// Keeping the secret `name`.
    pub fn with_secret(mut self, name: &str, value: &str) -> Self {
        self.secrets.insert(name.to_owned(), value.to_owned());
        self
    }

    /// What was asked: the permission, what for, and whether it was granted.
    pub fn audit(&self) -> Vec<(Permission, String, bool)> {
        lock(&self.audit).clone()
    }

    /// What the plugin wrote to the log.
    pub fn lines(&self) -> Vec<String> {
        lock(&self.lines).clone()
    }
}

impl std::fmt::Debug for FakeHost {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FakeHost")
            .field("plugin", &self.plugin)
            .field("granted", &self.granted)
            .finish_non_exhaustive()
    }
}

impl Drop for FakeHost {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

impl Host for FakeHost {
    fn plugin(&self) -> &str {
        &self.plugin
    }

    fn granted(&self) -> Vec<Permission> {
        self.granted.clone()
    }

    fn require(&self, permission: Permission, what: &str) -> Result<(), CiError> {
        let granted = self.granted.contains(&permission);
        lock(&self.audit).push((permission, what.to_owned(), granted));
        if granted {
            Ok(())
        } else {
            Err(CiError::Denied { plugin: self.plugin.clone(), permission, what: what.to_owned() })
        }
    }

    fn settings(&self) -> Value {
        self.settings.clone()
    }

    fn data_dir(&self) -> Result<PathBuf, CiError> {
        self.require(Permission::Disk, "usar a pasta própria")?;
        std::fs::create_dir_all(&self.dir).map_err(|e| CiError::failed(e.to_string()))?;
        Ok(self.dir.clone())
    }

    fn database(&self) -> Result<PathBuf, CiError> {
        Ok(self.data_dir()?.join("plugin.sqlite"))
    }

    fn secret(&self, name: &str) -> Result<String, CiError> {
        self.require(Permission::Secrets, &format!("ler o segredo {name}"))?;
        self.secrets
            .get(name)
            .cloned()
            .ok_or_else(|| CiError::unavailable(format!("não há o segredo {name}")))
    }

    fn log(&self, line: &str) {
        lock(&self.lines).push(line.to_owned());
    }
}

/// What happened to a [`Fake`], kept by a test after the plugin went to the core.
#[derive(Default)]
pub struct Probe {
    started: AtomicU32,
    stopped: AtomicU32,
    host: Mutex<Option<Arc<dyn Host>>>,
    watched: Mutex<BTreeMap<String, Arc<dyn Builds>>>,
    statuses: Mutex<Vec<CommitStatus>>,
    events: Mutex<Vec<BuildEvent>>,
}

impl std::fmt::Debug for Probe {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Probe")
            .field("started", &self.started())
            .field("stopped", &self.stopped())
            .field("watched", &self.watched())
            .finish_non_exhaustive()
    }
}

impl Probe {
    /// How many times it started.
    pub fn started(&self) -> u32 {
        self.started.load(Ordering::SeqCst)
    }

    /// How many times it stopped.
    pub fn stopped(&self) -> u32 {
        self.stopped.load(Ordering::SeqCst)
    }

    /// The host it got when it last started, while it is on.
    pub fn host(&self) -> Option<Arc<dyn Host>> {
        lock(&self.host).clone()
    }

    /// The jobs the fake trigger watches.
    pub fn watched(&self) -> Vec<String> {
        lock(&self.watched).keys().cloned().collect()
    }

    /// Makes the fake trigger ask for a build of `job`, as a schedule does when its time comes.
    pub fn fire(&self, job: &str) -> Result<u64, CiError> {
        let builds = lock(&self.watched).get(job).cloned();
        match builds {
            Some(builds) => builds.request(BuildRequest::new(job, "disparado pelo gatilho falso")),
            None => Err(CiError::unavailable(format!("o gatilho falso não vigia {job}"))),
        }
    }

    /// The statuses the fake reporter was given.
    pub fn statuses(&self) -> Vec<CommitStatus> {
        lock(&self.statuses).clone()
    }

    /// The events the fake notifier was told.
    pub fn events(&self) -> Vec<BuildEvent> {
        lock(&self.events).clone()
    }
}

/// A plugin that offers one extension point, doing the least that point asks, and writes to its
/// [`Probe`]. Its name is `fake-<point>`, its version 1.0.0 and it asks for no permission,
/// unless told otherwise.
pub struct Fake {
    point: Point,
    name: String,
    manifest: ModuleManifest,
    dependencies: Vec<String>,
    configured: bool,
    migrations: Vec<Migration>,
    fails_to_start: Option<String>,
    probe: Arc<Probe>,
}

impl std::fmt::Debug for Fake {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Fake").field("point", &self.point).field("name", &self.name).finish_non_exhaustive()
    }
}

impl Fake {
    /// The fake of `point`.
    pub fn new(point: Point) -> Self {
        let name = format!("fake-{}", point.name().replace('_', "-"));
        let manifest = ModuleManifest::new().version("1.0.0").unwrap_or_default();
        Self {
            point,
            name,
            manifest,
            dependencies: Vec::new(),
            configured: false,
            migrations: Vec::new(),
            fails_to_start: None,
            probe: Arc::default(),
        }
    }

    /// One of each point a plugin of the CI offers, screens left out: they are a module's own,
    /// tested by `ravenrs-app`.
    pub fn all() -> Vec<Self> {
        Point::ALL.into_iter().filter(|p| *p != Point::Screens).map(Self::new).collect()
    }

    /// Called `name`.
    pub fn named(mut self, name: &str) -> Self {
        self.name = name.to_owned();
        self
    }

    /// At `version`.
    pub fn version(mut self, version: &str) -> Self {
        self.manifest.version = ravenrs_app::Version::parse(version).ok();
        self
    }

    /// Requiring `plugin` in `range`, such as `^2`.
    pub fn requires(mut self, plugin: &str, range: &str) -> Self {
        if let Ok(range) = VersionRange::compatible(range.trim_start_matches('^')) {
            self.manifest.requires.push((plugin.to_owned(), range));
        }
        self
    }

    /// Coming after `plugin`, with no version asked.
    pub fn after(mut self, plugin: &str) -> Self {
        self.dependencies.push(plugin.to_owned());
        self
    }

    /// Declaring `permission`.
    pub fn permission(mut self, permission: Permission) -> Self {
        self.manifest.capabilities.push(permission.capability().to_owned());
        self
    }

    /// With a configuration of one required text, `context`, and a number, `every`.
    pub fn configured(mut self) -> Self {
        self.configured = true;
        self
    }

    /// With this migration of its own database.
    pub fn migration(mut self, id: &str, sql: &str) -> Self {
        self.migrations.push(Migration::new(id, sql));
        self
    }

    /// Failing to start, saying `why`.
    pub fn failing_to_start(mut self, why: &str) -> Self {
        self.fails_to_start = Some(why.to_owned());
        self
    }

    /// What happens to it.
    pub fn probe(&self) -> Arc<Probe> {
        self.probe.clone()
    }

    fn offers(&self, point: Point) -> bool {
        self.point == point && self.probe.host().is_some()
    }
}

impl Module for Fake {
    fn name(&self) -> &str {
        &self.name
    }

    fn dependencies(&self) -> Vec<String> {
        self.dependencies.clone()
    }

    fn manifest(&self) -> ModuleManifest {
        self.manifest.clone()
    }
}

impl CiPlugin for Fake {
    fn summary(&self) -> String {
        format!("Plugin falso do ponto {}.", self.point)
    }

    fn config(&self) -> Option<EntityInfo> {
        self.configured.then(|| {
            EntityInfo::new("FakeSettings")
                .field(FieldInfo::new("context", FieldKind::Text))
                .field(FieldInfo::new("every", FieldKind::Integer))
                .rule(Rule::required("context"))
        })
    }

    fn migrations(&self) -> Vec<Migration> {
        self.migrations.clone()
    }

    fn start(&self, host: Arc<dyn Host>) -> Result<(), CiError> {
        if let Some(why) = &self.fails_to_start {
            return Err(CiError::failed(why.clone()));
        }
        self.probe.started.fetch_add(1, Ordering::SeqCst);
        host.log(&format!("{} ligado", self.name));
        *lock(&self.probe.host) = Some(host);
        Ok(())
    }

    fn stop(&self) {
        self.probe.stopped.fetch_add(1, Ordering::SeqCst);
        lock(&self.probe.watched).clear();
        *lock(&self.probe.host) = None;
    }

    fn steps(&self, catalog: &mut Catalog) {
        if self.offers(Point::Step) {
            catalog.add(Echo);
        }
    }

    fn scms(&self) -> Vec<Arc<dyn Scm>> {
        if self.offers(Point::Scm) { vec![Arc::new(FakeScm)] } else { Vec::new() }
    }

    fn branch_sources(&self) -> Vec<Arc<dyn BranchSource>> {
        if self.offers(Point::BranchSource) { vec![Arc::new(FakeBranches)] } else { Vec::new() }
    }

    fn triggers(&self) -> Vec<Arc<dyn Trigger>> {
        if self.offers(Point::Trigger) { vec![Arc::new(FakeTrigger(self.probe.clone()))] } else { Vec::new() }
    }

    fn publishers(&self) -> Vec<Arc<dyn Publisher>> {
        if self.offers(Point::Publisher) { vec![Arc::new(FakePublisher)] } else { Vec::new() }
    }

    fn status_reporters(&self) -> Vec<Arc<dyn StatusReporter>> {
        if self.offers(Point::StatusReporter) {
            vec![Arc::new(FakeReporter(self.probe.clone()))]
        } else {
            Vec::new()
        }
    }

    fn notifiers(&self) -> Vec<Arc<dyn Notifier>> {
        if self.offers(Point::Notifier) {
            vec![Arc::new(FakeNotifier(self.probe.clone()))]
        } else {
            Vec::new()
        }
    }

    fn credential_kinds(&self) -> Vec<Arc<dyn CredentialKind>> {
        if self.offers(Point::CredentialKind) { vec![Arc::new(FakeToken)] } else { Vec::new() }
    }

    fn agent_launchers(&self) -> Vec<Arc<dyn AgentLauncher>> {
        if self.offers(Point::AgentLauncher) { vec![Arc::new(FakeLauncher)] } else { Vec::new() }
    }

    fn pipeline_formats(&self) -> Vec<Arc<dyn PipelineFormat>> {
        if self.offers(Point::PipelineFormat) { vec![Arc::new(FakeLines)] } else { Vec::new() }
    }

    fn routes(&self) -> Vec<Route> {
        if self.offers(Point::Routes) {
            vec![Route::new("GET", "/api/fake/hello", |request| {
                HttpResponse::json(200, &json!({ "hello": request.actor }))
            })]
        } else {
            Vec::new()
        }
    }

    fn cli_commands(&self) -> Vec<Arc<dyn CliCommand>> {
        if self.offers(Point::CliCommand) { vec![Arc::new(FakeCommand)] } else { Vec::new() }
    }
}

/// The step `fake-echo`: passes its items on with `said` set to its parameter `says`. With
/// `fail: true` it fails; with `sleep_ms` it waits that long first, looking at its token.
#[derive(Debug)]
pub struct Echo;

impl NodeKind for Echo {
    fn spec(&self) -> NodeSpec {
        NodeSpec::new(ECHO, 1).title("Eco").input("main").output("main")
    }

    fn execute(&self, cx: &NodeContext<'_>) -> Result<Ports, NodeError> {
        if let Some(ms) = cx.parameter("sleep_ms").and_then(Value::as_u64) {
            cx.sleep(ms)?;
        }
        if cx.parameter("fail").and_then(Value::as_bool) == Some(true) {
            return Err(NodeError::new("o eco falhou, como pedido"));
        }
        let says = cx.parameter("says").cloned().unwrap_or(Value::Null);
        let items = cx
            .input("main")
            .iter()
            .map(|item| {
                let mut json = item.json.clone();
                if let Value::Object(object) = &mut json {
                    object.insert("said".to_owned(), says.clone());
                }
                item.derive(json)
            })
            .collect();
        Ok(Ports::from([("main".to_owned(), items)]))
    }
}

struct FakeScm;

impl Scm for FakeScm {
    fn id(&self) -> &str {
        "fake-scm"
    }

    fn checkout(&self, request: &CheckoutRequest, log: &dyn Fn(&str)) -> Result<Checkout, CiError> {
        log(&format!("buscando {}", request.url));
        Ok(Checkout { commit: "f00d".to_owned(), branch: Some("main".to_owned()), merged_onto: None })
    }
}

struct FakeBranches;

impl BranchSource for FakeBranches {
    fn id(&self) -> &str {
        "fake-branches"
    }

    fn discover(&self, settings: &Value) -> Result<Vec<Head>, CiError> {
        let forks = settings.get("forks").and_then(Value::as_bool).unwrap_or(false);
        let mut heads = vec![
            Head { name: "main".to_owned(), kind: HeadKind::Branch, commit: "a1".to_owned() },
            Head {
                name: "PR-1".to_owned(),
                kind: HeadKind::PullRequest { number: 1, target: "main".to_owned(), from_fork: false },
                commit: "b2".to_owned(),
            },
            Head {
                name: "PR-2".to_owned(),
                kind: HeadKind::PullRequest { number: 2, target: "main".to_owned(), from_fork: true },
                commit: "c3".to_owned(),
            },
        ];
        if !forks {
            heads.retain(|head| !matches!(head.kind, HeadKind::PullRequest { from_fork: true, .. }));
        }
        Ok(heads)
    }
}

struct FakeTrigger(Arc<Probe>);

impl Trigger for FakeTrigger {
    fn id(&self) -> &str {
        "fake-trigger"
    }

    fn watch(&self, job: &str, settings: &Value, builds: Arc<dyn Builds>) -> Result<(), CiError> {
        if settings.get("broken").is_some() {
            return Err(CiError::settings("broken", "o gatilho falso recusa esta configuração"));
        }
        lock(&self.0.watched).insert(job.to_owned(), builds);
        Ok(())
    }

    fn unwatch(&self, job: &str) {
        lock(&self.0.watched).remove(job);
    }
}

struct FakePublisher;

impl Publisher for FakePublisher {
    fn id(&self) -> &str {
        "fake-publisher"
    }

    fn publish(&self, build: &BuildInfo, _: &Value, _: &Path) -> Result<Vec<Attachment>, CiError> {
        let text = format!("{} #{} {}", build.job, build.number, build.status.words());
        Ok(vec![
            Attachment::new("resumo", "resumo.txt", "text/plain", text.into_bytes())
                .summary(json!({ "status": build.status })),
        ])
    }
}

struct FakeReporter(Arc<Probe>);

impl StatusReporter for FakeReporter {
    fn id(&self) -> &str {
        "fake-status"
    }

    fn report(&self, status: &CommitStatus, _: &Value) -> Result<(), CiError> {
        lock(&self.0.statuses).push(status.clone());
        Ok(())
    }
}

struct FakeNotifier(Arc<Probe>);

impl Notifier for FakeNotifier {
    fn id(&self) -> &str {
        "fake-notifier"
    }

    fn notify(&self, event: &BuildEvent, _: &Value) -> Result<(), CiError> {
        lock(&self.0.events).push(event.clone());
        Ok(())
    }
}

struct FakeToken;

impl CredentialKind for FakeToken {
    fn id(&self) -> &str {
        "fake-token"
    }

    fn caption(&self) -> String {
        "Token falso".to_owned()
    }

    fn fields(&self) -> EntityInfo {
        let mut token = FieldInfo::new("token", FieldKind::Text);
        token.secret = true;
        EntityInfo::new("FakeToken").field(token).rule(Rule::required("token"))
    }

    fn hidden(&self, credential: &Value) -> Vec<String> {
        credential.get("token").and_then(Value::as_str).map(str::to_owned).into_iter().collect()
    }
}

struct FakeLauncher;

struct FakeAgent {
    name: String,
    alive: AtomicBool,
}

impl LaunchedAgent for FakeAgent {
    fn name(&self) -> String {
        self.name.clone()
    }

    fn alive(&self) -> bool {
        self.alive.load(Ordering::SeqCst)
    }

    fn stop(&self) {
        self.alive.store(false, Ordering::SeqCst);
    }
}

impl AgentLauncher for FakeLauncher {
    fn id(&self) -> &str {
        "fake-agent"
    }

    fn launch(&self, request: &LaunchRequest) -> Result<Box<dyn LaunchedAgent>, CiError> {
        let name = format!("falso-{}", request.build.as_deref().unwrap_or("livre"));
        Ok(Box::new(FakeAgent { name, alive: AtomicBool::new(true) }))
    }
}

/// The format `fake-lines`: one step per line, `Stage: what it says`. `Stage: !fail` fails and
/// `Stage: ~500` waits 500 ms, looking at its token. Blank lines and lines starting with `#` are
/// left out.
#[derive(Debug)]
pub struct FakeLines;

impl PipelineFormat for FakeLines {
    fn id(&self) -> &str {
        "fake-lines"
    }

    fn file_names(&self) -> Vec<String> {
        vec!["pipeline.txt".to_owned()]
    }

    fn read(&self, job: &str, text: &str) -> Result<Pipeline, Vec<FormatError>> {
        let mut flow = FlowDefinition::new(job, job).node(NodeDef::new("start", "manual").title("Início"));
        let mut errors = Vec::new();
        let mut previous = "start".to_owned();
        for (n, line) in text.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((stage, says)) = line.split_once(':') else {
                errors.push(FormatError::at(n + 1, "falta o ':' entre o estágio e o passo"));
                continue;
            };
            let (stage, says) = (stage.trim(), says.trim());
            let id = format!("step-{}", n + 1);
            let mut node = NodeDef::new(&id, ECHO)
                .title(says)
                .parameter(STAGE_KEY, json!(stage))
                .parameter("says", json!(says));
            if says == "!fail" {
                node = node.parameter("fail", json!(true));
            } else if let Some(ms) = says.strip_prefix('~').and_then(|ms| ms.parse::<u64>().ok()) {
                node = node.parameter("sleep_ms", json!(ms));
            }
            flow = flow.node(node).connect(&previous, "main", &id, "main");
            previous = id;
        }
        if errors.is_empty() { Ok(Pipeline::new(flow, "start")) } else { Err(errors) }
    }
}

struct FakeCommand;

impl CliCommand for FakeCommand {
    fn name(&self) -> &str {
        "fake"
    }

    fn summary(&self) -> String {
        "Repete o que recebeu.".to_owned()
    }

    fn run(&self, args: &[String], out: &mut dyn Write) -> Result<i32, CiError> {
        writeln!(out, "fake: {}", args.join(" ")).map_err(|e| CiError::failed(e.to_string()))?;
        Ok(0)
    }
}
