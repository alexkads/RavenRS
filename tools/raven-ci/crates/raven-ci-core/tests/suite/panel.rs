//! What the panel reads and does (#884–#890), through the service with no HTTP: the screens of the
//! plugins on, the overview, the occupancy, acting on builds by role, the plugins, people, the
//! audit, the files of the panel and the live watch.

use std::cell::RefCell;
use std::sync::Arc;

use raven_ci_core::api::{create_user, service_with};
use raven_ci_core::panel::{LiveWatch, PanelOptions};
use raven_ci_core::{Ci, JobSpec};
use raven_ci_plugin_api::testing::Fake;
use raven_ci_plugin_api::{BuildRequest, CiPlugin, Point, ROLE_READ, ROLE_RUN};
use ravenrs_app::{ModelNode, Module, ModuleManifest};
use ravenrs_app_server::{ApiRequest, ApiService};
use serde_json::{Value, json};

use crate::common::OnDisk;

const PASSWORD: &str = "uma-senha-boa-de-teste";

/// A plugin that puts a page in the menu and a tab on the page of a build, the way a module of
/// `ravenrs-app` puts a dashboard in the model.
struct Telas;

impl Module for Telas {
    fn name(&self) -> &str {
        "telas"
    }
    fn manifest(&self) -> ModuleManifest {
        ModuleManifest::new().version("1.0.0").unwrap_or_default()
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
            views.children.push(dashboard("travas", "Travas", "/api/ci/telas/travas"));
            views.children.push(dashboard("cobertura", "Cobertura", "/api/ci/telas/{job}/{number}"));
        }
        if let Some(navigation) = root.children.iter_mut().find(|n| n.id == "NavigationItems") {
            navigation.children.push(ModelNode::new("Servidor").with("Caption", "Servidor").child(
                ModelNode::new("travas").with("Caption", "Travas").with("View", "travas_DashboardView"),
            ));
            navigation.children.push(
                ModelNode::new("build").with("Caption", "build").child(
                    ModelNode::new("cobertura")
                        .with("Caption", "Cobertura")
                        .with("View", "cobertura_DashboardView"),
                ),
            );
        }
    }
}

impl CiPlugin for Telas {}

struct Painel {
    _disk: OnDisk,
    ci: Arc<Ci>,
    api: ApiService,
    files: std::path::PathBuf,
}

fn painel(name: &str) -> Painel {
    let disk = OnDisk::new(name);
    let ci = Arc::new(disk.open(raven_ci_core::MemorySecrets::default()));
    ci.add_plugin(Arc::new(Telas));
    ci.add_plugin(Arc::new(Fake::new(Point::Step)));
    ci.add_plugin(Arc::new(Fake::new(Point::PipelineFormat)));
    ci.apply_plugins().expect("on");
    let users = raven_ci_core::api::users_file(&disk.dir);
    create_user(&users, "ana", PASSWORD, ROLE_READ).expect("the first, an administrator");
    create_user(&users, "bia", PASSWORD, ROLE_RUN).expect("bia");
    create_user(&users, "caio", PASSWORD, ROLE_READ).expect("caio");
    let files = disk.dir.join("web");
    std::fs::create_dir_all(files.join("pkg")).expect("the folder of the panel");
    std::fs::write(files.join("index.html"), "<!doctype html><title>Raven CI</title>").expect("index");
    std::fs::write(files.join("pkg/painel.wasm"), b"\0asm").expect("wasm");
    let options = PanelOptions {
        executors: 2,
        files: Some(files.clone()),
        version: "0.1.0".to_owned(),
        users_file: None,
    };
    let api = service_with(ci.clone(), &users, options).expect("the service");
    Painel { _disk: disk, ci, api, files }
}

impl Painel {
    fn token(&mut self, name: &str) -> String {
        let answer = self.api.handle(
            ApiRequest::new("POST", "/api/auth/token").json(&json!({ "name": name, "password": PASSWORD })),
        );
        assert_eq!(answer.status, 200, "{answer:?}");
        answer.body["token"].as_str().expect("a token").to_owned()
    }

    fn ask(&mut self, method: &str, path: &str, token: Option<&str>, body: Option<Value>) -> (u16, Value) {
        let (path, query) = path.split_once('?').unwrap_or((path, ""));
        let mut request = ApiRequest::new(method, path);
        for pair in query.split('&').filter(|p| !p.is_empty()) {
            let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
            request = request.query(k, v);
        }
        if let Some(token) = token {
            request = request.bearer(token);
        }
        if let Some(body) = body {
            request = request.json(&body);
        }
        let answer = self.api.handle(request);
        let body = match &answer.raw {
            Some((_, bytes)) => {
                serde_json::from_slice(bytes).unwrap_or_else(|_| json!(String::from_utf8_lossy(bytes)))
            }
            None => answer.body.clone(),
        };
        (answer.status, body)
    }

