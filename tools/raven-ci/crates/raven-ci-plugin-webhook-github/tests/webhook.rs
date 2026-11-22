//! The webhook of GitHub (#910): signed events ask for builds and scans of the jobs of their
//! repository only, and anything not signed by a secret of a job is refused.
#![allow(clippy::expect_used, reason = "tests fail loudly")]

use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use raven_ci_plugin_api::testing::FakeHost;
use raven_ci_plugin_api::{BuildRequest, Builds, CiError, CiPlugin, HttpRequest, HttpResponse, Permission};
use raven_ci_plugin_webhook_github::{PATH, WebhookGithubPlugin};
use serde_json::{Value, json};

/// What a job was asked: builds (`job`, cause, who) and scans.
#[derive(Default)]
struct Asked {
    builds: Mutex<Vec<(String, String, Option<String>)>>,
    scans: Mutex<Vec<String>>,
}

impl Builds for Asked {
    fn request(&self, request: BuildRequest) -> Result<u64, CiError> {
        let mut builds = self.builds.lock().unwrap_or_else(PoisonError::into_inner);
        builds.push((request.job, request.cause, request.actor));
        Ok(builds.len() as u64)
    }

    fn scan(&self, job: &str) -> Result<Vec<String>, CiError> {
        self.scans.lock().unwrap_or_else(PoisonError::into_inner).push(job.to_owned());
        Ok(Vec::new())
    }
}

impl Asked {
    fn builds(&self) -> Vec<(String, String, Option<String>)> {
        self.builds.lock().unwrap_or_else(PoisonError::into_inner).clone()
    }

    fn scans(&self) -> Vec<String> {
        self.scans.lock().unwrap_or_else(PoisonError::into_inner).clone()
    }

    /// The scans, once `count` of them happened or two seconds passed: they run on a thread.
    fn scans_after(&self, count: usize) -> Vec<String> {
        let until = Instant::now() + Duration::from_secs(2);
        while self.scans().len() < count && Instant::now() < until {
            std::thread::sleep(Duration::from_millis(10));
        }
        self.scans()
    }
}

struct Setup {
    plugin: WebhookGithubPlugin,
    host: Arc<FakeHost>,
    asked: Arc<Asked>,
}

fn setup(jobs: &[(&str, Value)]) -> Setup {
    let plugin = WebhookGithubPlugin::new();
    let host = Arc::new(
        FakeHost::new("webhook-github", &[Permission::Secrets])
            .with_secret("segredo-site", "s3gr3do-do-site")
            .with_secret("segredo-api", "outro-segredo"),
    );
    plugin.start(host.clone()).expect("started");
    let asked = Arc::new(Asked::default());
    let trigger = plugin.triggers().remove(0);
    for (job, settings) in jobs {
        trigger.watch(job, settings, asked.clone()).expect("watched");
    }
    Setup { plugin, host, asked }
}

fn one_branch() -> Value {
    json!({ "id": "github-webhook", "repository": "dono/site", "secret": "segredo-site", "branch": "main" })
}

fn many_branches() -> Value {
    json!({ "id": "github-webhook", "repository": "dono/site", "secret": "segredo-site" })
}

/// A request of GitHub for `event`, with `body` signed by `secret` (none: not signed).
fn delivery(event: &str, body: &Value, secret: Option<&str>, id: &str) -> HttpRequest {
    let bytes = serde_json::to_vec(body).expect("json");
    let mut request =
        HttpRequest { method: "POST".into(), path: PATH.into(), body: bytes, ..HttpRequest::default() };
    request.headers.insert("x-github-event".into(), event.into());
    request.headers.insert("x-github-delivery".into(), id.into());
    if let Some(secret) = secret {
        request
            .headers
            .insert("x-hub-signature-256".into(), ravenrs_app_server::sign(secret.as_bytes(), &request.body));
    }
    request
}

fn post(setup: &Setup, request: &HttpRequest) -> (u16, Value) {
    let route = setup.plugin.routes().remove(0);
    assert!(route.public, "GitHub does not sign in");
    assert!(route.matches("POST", PATH));
    let HttpResponse { status, body, .. } = (route.handler)(request);
    (status, serde_json::from_slice(&body).unwrap_or(Value::Null))
}

fn push(branch: &str) -> Value {
    json!({
        "ref": format!("refs/heads/{branch}"),
        "after": "0123456789abcdef0123456789abcdef01234567",
        "deleted": false,
        "repository": { "full_name": "dono/site" },
        "pusher": { "name": "ana" },
    })
}

#[test]
fn a_signed_push_to_the_branch_of_a_job_asks_for_its_build_with_the_commit_and_who() {
    let setup = setup(&[("site", one_branch())]);
    let (status, body) = post(&setup, &delivery("push", &push("main"), Some("s3gr3do-do-site"), "d1"));
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["builds"], json!([{ "job": "site", "number": 1 }]));
    assert_eq!(
        setup.asked.builds(),
        [("site".to_owned(), "push de 0123456 em main (GitHub)".to_owned(), Some("ana".to_owned()))]
    );

    let (status, body) = post(&setup, &delivery("push", &push("outra"), Some("s3gr3do-do-site"), "d2"));
    assert_eq!(status, 202, "{body}");
    assert_eq!(body["ignored"], json!(["site: não vigia a branch outra"]));
    assert_eq!(setup.asked.builds().len(), 1);
}

