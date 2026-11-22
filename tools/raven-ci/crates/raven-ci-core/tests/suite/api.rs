//! The HTTP API of the server (#873), through the service itself with no HTTP: who comes in, what
//! the core answers, the routes of the plugins (public, signed in, switched off), and the triggers
//! the jobs name.

use std::sync::Arc;

use raven_ci_core::api::{create_user, service};
use raven_ci_core::{Ci, JobSpec};
use raven_ci_plugin_api::testing::Fake;
use raven_ci_plugin_api::{
    BuildRequest, CiPlugin, HttpRequest, HttpResponse, Point, ROLE_ADMIN, ROLE_READ, ROLE_RUN, Route,
};
use ravenrs_app::{Module, ModuleManifest};
use ravenrs_app_server::{ApiRequest, ApiService};
use serde_json::{Value, json};

use crate::common::OnDisk;

const PASSWORD: &str = "uma-senha-boa-de-teste";

/// A plugin with a public route under /hooks/, one for people signed in that says who they are,
/// and one that answers text.
struct Rotas;

impl Module for Rotas {
    fn name(&self) -> &str {
        "rotas"
    }
    fn manifest(&self) -> ModuleManifest {
        ModuleManifest::new().version("1.0.0").unwrap_or_default()
    }
}

impl CiPlugin for Rotas {
    fn routes(&self) -> Vec<Route> {
        vec![
            Route::new("POST", "/hooks/teste", |request: &HttpRequest| {
                HttpResponse::json(202, &json!({ "recebi": request.body.len(), "quem": request.actor }))
            })
            .public(),
            Route::new("GET", "/api/rotas/quem", |request: &HttpRequest| {
                HttpResponse::json(200, &json!({ "quem": request.actor, "papeis": request.roles, "roda": request.may_run() }))
            }),
            Route::new("GET", "/api/rotas/texto", |_: &HttpRequest| HttpResponse::text(200, "olá")),
        ]
    }
}

struct Server {
    _disk: OnDisk,
    ci: Arc<Ci>,
    api: ApiService,
}

fn server(name: &str) -> Server {
    let disk = OnDisk::new(name);
    let ci = Arc::new(disk.open(raven_ci_core::MemorySecrets::default()));
    ci.add_plugin(Arc::new(Rotas));
    ci.add_plugin(Arc::new(Fake::new(Point::Step)));
    ci.add_plugin(Arc::new(Fake::new(Point::PipelineFormat)));
    ci.apply_plugins().expect("on");
    let users = raven_ci_core::api::users_file(&disk.dir);
    assert_eq!(create_user(&users, "ana", PASSWORD, ROLE_READ).expect("first"), ROLE_ADMIN, "the first is an admin");
    create_user(&users, "bia", PASSWORD, ROLE_RUN).expect("bia");
    create_user(&users, "caio", PASSWORD, ROLE_READ).expect("caio");
    assert!(create_user(&users, "bia", PASSWORD, ROLE_RUN).is_err(), "a name already there");
    assert!(create_user(&users, "davi", PASSWORD, "Chefes").is_err(), "a role that does not exist");
    let api = service(ci.clone(), &users).expect("the service");
    Server { _disk: disk, ci, api }
}

impl Server {
    fn token(&mut self, name: &str) -> String {
        let answer = self.api.handle(
            ApiRequest::new("POST", "/api/auth/token").json(&json!({ "name": name, "password": PASSWORD })),
        );
        assert_eq!(answer.status, 200, "{answer:?}");
        answer.body["token"].as_str().expect("a token").to_owned()
    }

    fn get(&mut self, path: &str, token: Option<&str>) -> (u16, Value) {
        let (path, query) = path.split_once('?').unwrap_or((path, ""));
        let mut request = ApiRequest::new("GET", path);
        for pair in query.split('&').filter(|p| !p.is_empty()) {
            let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
            request = request.query(k, v);
        }
        if let Some(token) = token {
            request = request.bearer(token);
        }
        let answer = self.api.handle(request);
        let body = match &answer.raw {
            Some((_, bytes)) => serde_json::from_slice(bytes).unwrap_or_else(|_| json!(String::from_utf8_lossy(bytes))),
            None => answer.body.clone(),
        };
        (answer.status, body)
    }
}

#[test]
fn no_one_signed_in_reads_nothing_of_the_ci() {
    let mut server = server("api-anonimo");
    for path in ["/api/ci/jobs", "/api/ci/queue", "/api/ci/build?job=x&number=1", "/api/rotas/quem", "/api/me"] {
        assert_eq!(server.get(path, None).0, 401, "{path}");
        assert_eq!(server.get(path, Some("um-token-que-nao-vale")).0, 401, "{path}");
    }
}