    fn get(&mut self, path: &str, token: &str) -> (u16, Value) {
        self.ask("GET", path, Some(token), None)
    }

    fn post(&mut self, path: &str, token: &str, body: Value) -> (u16, Value) {
        self.ask("POST", path, Some(token), Some(body))
    }

    /// A job with two stages, built once.
    fn built(&self, job: &str) -> u64 {
        self.ci.save_job(JobSpec::new(job, "fake-lines", "Compilar: compila\nTestar: testa")).expect("saved");
        let n = self.ci.request_build(BuildRequest::new(job, "teste")).expect("asked").number;
        self.ci.work_until_idle("executor-1").expect("runs");
        n
    }
}

#[test]
fn a_plugin_puts_a_page_and_a_tab_and_switched_off_takes_them_away() {
    let mut painel = painel("painel-telas");
    let token = painel.token("caio");
    let (status, screens) = painel.get("/api/ci/screens", &token);
    assert_eq!(status, 200, "{screens}");
    let screens = screens.as_array().expect("a list");
    assert_eq!(screens.len(), 2, "{screens:?}");
    let page = screens.iter().find(|s| s["kind"] == "page").expect("a page");
    assert_eq!(page["caption"], "Travas");
    assert_eq!(page["group"], "Servidor");
    assert_eq!(page["items"][0]["source"], "/api/ci/telas/travas");
    let tab = screens.iter().find(|s| s["kind"] == "build_tab").expect("a tab");
    assert_eq!(tab["items"][0]["source"], "/api/ci/telas/{job}/{number}");
    painel.ci.disable_plugin("telas").expect("off");
    let (_, screens) = painel.get("/api/ci/screens", &token);
    assert_eq!(screens, json!([]), "a plugin switched off leaves the panel");
}

#[test]
fn the_overview_has_every_job_with_its_last_build_and_trend() {
    let mut painel = painel("painel-visao");
    painel.built("apps/site");
    painel.built("apps/site");
    painel.ci.save_job(JobSpec::new("api", "fake-lines", "Compilar: compila")).expect("a job never built");
    let token = painel.token("caio");
    let (status, overview) = painel.get("/api/ci/overview", &token);
    assert_eq!(status, 200, "{overview}");
    let jobs = overview["jobs"].as_array().expect("jobs");
    let site = jobs.iter().find(|j| j["name"] == "apps/site").expect("the site");
    assert_eq!(site["folder"], "apps");
    assert_eq!(site["last"]["number"], 2);
    assert_eq!(site["last"]["status"], "succeeded");
    assert_eq!(site["trend_ms"].as_array().map(Vec::len), Some(2));
    let never = jobs.iter().find(|j| j["name"] == "api").expect("the api");
    assert!(never["last"].is_null());
    assert_eq!(overview["formats"], json!(["fake-lines"]));
    let (_, server) = painel.get("/api/ci/server", &token);
    assert_eq!(server["executors"], 2);
    assert_eq!(server["version"], "0.1.0");
}

#[test]
fn the_occupancy_says_which_executor_ran_each_build() {
    let mut painel = painel("painel-ocupacao");
    let n = painel.built("site");
    let token = painel.token("caio");
    let (status, occupancy) = painel.get("/api/ci/occupancy?since_ms=0", &token);
    assert_eq!(status, 200, "{occupancy}");
    let names: Vec<&str> = occupancy["executors"]
        .as_array()
        .expect("executors")
        .iter()
        .filter_map(|e| e["name"].as_str())
        .collect();
    assert_eq!(names, ["executor-1", "executor-2"]);
    let build = &occupancy["builds"][0];
    assert_eq!(build["number"], n);
    assert_eq!(build["worker"], "executor-1");
    assert!(build["start_ms"].as_i64().is_some() && build["end_ms"].as_i64().is_some(), "{build}");
}

