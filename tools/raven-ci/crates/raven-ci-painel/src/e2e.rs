//! The panel on the test channel: what Raven Test Studio drives when it opens the panel with
//! `--e2e-serve`, with no window. The panel talks to a real core of Raven CI, in a folder of its
//! own under the system's temporary folder, through the same API `raven-ci servir` answers, only
//! with no HTTP: each request is handed to the service and answered before it returns.
//!
//! The core has the plugins of the tests (a step and the format `fake-lines`), the plugin
//! api-trigger for Disparar, and [`Demonstracao`], which puts a page in the menu and a tab on the
//! page of a build; three people (ana, administrator; bia, executor; caio, reader), with the
//! password [`PASSWORD`]; and four jobs, three of them built.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;

use raven_ci_core::api::{create_user, service_with, users_file};
use raven_ci_core::panel::PanelOptions;
use raven_ci_core::{Ci, JobSpec};
use raven_ci_plugin_api::{BuildRequest, CiPlugin, HttpRequest, HttpResponse, ROLE_READ, ROLE_RUN, Route};
use ravenrs::prelude::*;
use ravenrs_app::{ModelNode, Module, ModuleManifest};
use ravenrs_app_remote::{RemoteClient, RemoteRequest, RemoteResponse, Transport};
use ravenrs_app_server::{ApiRequest, ApiService};
use serde_json::json;

/// The password of the three people of the tests.
pub const PASSWORD: &str = "senha-do-teste-e2e";

/// The scenarios the panel brings, one per file of `tests/e2e/`: the tool gets them when it opens
/// the panel, and `tests/scenarios.rs` plays them.
pub const SCENARIOS: [&str; 8] = [
    include_str!("../tests/e2e/entrar.raventest"),
    include_str!("../tests/e2e/build.raventest"),
    include_str!("../tests/e2e/disparar.raventest"),
    include_str!("../tests/e2e/fila.raventest"),
    include_str!("../tests/e2e/fila-motivo.raventest"),
    include_str!("../tests/e2e/plugins.raventest"),
    include_str!("../tests/e2e/plugin-recusado.raventest"),
    include_str!("../tests/e2e/credenciais.raventest"),
];

/// A plugin of the tests that puts a page in the menu (Travas, under Servidor) and a tab on the
/// page of a build (Cobertura), each over a route of its own: what any plugin does to be on the
/// panel.
#[derive(Debug)]
pub struct Demonstracao;

impl Module for Demonstracao {
    fn name(&self) -> &str {
        "demonstracao"
    }

    fn manifest(&self) -> ModuleManifest {
        ModuleManifest::new().version("0.1.0").unwrap_or_default()
    }

    fn extend_model(&self, root: &mut ModelNode) {
        let dashboard = |name: &str, caption: &str, control: &str| {
            ModelNode::new(format!("{name}_DashboardView"))
                .with("Kind", "DashboardView")
                .with("Caption", caption)
                .child(
                    ModelNode::new("Items").child(
                        ModelNode::new("dados")
                            .with("Caption", caption)
                            .with("Control", control)
                            .with("Index", 0),
                    ),
                )
        };
        if let Some(views) = root.children.iter_mut().find(|n| n.id == "Views") {
            views.children.push(dashboard("travas", "Travas", "/api/ci/demonstracao/travas"));
            views.children.push(dashboard(
                "cobertura",
                "Cobertura",
                "/api/ci/demonstracao/cobertura?job={job}&number={number}",
            ));
        }
        if let Some(navigation) = root.children.iter_mut().find(|n| n.id == "NavigationItems") {
            navigation.children.push(ModelNode::new("Servidor").with("Caption", "Servidor").child(
                ModelNode::new("travas").with("Caption", "Travas").with("View", "travas_DashboardView"),
            ));
            navigation.children.push(
                ModelNode::new(raven_ci_core::panel::BUILD_TAB).child(
                    ModelNode::new("cobertura")
                        .with("Caption", "Cobertura")
                        .with("View", "cobertura_DashboardView"),
                ),
            );
        }
    }
}

impl CiPlugin for Demonstracao {
    fn summary(&self) -> String {
        "Põe a página Travas no menu e a aba Cobertura na página do build.".to_owned()
    }

    // The scenario of its own screens, as any plugin with screens brings it (#919).
    fn scenarios(&self) -> Vec<String> {
        vec![include_str!("../tests/e2e/plugins/demonstracao.raventest").to_owned()]
    }