#[test]
fn a_pull_request_that_moves_asks_for_a_scan_and_one_labeled_is_ignored() {
    let setup = setup(&[("site", many_branches())]);
    let event =
        |action: &str| json!({ "action": action, "number": 3, "repository": { "full_name": "dono/site" } });
    let (status, body) =
        post(&setup, &delivery("pull_request", &event("synchronize"), Some("s3gr3do-do-site"), "p1"));
    assert_eq!(status, 202, "{body}");
    assert_eq!(body["scans"], json!(["site"]));
    assert_eq!(setup.asked.scans_after(1), ["site"]);

    let (status, body) =
        post(&setup, &delivery("pull_request", &event("labeled"), Some("s3gr3do-do-site"), "p2"));
    assert_eq!(status, 202);
    assert!(body["message"].as_str().is_some_and(|m| m.contains("labeled")), "{body}");
    std::thread::sleep(Duration::from_millis(50));
    assert_eq!(setup.asked.scans(), ["site"], "no second scan");

    // A push to a job of many branches is a scan too, even one that deletes the branch.
    let mut deleted = push("feature/x");
    deleted["deleted"] = json!(true);
    post(&setup, &delivery("push", &deleted, Some("s3gr3do-do-site"), "p3"));
    assert_eq!(setup.asked.scans_after(2), ["site", "site"]);
}

#[test]
fn a_wrong_or_missing_signature_is_refused_with_a_line_in_the_log_and_nothing_asked() {
    let setup = setup(&[("site", one_branch()), ("monorepo", many_branches())]);
    let (status, body) = post(&setup, &delivery("push", &push("main"), Some("não é o segredo"), "x1"));
    assert_eq!(status, 401);
    assert_eq!(body["message"], "A notificação não foi aceita.", "no detail of which part failed");
    let (status, _) = post(&setup, &delivery("push", &push("main"), None, "x2"));
    assert_eq!(status, 401);

    let mut tampered = delivery("push", &push("main"), Some("s3gr3do-do-site"), "x3");
    tampered.body = serde_json::to_vec(&push("release")).expect("json");
    assert_eq!(post(&setup, &tampered).0, 401, "a body changed after it was signed");

    assert!(setup.asked.builds().is_empty());
    assert!(setup.asked.scans_after(0).is_empty());
    let lines = setup.host.lines();
    assert_eq!(lines.len(), 3, "{lines:?}");
    assert!(lines.iter().all(|l| l.starts_with("webhook do GitHub recusado")), "{lines:?}");
}

#[test]
fn a_secret_of_one_repository_moves_nothing_of_another() {
    let api = json!({ "id": "github-webhook", "repository": "dono/api", "secret": "segredo-api", "branch": "main" });
    let setup = setup(&[("site", one_branch()), ("api", api)]);
    // Signed with the secret of the site, saying it is the api.
    let mut body = push("main");
    body["repository"]["full_name"] = json!("dono/api");
    let (status, answer) = post(&setup, &delivery("push", &body, Some("s3gr3do-do-site"), "i1"));
    assert_eq!(status, 202, "{answer}");
    assert!(setup.asked.builds().is_empty());

    // Signed with the secret of the api, for the api: only the api.
    let (status, _) = post(&setup, &delivery("push", &body, Some("outro-segredo"), "i2"));
    assert_eq!(status, 200);
    let jobs: Vec<String> = setup.asked.builds().into_iter().map(|(job, _, _)| job).collect();
    assert_eq!(jobs, ["api"]);
}

#[test]
fn the_same_delivery_twice_asks_for_one_build() {
    let setup = setup(&[("site", one_branch())]);
    let request = delivery("push", &push("main"), Some("s3gr3do-do-site"), "repetida");
    assert_eq!(post(&setup, &request).0, 200);
    let (status, body) = post(&setup, &request);
    assert_eq!(status, 200);
    assert_eq!(body["duplicate"], json!(true));
    assert_eq!(setup.asked.builds().len(), 1);
}

#[test]
fn ping_tags_and_other_events_answer_without_asking() {
    let setup = setup(&[("site", one_branch())]);
    let ping = json!({ "zen": "Keep it logically awesome.", "repository": { "full_name": "dono/site" } });
    assert_eq!(post(&setup, &delivery("ping", &ping, Some("s3gr3do-do-site"), "z1")).0, 200);
    let mut tag = push("main");
    tag["ref"] = json!("refs/tags/v1.0");
    let (status, body) = post(&setup, &delivery("push", &tag, Some("s3gr3do-do-site"), "z2"));
    assert_eq!((status, body["message"].clone()), (202, json!("ignorado: refs/tags/v1.0 não é uma branch")));
    let (status, _) = post(&setup, &delivery("issues", &ping, Some("s3gr3do-do-site"), "z3"));
    assert_eq!(status, 202);
    assert!(setup.asked.builds().is_empty());
}

#[test]
fn a_job_without_repository_or_secret_is_refused_saying_the_field() {
    let plugin = WebhookGithubPlugin::new();
    let trigger = plugin.triggers().remove(0);
    let asked: Arc<dyn Builds> = Arc::new(Asked::default());
    let refused = |settings: Value| match trigger.watch("x", &settings, asked.clone()) {
        Err(CiError::Settings { field, .. }) => field,
        other => panic!("{:?}", other.map(|()| "watched")),
    };
    assert_eq!(refused(json!({ "id": "github-webhook", "secret": "s" })), "repository");
    assert_eq!(refused(json!({ "id": "github-webhook", "repository": "site", "secret": "s" })), "repository");
    assert_eq!(refused(json!({ "id": "github-webhook", "repository": "dono/site" })), "secret");
    assert_eq!(
        refused(json!({ "id": "github-webhook", "repository": "dono/site", "secret": "s", "branch": 3 })),
        "branch"
    );
}
