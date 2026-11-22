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
use raven_ci_plugin_api::testing::Fake;
use raven_ci_plugin_api::{
    BuildRequest, CiPlugin, HttpRequest, HttpResponse, Point, ROLE_READ, ROLE_RUN, Route,
};
use ravenrs::prelude::*;
use ravenrs_app::{ModelNode, Module, ModuleManifest};
use ravenrs_app_remote::{RemoteClient, RemoteRequest, RemoteResponse, Transport};
use ravenrs_app_server::{ApiRequest, ApiService};
use serde_json::json;

/// The password of the three people of the tests.
pub const PASSWORD: &str = "senha-do-teste-e2e";

/// The scenarios the panel brings, one per file of `tests/e2e/`: the tool gets them when it opens
/// the panel, and `tests/scenarios.rs` plays them.
pub const SCENARIOS: [&str; 5] = [
    include_str!("../tests/e2e/entrar.raventest"),
    include_str!("../tests/e2e/build.raventest"),
    include_str!("../tests/e2e/disparar.raventest"),
    include_str!("../tests/e2e/fila.raventest"),
    include_str!("../tests/e2e/plugins.raventest"),
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

/// The core of the tests in `folder`, with its plugins, people and jobs, and a client that talks
/// to it with no HTTP.
pub fn server(folder: &Path) -> Result<(Arc<Ci>, RemoteClient), String> {
    let failed = |e: &dyn std::fmt::Display| e.to_string();
    let ci = Arc::new(Ci::open(folder).map_err(|e| failed(&e))?);
    ci.add_plugin(Arc::new(Fake::new(Point::Step)));
    ci.add_plugin(Arc::new(Fake::new(Point::PipelineFormat)));
    ci.add_plugin(Arc::new(raven_ci_plugin_api_trigger::ApiTriggerPlugin::new()));
    ci.add_plugin(Arc::new(Demonstracao));
    ci.apply_plugins().map_err(|e| failed(&e))?;
    let users = users_file(folder);
    create_user(&users, "ana", PASSWORD, ROLE_READ).map_err(|e| failed(&e))?;
    create_user(&users, "bia", PASSWORD, ROLE_RUN).map_err(|e| failed(&e))?;
    create_user(&users, "caio", PASSWORD, ROLE_READ).map_err(|e| failed(&e))?;
    let jobs = [
        (
            "apps/site",
            "Checkout: buscar o código\nLint: cargo clippy\nTestes: cargo nextest run\nImagem: docker build",
        ),
        ("apps/loja-web", "Instalar: pnpm install\nTestes: pnpm test"),
        ("api-pedidos", "Compilar: cargo build\nTestes: !fail"),
        ("imagens-base", "Imagem: docker build"),
    ];
    for (name, pipeline) in jobs {
        // Every job takes requests from the API, as Disparar asks.
        let spec =
            JobSpec::new(name, "fake-lines", pipeline).settings(json!({ "triggers": [{ "id": "api" }] }));
        ci.save_job(spec).map_err(|e| failed(&e))?;
    }
    for job in ["apps/site", "apps/loja-web", "api-pedidos", "apps/site"] {
        ci.request_build(BuildRequest::new(job, "Webhook do GitHub")).map_err(|e| failed(&e))?;
        ci.work_until_idle("executor-1").map_err(|e| failed(&e))?;
    }
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
    let (ci, painel) = panel(folder)?;
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
    let hello = ravenrs_e2e_wire::Hello::of("Raven CI").scenarios(SCENARIOS);
    let served = session(&folder("canal")).map_err(std::io::Error::other).and_then(|session| {
        ravenrs_e2e_wire::serve_filmed(hello, session, ravenrs::movie::ChannelFilm::default())
    });
    if let Err(error) = served {
        eprintln!("o canal de teste parou: {error}");
        std::process::exit(1);
    }
    true
}
