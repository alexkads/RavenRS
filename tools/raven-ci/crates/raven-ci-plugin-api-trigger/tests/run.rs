//! The plugin api-trigger (#874), through the service of the core with no HTTP: who may ask, which
//! jobs take requests, parameters, and the number of the build.
#![allow(clippy::expect_used, reason = "tests fail loudly")]

use std::sync::Arc;

use raven_ci_core::api::{create_user, service, users_file};
use raven_ci_core::{Ci, CiOptions, JobSpec};
use raven_ci_plugin_api::testing::Fake;
use raven_ci_plugin_api::{BuildStatus, Point, ROLE_READ, ROLE_RUN};
use raven_ci_plugin_api_trigger::ApiTriggerPlugin;
use ravenrs_app_server::{ApiRequest, ApiService};
use serde_json::{Value, json};

const PASSWORD: &str = "senha-boa-de-teste";

struct Setup {
    dir: std::path::PathBuf,
    ci: Arc<Ci>,
    api: ApiService,
}

impl Drop for Setup {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn setup(name: &str) -> Setup {
    let dir = std::env::temp_dir().join(format!("raven-ci-api-trigger-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let ci =
        Arc::new(Ci::new(CiOptions { data_dir: Some(dir.clone()), ..CiOptions::default() }).expect("a core"));
    ci.add_plugin(Arc::new(ApiTriggerPlugin::new()));
    ci.add_plugin(Arc::new(Fake::new(Point::Step)));
    ci.add_plugin(Arc::new(Fake::new(Point::PipelineFormat)));
    ci.apply_plugins().expect("on");
    let users = users_file(&dir);
    create_user(&users, "ana", PASSWORD, ROLE_READ).expect("the admin");
    create_user(&users, "bia", PASSWORD, ROLE_RUN).expect("bia");
    create_user(&users, "caio", PASSWORD, ROLE_READ).expect("caio");
    let api = service(ci.clone(), &users).expect("the service");
    Setup { dir, ci, api }
}

impl Setup {
    fn token(&mut self, name: &str) -> String {
        let answer = self.api.handle(
            ApiRequest::new("POST", "/api/auth/token").json(&json!({ "name": name, "password": PASSWORD })),
        );
        answer.body["token"].as_str().expect("a token").to_owned()
    }

    fn run(&mut self, token: Option<&str>, body: &Value) -> (u16, Value) {
        let mut request = ApiRequest::new("POST", "/api/ci/run").json(body);
        if let Some(token) = token {
            request = request.bearer(token);
        }
        let answer = self.api.handle(request);
        let body = answer
            .raw
            .map_or(answer.body, |(_, bytes)| serde_json::from_slice(&bytes).unwrap_or(Value::Null));
        (answer.status, body)
    }

    fn job(&self, name: &str, api: bool) {
        let pipeline = "Build: compila";
        let mut job = JobSpec::new(name, "fake-lines", pipeline);
        if api {
            job = job.settings(json!({ "triggers": [{ "id": "api" }] }));
        }
        self.ci.save_job(job).expect("saved");
    }
}

#[test]
fn someone_who_may_run_asks_for_a_build_and_gets_its_number() {
    let mut setup = setup("roda");
    setup.job("apps/site", true);
    assert!(setup.ci.watch_triggers().is_empty());
    let bia = setup.token("bia");
    let (status, said) =
        setup.run(Some(&bia), &json!({ "job": "apps/site", "parameters": { "ramo": "main" } }));
    assert_eq!(status, 201, "{said}");
    let number = said["number"].as_u64().expect("a number");
    let (_, again) = setup.run(Some(&bia), &json!({ "job": "apps/site", "parameters": { "ramo": "main" } }));
    assert_eq!(again["number"].as_u64(), Some(number), "an equal request joins the one in the queue");
    setup.ci.work_until_idle("e1").expect("runs");
    let build = setup.ci.build("apps/site", number).expect("build");
    assert_eq!(build.status, BuildStatus::Succeeded);
    assert_eq!((build.cause.as_str(), build.actor.as_deref()), ("pedido pela API", Some("bia")));
    assert_eq!(build.parameters["ramo"], "main");
    let ana = setup.token("ana");
    assert_eq!(setup.run(Some(&ana), &json!({ "job": "apps/site" })).0, 201, "an administrator may run");
}

#[test]
fn someone_who_only_reads_is_refused_and_no_one_is_asked_to_sign_in() {
    let mut setup = setup("recusa");
    setup.job("apps/site", true);
    setup.ci.watch_triggers();
    let caio = setup.token("caio");
    assert_eq!(setup.run(Some(&caio), &json!({ "job": "apps/site" })).0, 403);
    assert_eq!(setup.run(None, &json!({ "job": "apps/site" })).0, 401);
    assert!(setup.ci.queue().expect("queue").is_empty());
}

#[test]
fn a_job_that_does_not_take_requests_and_a_body_that_does_not_fit_are_said() {
    let mut setup = setup("erros");
    setup.job("fechado", false);
    setup.ci.watch_triggers();
    let bia = setup.token("bia");
    let (status, said) = setup.run(Some(&bia), &json!({ "job": "fechado" }));
    assert_eq!(status, 404);
    assert!(said["message"].as_str().unwrap_or_default().contains("não aceita pedidos pela API"), "{said}");
    assert_eq!(setup.run(Some(&bia), &json!({ "parameters": {} })).0, 400);
    assert_eq!(setup.run(Some(&bia), &json!({ "job": "fechado", "parameters": [1] })).0, 400);
}