#[test]
fn someone_signed_in_reads_the_jobs_a_build_and_only_the_new_lines_of_its_log() {
    let mut server = server("api-leitura");
    server.ci.save_job(JobSpec::new("apps/site", "fake-lines", "Build: compila\nTeste: testa")).expect("saved");
    let n = server.ci.request_build(BuildRequest::new("apps/site", "teste")).expect("asked").number;
    server.ci.work_until_idle("e1").expect("runs");
    let token = server.token("caio");
    let (status, jobs) = server.get("/api/ci/jobs", Some(&token));
    assert_eq!(status, 200);
    assert_eq!(jobs[0]["name"], "apps/site");
    let (status, build) = server.get(&format!("/api/ci/build?job=apps/site&number={n}"), Some(&token));
    assert_eq!(status, 200, "{build}");
    assert_eq!(build["build"]["status"], "succeeded");
    assert_eq!(build["stages"].as_array().map(Vec::len), Some(2));
    let (_, all) = server.get(&format!("/api/ci/log?job=apps/site&number={n}"), Some(&token));
    let lines = all.as_array().expect("lines");
    assert!(lines.len() >= 2, "{all}");
    let second = lines[1]["seq"].as_u64().expect("seq");
    let (_, after) = server.get(&format!("/api/ci/log?job=apps/site&number={n}&after={second}"), Some(&token));
    assert_eq!(after.as_array().map(Vec::len), Some(lines.len() - 2));
    let (status, missing) = server.get("/api/ci/build?job=apps/site&number=99", Some(&token));
    assert_eq!(status, 404, "{missing}");
    let (status, _) = server.get("/api/ci/build?job=apps/site", Some(&token));
    assert_eq!(status, 400, "the number is missing");
}

#[test]
fn a_route_of_a_plugin_knows_who_called_and_a_public_one_needs_no_one() {
    let mut server = server("api-rotas");
    let hook = server.api.handle(ApiRequest::new("POST", "/hooks/teste").bytes(b"{}".to_vec()));
    assert_eq!(hook.status, 202);
    let body: Value = serde_json::from_slice(&hook.raw.expect("raw").1).expect("json");
    assert_eq!(body, json!({ "recebi": 2, "quem": null }));
    let (bia, ana, caio) = (server.token("bia"), server.token("ana"), server.token("caio"));
    let (_, said) = server.get("/api/rotas/quem", Some(&bia));
    assert_eq!(said, json!({ "quem": "bia", "papeis": [ROLE_RUN], "roda": true }));
    let (_, said) = server.get("/api/rotas/quem", Some(&ana));
    assert_eq!(said["roda"], true, "an administrator may run: {said}");
    let (_, said) = server.get("/api/rotas/quem", Some(&caio));
    assert_eq!(said["roda"], false, "someone who only reads may not: {said}");
    let text = server.api.handle(ApiRequest::new("GET", "/api/rotas/texto").bearer(&bia));
    assert_eq!(text.raw, Some(("text/plain; charset=utf-8".to_owned(), "olá".as_bytes().to_vec())));
}

#[test]
fn the_routes_of_a_plugin_switched_off_are_gone_and_come_back_with_no_restart() {
    let mut server = server("api-desliga");
    let token = server.token("bia");
    assert_eq!(server.get("/api/rotas/quem", Some(&token)).0, 200);
    server.ci.disable_plugin("rotas").expect("off");
    assert_eq!(server.get("/api/rotas/quem", Some(&token)).0, 404);
    assert_eq!(server.api.handle(ApiRequest::new("POST", "/hooks/teste")).status, 401, "no route, no way in");
    server.ci.enable_plugin("rotas").expect("on");
    assert_eq!(server.get("/api/rotas/quem", Some(&token)).0, 200);
}

#[test]
fn the_triggers_the_jobs_name_are_watched_and_a_wrong_one_is_said() {
    let disk = OnDisk::new("api-gatilhos");
    let ci = Arc::new(disk.open(raven_ci_core::MemorySecrets::default()));
    let trigger = Fake::new(Point::Trigger);
    let probe = trigger.probe();
    ci.add_plugin(Arc::new(trigger));
    ci.add_plugin(Arc::new(Fake::new(Point::Step)));
    ci.add_plugin(Arc::new(Fake::new(Point::PipelineFormat)));
    ci.apply_plugins().expect("on");
    let watched = json!({ "triggers": [{ "id": "fake-trigger" }] });
    ci.save_job(JobSpec::new("vigiado", "fake-lines", "Build: ok").settings(watched)).expect("saved");
    let broken = json!({ "triggers": [{ "id": "fake-trigger", "broken": true }, { "id": "cron" }] });
    ci.save_job(JobSpec::new("quebrado", "fake-lines", "Build: ok").settings(broken)).expect("saved");
    let problems = ci.watch_triggers();
    assert_eq!(probe.watched(), ["vigiado"]);
    assert_eq!(problems.len(), 2, "{problems:?}");
    assert!(problems.iter().any(|p| p.contains("o gatilho cron de quebrado: falta o plugin")), "{problems:?}");
    let number = probe.fire("vigiado").expect("fired");
    ci.work_until_idle("e1").expect("runs");
    assert_eq!(ci.build("vigiado", number).expect("build").cause, "disparado pelo gatilho falso");
    ci.unwatch_triggers();
    assert!(probe.watched().is_empty(), "the core lets go of itself");
}