    fn routes(&self) -> Vec<Route> {
        vec![
            Route::new("GET", "/api/ci/demonstracao/travas", |_: &HttpRequest| {
                HttpResponse::json(
                    200,
                    &json!([
                        { "trava": "compilacao", "com": "apps/site #2", "desde": "14:02" },
                        { "trava": "deploy", "com": "livre", "desde": "—" },
                    ]),
                )
            }),
            Route::new("GET", "/api/ci/demonstracao/cobertura", |request: &HttpRequest| {
                let job = request.query.get("job").cloned().unwrap_or_default();
                HttpResponse::json(200, &json!({ "job": job, "linhas": "81,4%", "ramos": "67,0%" }))
            }),
        ]
    }
}

/// A plugin that asks for another the server does not have: it shows as refused, with why (T4).
#[derive(Debug)]
pub struct Relatorios;

impl Module for Relatorios {
    fn name(&self) -> &str {
        "relatorios"
    }

    fn manifest(&self) -> ModuleManifest {
        let manifest = ModuleManifest::new().version("2.0.0").unwrap_or_default();
        match ravenrs_app::VersionRange::compatible("1.0.0") {
            Ok(range) => manifest.requires("graficos", range),
            Err(_) => manifest,
        }
    }
}

impl CiPlugin for Relatorios {
    fn summary(&self) -> String {
        "Relatórios dos builds, com os gráficos do plugin graficos.".to_owned()
    }
}

/// Answers each request with the service, before returning, as on the desktop.
#[derive(Clone)]
struct NoHttp(Rc<RefCell<ApiService>>);

impl Transport for NoHttp {
    fn send(&self, request: RemoteRequest, done: Box<dyn FnOnce(Result<RemoteResponse, String>)>) {
        let address = request.url.split_once("://").map_or(request.url.as_str(), |(_, rest)| rest);
        let target = address.split_once('/').map_or("", |(_, rest)| rest);
        let (path, query) = target.split_once('?').unwrap_or((target, ""));
        let mut asked = ApiRequest::new(&request.method, &format!("/{path}"));
        for pair in query.split('&').filter(|pair| !pair.is_empty()) {
            let (name, value) = pair.split_once('=').unwrap_or((pair, ""));
            asked = asked.query(name, &unescape(value));
        }
        if let Some(token) = &request.bearer {
            asked = asked.bearer(token);
        }
        if let Some(body) = &request.body {
            asked.body = body.clone().into_bytes();
        }
        let answer = self.0.borrow_mut().handle(asked);
        let body = match &answer.raw {
            Some((_, bytes)) => String::from_utf8_lossy(bytes).into_owned(),
            None => answer.body.to_string(),
        };
        done(Ok(RemoteResponse::new(answer.status, body)));
    }
}

