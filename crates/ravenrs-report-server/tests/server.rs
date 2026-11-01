//! The server driven as a client drives it: over HTTP, with a real SMTP
//! conversation, a data host, and the scheduler's clock in the test's hands.
#![allow(clippy::unwrap_used, clippy::expect_used, missing_docs)]

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};

use axum::Json;
use axum::routing::get;
use jiff::Timestamp;
use ravenrs_report::{Band, Control, DataSource, DataSourceKind, ParameterKind, Report};
use ravenrs_report_server::config::{Config, DataHost, SmtpConfig, SmtpSecurity};
use ravenrs_report_server::deliver::{Mail, Mailer, SmtpMailer};
use ravenrs_report_server::http::{AppState, router};
use ravenrs_report_server::job::Job;
use ravenrs_report_server::scheduler::Runner;
use ravenrs_report_server::store::{JobState, RunRecord, RunStatus, Store};
use serde_json::{Value, json};

const TOKEN: &str = "test-token-with-enough-length";

/// Records messages and fails for the addresses it is told to.
#[derive(Default)]
struct FakeMailer {
    sent: Mutex<Vec<Mail>>,
    fail_for: Mutex<Vec<String>>,
}

impl Mailer for FakeMailer {
    fn send(&self, mail: &Mail) -> Result<(), String> {
        if mail.to.iter().any(|to| self.fail_for.lock().unwrap().contains(to)) {
            return Err("mailbox unavailable".to_owned());
        }
        self.sent.lock().unwrap().push(mail.clone());
        Ok(())
    }
}

struct Server {
    _dir: tempfile::TempDir,
    url: String,
    store: Store,
    runner: Runner,
    mailer: Arc<FakeMailer>,
    output: std::path::PathBuf,
}

async fn start(change: impl FnOnce(&mut Config)) -> Server {
    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("out");
    std::fs::create_dir_all(&output).unwrap();
    let mut config = Config::new(dir.path().join("data"));
    config.retry_delay_seconds = 0;
    config.output_dir = Some(output.clone());
    config.smtp = Some(SmtpConfig {
        host: "unused".into(),
        port: None,
        security: SmtpSecurity::None,
        from: "r@example.com".into(),
        username_env: None,
        password_env: None,
    });
    change(&mut config);
    let config = Arc::new(config);
    let store = Store::open(&config.data_dir).unwrap();
    let mailer = Arc::new(FakeMailer::default());
    let dyn_mailer: Arc<dyn Mailer> = mailer.clone();
    let runner = Runner::new(store.clone(), config.clone(), Some(dyn_mailer));
    let app = router(AppState::new(store.clone(), runner.clone(), config, TOKEN.to_owned()));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, app).await });
    Server { _dir: dir, url, store, runner, mailer, output }
}

fn sales() -> Report {
    Report::new("Vendas")
        .culture("pt-BR")
        .parameter("Region", ParameterKind::Text, json!("Sul"))
        .filter("[Region] = ?Region")
        .data_source(DataSource {
            name: "Vendas".into(),
            kind: DataSourceKind::Json {
                data: json!([{ "Region": "Sul", "Name": "Ana", "Total": 10.5 }, { "Region": "Norte", "Name": "Bruno", "Total": 20 }]),
            },
        })
        .detail(
            Band::new(18.0)
                .control(Control::field("[Name]").at(0.0, 0.0, 200.0, 18.0))
                .control(Control::field("[Total]").format("C2").at(200.0, 0.0, 100.0, 18.0)),
        )
}