#[test]
fn only_executors_and_administrators_act_on_builds() {
    let mut painel = painel("painel-acoes");
    let n = painel.built("site");
    let (caio, bia) = (painel.token("caio"), painel.token("bia"));
    let asked = json!({ "job": "site", "number": n });
    for path in ["/api/ci/cancel", "/api/ci/rebuild", "/api/ci/priority"] {
        assert_eq!(painel.post(path, &caio, asked.clone()).0, 403, "{path}: a reader");
    }
    let (status, again) = painel.post("/api/ci/rebuild", &bia, asked);
    assert_eq!(status, 200, "{again}");
    let again = again["number"].as_u64().expect("a number");
    let (status, cancelled) = painel.post("/api/ci/cancel", &bia, json!({ "job": "site", "number": again }));
    assert_eq!(status, 200, "{cancelled}");
    assert_eq!(cancelled["status"], "cancelled");
    let (status, _) = painel.post("/api/ci/cancel", &bia, json!({ "job": "site" }));
    assert_eq!(status, 400, "the number is missing");
}

#[test]
fn a_pipeline_is_checked_with_its_lines_and_saved_only_by_an_administrator() {
    let mut painel = painel("painel-pipeline");
    let (caio, ana) = (painel.token("caio"), painel.token("ana"));
    let wrong = json!({ "format": "fake-lines", "text": "Compilar: compila\nsem dois pontos" });
    let (status, checked) = painel.post("/api/ci/pipeline/check", &caio, wrong.clone());
    assert_eq!(status, 200);
    assert_eq!(checked["errors"][0]["line"], 2, "{checked}");
    let job = |text: &str| json!({ "name": "novo", "format": "fake-lines", "pipeline": text });
    assert_eq!(painel.post("/api/ci/job", &caio, job("Compilar: compila")).0, 403);
    let (status, refused) = painel.post("/api/ci/job", &ana, job("sem dois pontos"));
    assert_eq!(status, 400, "{refused}");
    assert_eq!(refused["errors"][0]["line"], 1);
    let (status, _) = painel.post("/api/ci/job", &ana, job("Compilar: compila"));
    assert_eq!(status, 200);
    assert_eq!(painel.ci.job("novo").expect("saved").pipeline, "Compilar: compila");
    let (status, _) = painel.post("/api/ci/pipeline/check", &caio, json!({ "format": "yaml", "text": "" }));
    assert_eq!(status, 404, "no plugin reads that format");
}

#[test]
fn an_administrator_switches_plugins_makes_people_and_reads_the_audit() {
    let mut painel = painel("painel-admin");
    let (bia, ana) = (painel.token("bia"), painel.token("ana"));
    for path in ["/api/ci/plugins", "/api/ci/users", "/api/ci/audit"] {
        assert_eq!(painel.get(path, &bia).0, 403, "{path}: an executor");
    }
    let (status, plugins) = painel.get("/api/ci/plugins", &ana);
    assert_eq!(status, 200);
    assert!(plugins.as_array().expect("a list").iter().any(|p| p["name"] == "telas"));
    let (status, _) = painel.post("/api/ci/plugins/disable", &ana, json!({ "name": "telas" }));
    assert_eq!(status, 200);
    assert!(painel.ci.registry().on().iter().all(|p| p.name() != "telas"));
    let (status, _) = painel.post("/api/ci/plugins/enable", &ana, json!({ "name": "telas" }));
    assert_eq!(status, 200);
    let made = json!({ "name": "davi", "password": PASSWORD, "role": ROLE_READ });
    let (status, said) = painel.post("/api/ci/users", &ana, made);
    assert_eq!(status, 200, "{said}");
    let (_, users) = painel.get("/api/ci/users", &ana);
    let davi = users.as_array().expect("people").iter().find(|u| u["name"] == "davi").expect("davi");
    assert_eq!(davi["roles"], json!([ROLE_READ]));
    let davi = painel.token("davi");
    assert_eq!(painel.get("/api/ci/overview", &davi).0, 200, "davi reads");
    let (_, audit) = painel.get("/api/ci/audit", &ana);
    let said: Vec<&str> =
        audit.as_array().expect("lines").iter().filter_map(|l| l["what"].as_str()).collect();
    assert!(said.contains(&"desligou o plugin telas"), "{said:?}");
    assert!(said.contains(&"criou o usuário davi com o papel Leitores"), "{said:?}");
}