/// `%2F` back to `/`, as a server does with the query.
fn unescape(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && let Some(byte) = text.get(i + 1..i + 3).and_then(|hex| u8::from_str_radix(hex, 16).ok())
        {
            out.push(byte);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// A folder of its own for one session, emptied, under the system's temporary folder.
pub fn folder(name: &str) -> PathBuf {
    let folder = std::env::temp_dir().join(format!("raven-ci-painel-e2e-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&folder);
    folder
}

/// The jobs of the demonstration, in `.raven-ci.toml`. The steps only `echo` and `exit`, which
/// `sh` and `cmd` both run: the same builds on every system.
const JOBS: [(&str, &str); 4] = [
    (
        "apps/site",
        r#"[[stages]]
name = "Checkout"
steps = ["echo Buscando o PR mesclado em master", "echo HEAD is now at a1b2c3d"]

[[stages]]
name = "Lint"
steps = [{ sh = "echo cargo clippy --workspace -- -D warnings", name = "Clippy" }]

[[stages]]
name = "Testes"
steps = [
  "echo PASS [0.012s] ravenrs-flows plan::rejects_cycles",
  "echo PASS [0.040s] ravenrs-flows-runtime claim::lease_is_exclusive",
  "echo SLOW [12.4s] raven-ci registry::reloads_without_restart",
  "echo PASS [0.003s] raven-ci-plugin-shell masks_secrets",
]

[[stages]]
name = "Imagem"
steps = ["echo docker build -t ghcr.io/dono/site:2 ."]
"#,
    ),
    (
        "apps/loja-web",
        r#"[[stages]]
name = "Instalar"
steps = ["echo pnpm install --frozen-lockfile"]

[[stages]]
name = "Testes"
steps = ["echo PASS loja-web carrinho", "echo PASS loja-web checkout"]
"#,
    ),
    (
        "api-pedidos",
        r#"[[parameters]]
name = "BRANCH"
type = "text"
default = "develop"
description = "A branch que o build busca"

[[parameters]]
name = "E2E"
type = "choice"
choices = ["todos", "nenhum", "escolher suítes"]
default = "todos"
description = "Quais suítes de ponta a ponta rodam"

[[parameters]]
name = "UNIDADE"
type = "bool"
default = true
description = "Rodar os testes de unidade"

[[stages]]
name = "Compilar"
steps = ["echo cargo build --release"]

[[stages]]
name = "Testes"
steps = ["echo FAIL [0.210s] pedidos::total_com_desconto", "exit 1"]
"#,
    ),
    (
        "imagens-base",
        r#"[pipeline]
agent = { label = "docker" }

[[stages]]
name = "Imagem"
steps = ["echo docker build -t imagens/base ."]
"#,
    ),
];

/// The core of the tests in `folder`, with its plugins, people and jobs, and a client that talks
/// to it with no HTTP.
pub fn server(folder: &Path) -> Result<(Arc<Ci>, RemoteClient), String> {
    server_with(folder, Vec::new())
}

/// The same core with `extra` plugins on too: what a plugin's own test drives its screens on.
/// A plugin the core of the tests already has is not added twice.
pub fn server_with(folder: &Path, extra: Vec<Arc<dyn CiPlugin>>) -> Result<(Arc<Ci>, RemoteClient), String> {
    let failed = |e: &dyn std::fmt::Display| e.to_string();
    let ci = Arc::new(Ci::open(folder).map_err(|e| failed(&e))?);
    ci.add_plugin(Arc::new(raven_ci_plugin_shell::ShellPlugin::new()));
    ci.add_plugin(Arc::new(raven_ci_plugin_format_toml::TomlFormatPlugin));
    ci.add_plugin(Arc::new(raven_ci_plugin_format_jenkinsfile::JenkinsfilePlugin));
    ci.add_plugin(Arc::new(raven_ci_plugin_lock::LockPlugin::new()));
    ci.add_plugin(Arc::new(raven_ci_plugin_docker::DockerPlugin::new()));
    ci.add_plugin(Arc::new(raven_ci_plugin_api_trigger::ApiTriggerPlugin::new()));
    ci.add_plugin(Arc::new(raven_ci_plugin_credentials::CredentialsPlugin));
    ci.add_plugin(Arc::new(Demonstracao));
    ci.add_plugin(Arc::new(Relatorios));
    for plugin in extra {
        let name = plugin.name().to_owned();
        if ci.registry().list().map_err(|e| failed(&e))?.iter().all(|p| p.name != name) {
            ci.add_plugin(plugin);
        }
    }
    ci.apply_plugins().map_err(|e| failed(&e))?;
    // Docker is switched off: the build of imagens-base waits for it, and the panel says so.
    ci.disable_plugin("docker").map_err(|e| failed(&e))?;
    // A credential a job names, for the grid of Credenciais (#922).
    ci.save_credential(
        "npm-feed",
        "token",
        &json!({ "token": "npm_demonstracao_123456" }),
        Some("feed npm privado"),
        None,
    )
    .map_err(|e| failed(&e))?;
    let users = users_file(folder);
    create_user(&users, "ana", PASSWORD, ROLE_READ).map_err(|e| failed(&e))?;
    create_user(&users, "bia", PASSWORD, ROLE_RUN).map_err(|e| failed(&e))?;
    create_user(&users, "caio", PASSWORD, ROLE_READ).map_err(|e| failed(&e))?;
    for (name, pipeline) in JOBS {
        // Every job takes requests from the API, as Disparar asks.
        let spec = JobSpec::new(name, "toml", pipeline).settings(json!({ "triggers": [{ "id": "api" }] }));
        ci.save_job(spec).map_err(|e| failed(&e))?;
    }
    for job in ["apps/site", "apps/loja-web", "api-pedidos", "apps/site"] {
        ci.request_build(BuildRequest::new(job, "Webhook do GitHub")).map_err(|e| failed(&e))?;
        ci.work_until_idle("executor-1").map_err(|e| failed(&e))?;
    }
    // An agent with its template switched off: the build that asks for its label waits.
    ci.save_agent_template(&raven_ci_core::agents::AgentTemplate {
        name: "docker".to_owned(),
        labels: vec!["docker".to_owned()],
        launcher: "docker".to_owned(),
        settings: json!({}),
        executors: 2,
        max_agents: 2,
        idle_ms: 600_000,
    })
    .map_err(|e| failed(&e))?;
    ci.request_build(BuildRequest::new("imagens-base", "Agenda · 02:30")).map_err(|e| failed(&e))?;
    // An agent that came in, said hello and is connected now.
    let labels = vec!["linux".to_owned(), "rust".to_owned()];
    let token = ci.agent_token("oficina-1", &labels, 1).map_err(|e| failed(&e))?;
    let hello = raven_ci_core::agents::Hello {
        labels: labels.clone(),
        executors: 1,
        free: 1,
        session: "demonstracao".to_owned(),
        memory_bytes: Some(16 * 1024 * 1024),
        ..raven_ci_core::agents::Hello::default()
    };
    ci.agent_join(&raven_ci_core::agents::Join { name: "oficina-1".to_owned(), token, hello })
        .map_err(|e| failed(&e))?;
    // The plugin api-trigger takes requests only for the jobs it watches.
    if let Some(problem) = ci.watch_triggers().into_iter().next() {
        return Err(problem);
    }
    let options =
        PanelOptions { executors: 2, files: None, version: raven_ci_core_version(), users_file: None };
    let service = service_with(ci.clone(), &users, options)?;
    let client = RemoteClient::new("http://raven-ci.e2e", NoHttp(Rc::new(RefCell::new(service))));
    Ok((ci, client))
}

/// The version the server of the tests says it has.
fn raven_ci_core_version() -> String {
    "0.1.0".to_owned()
}

/// The panel over the core of the tests in `folder`, not signed in, and the core.
pub fn panel(folder: &Path) -> Result<(Arc<Ci>, crate::Painel), String> {
    let (ci, client) = server(folder)?;
    Ok((ci, crate::Painel::new(client, false)))
}

/// The session a scenario runs on: the panel over the core of the tests, from the screen of
/// signing in.
pub fn session(folder: &Path) -> Result<ravenrs_e2e::E2e, String> {
    session_with(folder, Vec::new())
}

/// The session with `extra` plugins on too, as [`server_with`].
pub fn session_with(folder: &Path, extra: Vec<Arc<dyn CiPlugin>>) -> Result<ravenrs_e2e::E2e, String> {
    let (ci, client) = server_with(folder, extra)?;
    let painel = crate::Painel::new(client, false);
    // The service holds the core as long as the session lives.
    drop(ci);
    Ok(ravenrs_e2e::E2e::new(Localized::new(Labels::pt_br(), painel), Theme::raven_dark())
        .viewport(Size::new(1440.0, 900.0), 1.0))
}

/// Answers the tool when the panel was opened with `--e2e-serve`, until it says bye; `false` when
/// it was opened as usual.
pub fn serve_if_asked() -> bool {
    if !std::env::args().skip(1).any(|arg| arg == ravenrs_e2e_wire::SERVE_FLAG) {
        return false;
    }
    let hello = ravenrs_e2e_wire::Hello::of("Raven CI").scenarios(all_scenarios());
    let served = session(&folder("canal")).map_err(std::io::Error::other).and_then(|session| {
        ravenrs_e2e_wire::serve_filmed(hello, session, ravenrs::movie::ChannelFilm::default())
    });
    if let Err(error) = served {
        eprintln!("o canal de teste parou: {error}");
        std::process::exit(1);
    }
    true
}

/// The scenarios the tool lists: the panel's own, then those of the plugins on in the core of the
/// tests, each plugin's after its name.
pub fn all_scenarios() -> Vec<String> {
    let mut all: Vec<String> = SCENARIOS.iter().map(|s| (*s).to_owned()).collect();
    let plugins: [Arc<dyn CiPlugin>; 1] = [Arc::new(Demonstracao)];
    for plugin in plugins {
        all.extend(plugin.scenarios());
    }
    all
}

/// A plugin's screens on the panel, for a test of the plugin's own crate: the core of the tests
/// with `plugin` on, and the panel over it from the screen of signing in, with no window. The
/// session clicks, types and checks as a `.raventest` does; its folder goes when the session goes.
pub fn with_plugin(plugin: Arc<dyn CiPlugin>) -> Result<ravenrs_e2e::E2e, String> {
    let dir = folder(&format!("plugin-{}", plugin.name()));
    session_with(&dir, vec![plugin])
}

/// Plays every scenario `plugin` brings, each on a session of its own with the plugin on; how many
/// it played, or the first that failed and why.
pub fn play_plugin_scenarios(plugin: &Arc<dyn CiPlugin>) -> Result<usize, String> {
    let scenarios = plugin.scenarios();
    for (n, text) in scenarios.iter().enumerate() {
        let scenario = ravenrs_e2e_wire::Scenario::parse(text)
            .map_err(|e| format!("{}: cenário {n}: {e}", plugin.name()))?;
        let dir = folder(&format!("plugin-{}-{n}", plugin.name()));
        let mut session = session_with(&dir, vec![plugin.clone()])?;
        let played = scenario.play(&mut session).map_err(|e| format!("{}: {e}", plugin.name()));
        drop(session);
        let _ = std::fs::remove_dir_all(&dir);
        played?;
    }
    Ok(scenarios.len())
}