struct Answer {
    status: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl Answer {
    fn json(&self) -> Value {
        serde_json::from_slice(&self.body).unwrap()
    }
    fn header(&self, name: &str) -> &str {
        self.headers.iter().find(|(n, _)| n == name).map_or("", |(_, v)| v)
    }
}

async fn call(
    method: &'static str,
    url: String,
    token: Option<&'static str>,
    body: Option<String>,
) -> Answer {
    tokio::task::spawn_blocking(move || {
        let agent: ureq::Agent = ureq::Agent::config_builder().http_status_as_error(false).build().into();
        let builder = ureq::http::Request::builder().method(method).uri(&url);
        let builder = match token {
            Some(token) => builder.header("Authorization", format!("Bearer {token}")),
            None => builder,
        };
        let mut response = match body {
            Some(body) => agent.run(builder.body(body).unwrap()),
            None => agent.run(builder.body(()).unwrap()),
        }
        .unwrap();
        let headers = response
            .headers()
            .iter()
            .map(|(n, v)| (n.to_string(), v.to_str().unwrap_or_default().to_owned()))
            .collect();
        let body = response.body_mut().with_config().limit(1 << 30).read_to_vec().unwrap();
        Answer { status: response.status().as_u16(), headers, body }
    })
    .await
    .unwrap()
}

impl Server {
    async fn get(&self, path: &str) -> Answer {
        call("GET", format!("{}{path}", self.url), Some(TOKEN), None).await
    }
    async fn put(&self, path: &str, body: String) -> Answer {
        call("PUT", format!("{}{path}", self.url), Some(TOKEN), Some(body)).await
    }
    async fn post(&self, path: &str, body: Value) -> Answer {
        call("POST", format!("{}{path}", self.url), Some(TOKEN), Some(body.to_string())).await
    }
    async fn delete(&self, path: &str) -> Answer {
        call("DELETE", format!("{}{path}", self.url), Some(TOKEN), None).await
    }
    fn save(&self, id: &str, job: Value) -> Job {
        let job: Job = serde_json::from_value(job).unwrap();
        job.validate().unwrap();
        self.store.save_layout(&job.layout, &sales().to_json()).unwrap();
        self.store.save_job(id, &job).unwrap();
        job
    }
    async fn run(&self, id: &str, job: Value) -> RunRecord {
        let job = self.save(id, job);
        self.runner.run_now(id, &job).await
    }
}

fn email_job(to: &[&str], separately: bool) -> Value {
    json!({
        "layout": "sales",
        "schedule": "0 8 * * *",
        "timezone": "America/Recife",
        "formats": ["pdf", "xlsx"],
        "deliveries": [{ "type": "email", "to": to, "separately": separately, "subject": "{report} em {date}" }]
    })
}

#[tokio::test(flavor = "multi_thread")]
async fn every_route_but_health_needs_the_token() {
    let s = start(|_| {}).await;
    assert_eq!(call("GET", format!("{}/health", s.url), None, None).await.status, 200);
    for (method, path) in [
        ("GET", "/layouts"),
        ("GET", "/jobs"),
        ("POST", "/render"),
        ("PUT", "/layouts/a"),
        ("POST", "/jobs/a/run"),
    ] {
        let url = format!("{}{path}", s.url);
        let body = (method != "GET").then(String::new);
        assert_eq!(call(method, url.clone(), None, body.clone()).await.status, 401, "{path} without a token");
        assert_eq!(
            call(method, url, Some("wrong-token-of-enough-length"), body).await.status,
            401,
            "{path} wrong token"
        );
    }
    assert_eq!(s.get("/layouts").await.status, 200);
}

#[tokio::test(flavor = "multi_thread")]
async fn layouts_are_stored_with_versions() {
    let s = start(|_| {}).await;
    assert_eq!(s.get("/layouts/sales").await.status, 404);
    let saved = s.put("/layouts/sales", sales().to_json()).await;
    assert_eq!((saved.status, saved.json()["version"].clone()), (200, json!(1)));
    assert_eq!(
        s.put("/layouts/sales", sales().to_json()).await.json()["version"],
        1,
        "same content, same version"
    );
    assert_eq!(s.put("/layouts/sales", Report::new("Outro").to_json()).await.json()["version"], 2);
    assert_eq!(s.get("/layouts").await.json(), json!(["sales"]));
    assert_eq!(s.get("/layouts/sales/versions").await.json().as_array().unwrap().len(), 2);
    let first: Value = s.get("/layouts/sales?version=1").await.json();
    assert_eq!(first["name"], "Vendas");
    assert_eq!(s.get("/layouts/sales").await.json()["name"], "Outro");
    assert_eq!(s.delete("/layouts/sales").await.status, 204);
    assert_eq!(s.delete("/layouts/sales").await.status, 404);
}

#[tokio::test(flavor = "multi_thread")]
async fn bad_input_is_refused_with_a_reason() {
    let s = start(|_| {}).await;
    assert_eq!(s.put("/layouts/x", "not json".to_owned()).await.status, 400);
    assert_eq!(s.put("/layouts/..%2Fx", sales().to_json()).await.status, 400);
    assert_eq!(s.get("/layouts/..%2F..%2Fsecret").await.status, 400);
    assert_eq!(s.get("/jobs/..%2Fx/runs").await.status, 400);
    assert_eq!(s.post("/render", json!({ "format": "pdf" })).await.status, 400);
    assert_eq!(s.post("/render", json!({ "name": "nope", "format": "pdf" })).await.status, 404);
    assert_eq!(s.post("/render", json!({ "name": "a", "format": "docx" })).await.status, 400);
    assert_eq!(s.put("/jobs/x", json!({ "layout": "none", "schedule": "x" }).to_string()).await.status, 400);
    let missing_layout = s.put("/jobs/x", email_job(&["a@example.com"], false).to_string()).await;
    assert_eq!(missing_layout.status, 400);
    assert!(missing_layout.json()["error"].as_str().unwrap().contains("does not exist"));
}

#[tokio::test(flavor = "multi_thread")]
async fn large_bodies_are_refused() {
    let s = start(|c| c.max_body_megabytes = 1).await;
    // Written raw: the server answers before reading it all, and closing early
    // would be an error to a client that insists on finishing the request.
    let address = s.url.trim_start_matches("http://").to_owned();
    let answer = tokio::task::spawn_blocking(move || {
        let stream = std::net::TcpStream::connect(&address).unwrap();
        stream.set_read_timeout(Some(std::time::Duration::from_secs(20))).unwrap();
        let mut writer = stream.try_clone().unwrap();
        let size = 2 * 1024 * 1024;
        std::thread::spawn(move || {
            let head = format!("PUT /layouts/big HTTP/1.1\r\nHost: x\r\nAuthorization: Bearer {TOKEN}\r\nContent-Length: {size}\r\n\r\n");
            let _ = writer.write_all(head.as_bytes());
            let _ = writer.write_all(&vec![b'x'; size]);
        });
        let mut line = String::new();
        BufReader::new(stream).read_line(&mut line).unwrap();
        line
    })
    .await
    .unwrap();
    assert!(answer.contains("413"), "{answer}");
}

#[tokio::test(flavor = "multi_thread")]
async fn renders_on_demand_in_each_format_with_parameters() {
    let s = start(|_| {}).await;
    s.put("/layouts/sales", sales().to_json()).await;
    let pdf = s.post("/render", json!({ "name": "sales", "format": "pdf" })).await;
    assert_eq!(pdf.status, 200);
    assert!(pdf.body.starts_with(b"%PDF-"));
    assert_eq!(pdf.header("content-type"), "application/pdf");
    assert!(pdf.header("content-disposition").contains("Vendas"), "{}", pdf.header("content-disposition"));
    let xlsx = s.post("/render", json!({ "name": "sales", "format": "xlsx" })).await;
    assert!(xlsx.body.starts_with(b"PK"));
    let csv = s
        .post("/render", json!({ "name": "sales", "format": "csv", "parameters": { "Region": "Norte" } }))
        .await;
    let text = String::from_utf8(csv.body.clone()).unwrap();
    assert!(text.contains("Bruno") && !text.contains("Ana"), "{text}");
    let inline = s
        .post("/render", json!({ "layout": serde_json::from_str::<Value>(&sales().to_json()).unwrap(), "data": [{ "Region": "Sul", "Name": "Carla" }], "format": "csv" }))
        .await;
    assert!(String::from_utf8(inline.body.clone()).unwrap().contains("Carla"));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_job_emails_its_files_and_records_who_received_them() {
    let s = start(|_| {}).await;
    let run = s.run("daily", email_job(&["a@example.com", "b@example.com"], false)).await;
    assert_eq!(run.status, RunStatus::Success, "{:?}", run.errors);
    assert_eq!(run.rows, Some(1));
    assert_eq!(run.attempts, 1);
    assert_eq!(run.layout_version, Some(1));
    let sent = s.mailer.sent.lock().unwrap().clone();
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].to, ["a@example.com", "b@example.com"]);
    assert!(sent[0].subject.starts_with("Vendas em 20"), "{}", sent[0].subject);
    assert_eq!(sent[0].attachments.len(), 2);
    assert!(sent[0].attachments[0].bytes.starts_with(b"%PDF-"));
    assert!(sent[0].attachments[1].name.ends_with(".xlsx"));
    assert_eq!(run.delivered[0].target, "a@example.com, b@example.com");
    let history = s.get("/jobs/daily/runs").await.json();
    assert_eq!(history[0]["id"], json!(run.id));
    assert_eq!(history[0]["status"], "success");
    assert_eq!(s.get(&format!("/jobs/daily/runs/{}", run.id)).await.status, 200);
}