#[test]
fn the_files_of_the_panel_come_with_no_one_signed_in_and_never_from_outside_its_folder() {
    let mut painel = painel("painel-arquivos");
    let index = painel.api.handle(ApiRequest::new("GET", "/painel/"));
    assert_eq!(index.status, 200);
    let (kind, bytes) = index.raw.expect("raw");
    assert!(kind.starts_with("text/html"));
    assert!(String::from_utf8_lossy(&bytes).contains("Raven CI"));
    let wasm = painel.api.handle(ApiRequest::new("GET", "/painel/pkg/painel.wasm"));
    assert_eq!(wasm.raw.expect("raw").0, "application/wasm");
    assert!(painel.files.join("index.html").exists());
    let outside = painel.api.handle(ApiRequest::new("GET", "/painel/../usuarios.sqlite"));
    assert!(outside.status == 400 || outside.status == 404, "{outside:?}");
    let missing = painel.api.handle(ApiRequest::new("GET", "/painel/nada.js"));
    assert_eq!(missing.status, 404);
    let token = painel.token("caio");
    assert_eq!(painel.get("/api/ci/screens", &token).0, 200);
}

#[test]
fn the_live_watch_tells_the_lines_and_the_status_of_builds() {
    let painel = painel("painel-ao-vivo");
    let n = painel.built("site");
    let heard = RefCell::new(Vec::new());
    LiveWatch::look_once(&painel.ci, &|kind, data| heard.borrow_mut().push((kind.to_owned(), data)));
    let heard = heard.into_inner();
    assert!(
        heard
            .iter()
            .any(|(kind, data)| kind == "build" && data["number"] == n && data["status"] == "succeeded"),
        "{heard:?}"
    );
    let watch = LiveWatch::start(painel.ci.clone(), std::time::Duration::from_millis(10), |_, _| {});
    drop(watch);
}

#[test]
fn the_build_says_when_each_stage_ran_and_which_plugins_it_used() {
    let mut painel = painel("painel-estagios");
    let n = painel.built("site");
    let token = painel.token("caio");
    let (status, body) = painel.get(&format!("/api/ci/build?job=site&number={n}"), &token);
    assert_eq!(status, 200, "{body}");
    // The time of a stage comes from the lines its steps wrote (a `sh` writes each line it
    // prints); the steps of the tests write none, so no stage has a time.
    assert_eq!(body["stage_times"], json!([]), "{body}");
    assert!(body["started_ms"].as_i64().is_some(), "{body}");
    let plugins: Vec<&str> = body["plugins"].as_array().expect("plugins").iter().filter_map(Value::as_str).collect();
    assert!(plugins.contains(&"fake-step") && plugins.contains(&"fake-pipeline-format"), "{plugins:?}");
    assert_eq!(body["build"]["worker"], "executor-1");
    assert_eq!(body["build"]["commit"], Value::Null, "a job with no code fetches no commit");
}

#[test]
fn the_history_has_every_build_newest_first_and_pages_by_time() {
    let mut painel = painel("painel-historico");
    let first = painel.built("site");
    let second = painel.built("api");
    let token = painel.token("caio");
    let (status, body) = painel.get("/api/ci/history?limit=1", &token);
    assert_eq!(status, 200, "{body}");
    let builds = body["builds"].as_array().expect("builds");
    assert_eq!(builds.len(), 1, "the limit");
    assert_eq!((builds[0]["job"].as_str(), builds[0]["number"].as_u64()), (Some("api"), Some(second)));
    let before = builds[0]["created_ms"].as_i64().expect("when") + 1;
    let (_, all) = painel.get(&format!("/api/ci/history?before_ms={before}"), &token);
    let jobs: Vec<(&str, u64)> = all["builds"]
        .as_array()
        .expect("builds")
        .iter()
        .filter_map(|b| Some((b["job"].as_str()?, b["number"].as_u64()?)))
        .collect();
    assert!(jobs.contains(&("site", first)) && jobs.contains(&("api", second)), "{jobs:?}");
    assert_eq!(painel.ask("GET", "/api/ci/history", None, None).0, 401, "only for who signed in");
}

#[test]
fn the_overview_and_the_occupancy_tell_the_agents_and_what_runs_now() {
    let mut painel = painel("painel-agentes");
    painel.built("site");
    let token = painel.token("caio");
    let (_, overview) = painel.get("/api/ci/overview", &token);
    assert_eq!(overview["agents"]["total"], 0, "{overview}");
    assert_eq!(overview["running_now"], json!([]), "nothing runs after the build");
    assert_eq!(overview["jobs"][0]["last"]["worker"], "executor-1", "{overview}");
    let (_, occupancy) = painel.get("/api/ci/occupancy?since_ms=0", &token);
    assert_eq!(occupancy["agents"], json!([]), "{occupancy}");
    assert_eq!(occupancy["templates"], json!([]), "{occupancy}");
    assert_eq!(occupancy["builds"][0]["waiting"], Value::Null, "an ended build waits for nothing");
}