#[tokio::test(flavor = "multi_thread")]
async fn separate_recipients_get_separate_messages() {
    let s = start(|_| {}).await;
    let run = s.run("daily", email_job(&["a@example.com", "b@example.com"], true)).await;
    assert_eq!(run.status, RunStatus::Success);
    let sent = s.mailer.sent.lock().unwrap().clone();
    assert_eq!(sent.iter().map(|m| m.to.clone()).collect::<Vec<_>>(), [["a@example.com"], ["b@example.com"]]);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_retry_only_sends_to_who_did_not_receive_it() {
    let s = start(|_| {}).await;
    s.mailer.fail_for.lock().unwrap().push("b@example.com".to_owned());
    let run = s.run("daily", email_job(&["a@example.com", "b@example.com"], true)).await;
    assert_eq!(run.status, RunStatus::Partial);
    assert_eq!(run.attempts, 3, "the first try and two retries");
    assert_eq!(run.delivered.len(), 1);
    assert_eq!(run.delivered[0].target, "a@example.com");
    assert!(run.errors[0].contains("b@example.com"), "{:?}", run.errors);
    assert_eq!(s.mailer.sent.lock().unwrap().len(), 1, "a@example.com got exactly one message");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_job_where_nothing_arrives_fails() {
    let s = start(|_| {}).await;
    s.mailer.fail_for.lock().unwrap().push("a@example.com".to_owned());
    let mut job = email_job(&["a@example.com"], false);
    job["retries"] = json!(0);
    let run = s.run("daily", job).await;
    assert_eq!((run.status, run.attempts), (RunStatus::Failed, 1));
}

#[tokio::test(flavor = "multi_thread")]
async fn an_empty_report_can_be_skipped() {
    let s = start(|_| {}).await;
    let mut job = email_job(&["a@example.com"], false);
    job["parameters"] = json!({ "Region": "Marte" });
    job["skipIfEmpty"] = json!(true);
    let run = s.run("daily", job.clone()).await;
    assert_eq!((run.status, run.rows), (RunStatus::SkippedEmpty, Some(0)));
    assert!(s.mailer.sent.lock().unwrap().is_empty());

    job["skipIfEmpty"] = json!(false);
    assert_eq!(s.run("daily", job).await.status, RunStatus::Success);
    assert_eq!(s.mailer.sent.lock().unwrap().len(), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn parameters_can_be_computed_at_each_run() {
    let s = start(|_| {}).await;
    let mut job = email_job(&["a@example.com"], false);
    job["parameters"] = json!({ "Region": { "expression": "Iif(1 = 1, 'Nor' + 'te', 'Sul')" } });
    job["formats"] = json!(["csv"]);
    s.run("daily", job).await;
    let sent = s.mailer.sent.lock().unwrap().clone();
    let text = String::from_utf8(sent[0].attachments[0].bytes.clone()).unwrap();
    assert!(text.contains("Bruno") && !text.contains("Ana"), "{text}");
}

#[tokio::test(flavor = "multi_thread")]
async fn folder_deliveries_write_files() {
    let s = start(|_| {}).await;
    let mut job = email_job(&["a@example.com"], false);
    job["deliveries"] = json!([{ "type": "folder", "path": "vendas/diario" }]);
    let run = s.run("daily", job).await;
    assert_eq!(run.status, RunStatus::Success, "{:?}", run.errors);
    let files = std::fs::read_dir(s.output.join("vendas/diario")).unwrap().count();
    assert_eq!(files, 2);
}

#[tokio::test(flavor = "multi_thread")]
async fn files_are_kept_only_when_asked_and_can_be_downloaded() {
    let s = start(|_| {}).await;
    let mut job = email_job(&["a@example.com"], false);
    assert!(s.run("plain", job.clone()).await.documents.is_empty());
    job["retentionDays"] = json!(7);
    let run = s.run("kept", job).await;
    assert_eq!(run.documents.len(), 2);
    let name = run.documents[0].replace(' ', "%20");
    let file = s.get(&format!("/jobs/kept/runs/{}/documents/{name}", run.id)).await;
    assert_eq!(file.status, 200);
    assert!(file.body.starts_with(b"%PDF-"));
    let escape = s.get(&format!("/jobs/kept/runs/{}/documents/..%2F..%2Fjobs", run.id)).await;
    assert_eq!(escape.status, 404);
}

#[tokio::test(flavor = "multi_thread")]
async fn old_files_are_deleted_past_their_retention() {
    let s = start(|_| {}).await;
    let mut job = email_job(&["a@example.com"], false);
    job["retentionDays"] = json!(1);
    let run = s.run("kept", job).await;
    let future = run.started_at.checked_add(jiff::SignedDuration::from_hours(24 * 2)).unwrap();
    assert_eq!(s.store.purge_documents("kept", future).unwrap(), 1);
    assert_eq!(s.store.document("kept", &run.id, &run.documents[0]).unwrap(), None);
    assert!(s.store.run("kept", &run.id).unwrap().is_some(), "the history stays");
}

#[tokio::test(flavor = "multi_thread")]
async fn the_history_keeps_its_limit() {
    let s = start(|c| c.history_limit = 2).await;
    let job = s.save("daily", email_job(&["a@example.com"], false));
    let mut ids = Vec::new();
    for _ in 0..4 {
        ids.push(s.runner.run_now("daily", &job).await.id);
    }
    let kept: Vec<String> = s.store.runs("daily", 10).unwrap().into_iter().map(|r| r.id).collect();
    assert_eq!(kept, [ids[3].clone(), ids[2].clone()]);
}

fn at(text: &str) -> Timestamp {
    text.parse().unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn the_schedule_runs_each_time_once() {
    let s = start(|_| {}).await;
    s.save("daily", email_job(&["a@example.com"], false));
    // The first look only works out the next run, 08:00 in Recife (11:00 UTC).
    assert!(s.runner.tick(at("2026-11-01T10:00:00Z")).is_empty());
    assert_eq!(s.store.job_state("daily").unwrap().next_run, Some(at("2026-11-01T11:00:00Z")));
    assert!(s.runner.tick(at("2026-11-01T10:59:59Z")).is_empty());

    let started = s.runner.tick(at("2026-11-01T11:00:10Z"));
    assert_eq!(started.len(), 1);
    assert!(s.runner.tick(at("2026-11-01T11:00:20Z")).is_empty(), "not twice in the same minute");
    let run = started.into_iter().next().unwrap().await.unwrap();
    assert_eq!(run.status, RunStatus::Success);
    assert_eq!(run.scheduled_for, at("2026-11-01T11:00:00Z"));
    assert!(!run.manual);
    assert_eq!(s.store.job_state("daily").unwrap().next_run, Some(at("2026-11-02T11:00:00Z")));
    let sent = s.mailer.sent.lock().unwrap().clone();
    assert_eq!(sent[0].subject, "Vendas em 2026-11-01", "the date is the schedule's, in its time zone");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_run_missed_while_down_still_happens_within_the_grace() {
    let s = start(|c| c.misfire_grace_minutes = 60).await;
    s.save("daily", email_job(&["a@example.com"], false));
    s.store.save_job_state("daily", &JobState { next_run: Some(at("2026-11-01T11:00:00Z")) }).unwrap();
    let started = s.runner.tick(at("2026-11-01T11:30:00Z"));
    assert_eq!(started.into_iter().next().unwrap().await.unwrap().status, RunStatus::Success);
    assert_eq!(s.store.job_state("daily").unwrap().next_run, Some(at("2026-11-02T11:00:00Z")));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_run_missed_for_too_long_is_recorded_and_not_sent() {
    let s = start(|c| c.misfire_grace_minutes = 60).await;
    s.save("daily", email_job(&["a@example.com"], false));
    s.store.save_job_state("daily", &JobState { next_run: Some(at("2026-11-01T11:00:00Z")) }).unwrap();
    assert!(s.runner.tick(at("2026-11-03T09:00:00Z")).is_empty());
    let runs = s.store.runs("daily", 10).unwrap();
    assert_eq!(runs[0].status, RunStatus::Missed);
    assert_eq!(runs[0].scheduled_for, at("2026-11-01T11:00:00Z"));
    assert!(s.mailer.sent.lock().unwrap().is_empty());
    assert_eq!(s.store.job_state("daily").unwrap().next_run, Some(at("2026-11-03T11:00:00Z")));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_disabled_job_only_runs_when_asked() {
    let s = start(|_| {}).await;
    let mut job = email_job(&["a@example.com"], false);
    job["enabled"] = json!(false);
    s.save("daily", job);
    s.store.save_job_state("daily", &JobState { next_run: Some(at("2026-11-01T11:00:00Z")) }).unwrap();
    assert!(s.runner.tick(at("2026-11-01T11:00:10Z")).is_empty());
    assert_eq!(s.post("/jobs/daily/run", json!({})).await.status, 202);
    for _ in 0..200 {
        if !s.store.runs("daily", 1).unwrap().is_empty() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
    assert!(s.store.runs("daily", 1).unwrap()[0].manual);
}

#[tokio::test(flavor = "multi_thread")]
async fn jobs_over_the_api_and_a_layout_in_use_stays() {
    let s = start(|_| {}).await;
    s.put("/layouts/sales", sales().to_json()).await;
    let saved = s.put("/jobs/daily", email_job(&["a@example.com"], false).to_string()).await;
    assert_eq!(saved.status, 200, "{:?}", saved.json());
    assert!(saved.json()["nextRun"].is_string());
    assert_eq!(s.get("/jobs").await.json()[0]["id"], "daily");
    assert_eq!(s.get("/jobs/daily").await.json()["job"]["layout"], "sales");
    assert_eq!(s.delete("/layouts/sales").await.status, 409);
    assert_eq!(s.delete("/jobs/daily").await.status, 204);
    assert_eq!(s.get("/jobs/daily").await.status, 404);
    assert_eq!(s.delete("/layouts/sales").await.status, 204);
}

#[tokio::test(flavor = "multi_thread")]
async fn jobs_needing_what_the_server_lacks_are_refused() {
    let s = start(|c| {
        c.smtp = None;
        c.output_dir = None;
    })
    .await;
    s.put("/layouts/sales", sales().to_json()).await;
    assert_eq!(s.put("/jobs/a", email_job(&["a@example.com"], false).to_string()).await.status, 400);
    let mut folder = email_job(&["a@example.com"], false);
    folder["deliveries"] = json!([{ "type": "folder", "path": "x" }]);
    assert_eq!(s.put("/jobs/a", folder.to_string()).await.status, 400);
}

fn url_report(url: &str) -> Report {
    Report::new("Remoto")
        .data_source(DataSource {
            name: "A".into(),
            kind: DataSourceKind::Url { url: url.to_owned(), headers: Default::default() },
        })
        .detail(Band::new(18.0).control(Control::field("[Name]").at(0.0, 0.0, 200.0, 18.0)))
}

async fn data_host(app: axum::Router) -> u16 {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move { axum::serve(listener, app).await });
    port
}

#[tokio::test(flavor = "multi_thread")]
async fn url_sources_read_allowed_hosts_only() {
    let port = data_host(axum::Router::new().route(
        "/rows",
        get(|axum::extract::RawQuery(q): axum::extract::RawQuery, headers: axum::http::HeaderMap| async move {
            Json(json!([{ "Name": format!("q={} key={}", q.unwrap_or_default(), headers.get("x-key").map_or("none", |v| v.to_str().unwrap_or(""))) }]))
        }),
    ))
    .await;
    let allowed = DataHost { host: "127.0.0.1".into(), headers_env: Default::default() };
    let s = start(move |c| c.data_hosts = vec![allowed]).await;
    let layout = serde_json::from_str::<Value>(
        &url_report(&format!("http://127.0.0.1:{port}/rows?r={{?Who}}")).to_json(),
    )
    .unwrap();
    let ok = s
        .post("/render", json!({ "layout": layout, "format": "csv", "parameters": { "Who": "a b&c" } }))
        .await;
    assert_eq!(ok.status, 200);
    assert!(
        String::from_utf8_lossy(&ok.body).contains("r=a%20b%26c"),
        "{}",
        String::from_utf8_lossy(&ok.body)
    );

    for url in ["http://169.254.169.254/latest/meta-data", "http://localhost:1/x", "file:///etc/passwd"] {
        let layout = serde_json::from_str::<Value>(&url_report(url).to_json()).unwrap();
        let refused = s.post("/render", json!({ "layout": layout, "format": "csv" })).await;
        assert_eq!(refused.status, 400, "{url}");
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn redirects_are_not_followed_out_of_an_allowed_host() {
    let port = data_host(axum::Router::new().route(
        "/rows",
        get(|| async { axum::response::Redirect::temporary("http://169.254.169.254/latest/meta-data") }),
    ))
    .await;
    let s = start(|c| {
        c.data_hosts = vec![DataHost { host: "127.0.0.1".into(), headers_env: Default::default() }]
    })
    .await;
    let layout =
        serde_json::from_str::<Value>(&url_report(&format!("http://127.0.0.1:{port}/rows")).to_json())
            .unwrap();
    let refused = s.post("/render", json!({ "layout": layout, "format": "csv" })).await;
    assert_eq!(refused.status, 400);
    assert!(refused.json()["error"].as_str().unwrap().contains("307"), "{:?}", refused.json());
}

#[tokio::test(flavor = "multi_thread")]
async fn sql_sources_say_they_are_not_supported_yet() {
    let s = start(|_| {}).await;
    let report = Report::new("Q").data_source(DataSource {
        name: "A".into(),
        kind: DataSourceKind::Sql { connection: "erp".into(), query: "select 1".into() },
    });
    let layout = serde_json::from_str::<Value>(&report.to_json()).unwrap();
    let answer = s.post("/render", json!({ "layout": layout, "format": "pdf" })).await;
    assert_eq!(answer.status, 400);
    assert!(answer.json()["error"].as_str().unwrap().contains("not supported"));
}

/// A minimal SMTP server that keeps what it receives.
fn smtp_server() -> (u16, Arc<Mutex<Vec<String>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let received = Arc::new(Mutex::new(Vec::new()));
    let store = received.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut out) = stream else { return };
            let mut lines = BufReader::new(out.try_clone().unwrap());
            out.write_all(b"220 test\r\n").unwrap();
            let mut in_data = false;
            let mut message = String::new();
            let mut line = String::new();
            while lines.read_line(&mut line).unwrap_or(0) > 0 {
                if in_data {
                    if line == ".\r\n" {
                        in_data = false;
                        store.lock().unwrap().push(std::mem::take(&mut message));
                        out.write_all(b"250 ok\r\n").unwrap();
                    } else {
                        message.push_str(&line);
                    }
                } else {
                    let upper = line.to_ascii_uppercase();
                    message.push_str(if upper.starts_with("RCPT") { &line } else { "" });
                    let reply: &[u8] = if upper.starts_with("DATA") {
                        in_data = true;
                        b"354 go\r\n"
                    } else if upper.starts_with("QUIT") {
                        b"221 bye\r\n"
                    } else {
                        b"250 ok\r\n"
                    };
                    out.write_all(reply).unwrap();
                }
                line.clear();
            }
        }
    });
    (port, received)
}

#[tokio::test(flavor = "multi_thread")]
async fn a_real_smtp_conversation_delivers_the_attachments() {
    let (port, received) = smtp_server();
    let mailer = SmtpMailer::new(&SmtpConfig {
        host: "127.0.0.1".into(),
        port: Some(port),
        security: SmtpSecurity::None,
        from: "Relatórios <r@example.com>".into(),
        username_env: None,
        password_env: None,
    })
    .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).unwrap();
    store.save_layout("sales", &sales().to_json()).unwrap();
    let mut config = Config::new(dir.path());
    config.retry_delay_seconds = 0;
    let runner = Runner::new(store, Arc::new(config), Some(Arc::new(mailer)));
    let job: Job = serde_json::from_value(email_job(&["Ana <ana@example.com>"], false)).unwrap();
    let run = runner.run_now("daily", &job).await;
    assert_eq!(run.status, RunStatus::Success, "{:?}", run.errors);
    let messages = received.lock().unwrap().clone();
    assert_eq!(messages.len(), 1);
    let message = &messages[0];
    assert!(message.contains("ana@example.com"));
    assert!(message.contains(".pdf") && message.contains(".xlsx"), "{}", &message[..message.len().min(800)]);
    assert!(message.contains("Content-Transfer-Encoding: base64"));
}
