//! The plugin `webhook-out` (#912) on builds run for real in the core, posting to an HTTP server
//! of the test on a free port: the body, its signature, the tries again and the refusals.
#![allow(clippy::expect_used, clippy::panic, reason = "tests fail loudly")]

use std::collections::{BTreeMap, VecDeque};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use raven_ci_core::{Ci, CiOptions, JobSpec, MemorySecrets};
use raven_ci_plugin_api::{BuildRequest, BuildStatus};
use raven_ci_plugin_format_toml::TomlFormatPlugin;
use raven_ci_plugin_shell::ShellPlugin;
use raven_ci_plugin_webhook_out::WebhookOutPlugin;
use serde_json::{Value, json};

const SECRET: &str = "segredo-do-chat";

/// What the server of the test got: the headers in lower case and the body.
type Got = Arc<Mutex<Vec<(BTreeMap<String, String>, Vec<u8>)>>>;

/// An HTTP server on a free port answering each request with the next status of `answers`
/// (200 once they are over); where it is, and what it got.
fn server(answers: &[u16]) -> (String, Got) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("a port");
    let address = format!("http://{}/hook", listener.local_addr().expect("address"));
    let got: Got = Arc::default();
    let answers = Arc::new(Mutex::new(answers.iter().copied().collect::<VecDeque<u16>>()));
    let kept = got.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let mut reader = BufReader::new(stream.try_clone().expect("clone"));
            let mut headers = BTreeMap::new();
            let mut line = String::new();
            if reader.read_line(&mut line).is_err() {
                continue;
            }
            loop {
                line.clear();
                if reader.read_line(&mut line).is_err() || line.trim().is_empty() {
                    break;
                }
                if let Some((name, value)) = line.trim().split_once(':') {
                    headers.insert(name.trim().to_ascii_lowercase(), value.trim().to_owned());
                }
            }
            let length: usize = headers.get("content-length").and_then(|l| l.parse().ok()).unwrap_or(0);
            let mut body = vec![0; length];
            let _ = reader.read_exact(&mut body);
            kept.lock().unwrap_or_else(PoisonError::into_inner).push((headers, body));
            let status = answers.lock().unwrap_or_else(PoisonError::into_inner).pop_front().unwrap_or(200);
            let _ = write!(stream, "HTTP/1.1 {status} X\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
        }
    });
    (address, got)
}

fn ci() -> Ci {
    let secrets = MemorySecrets(BTreeMap::from([("chat".to_owned(), SECRET.to_owned())]));
    let ci = Ci::new(CiOptions { secrets: Arc::new(secrets), ..CiOptions::default() }).expect("a core");
    ci.add_plugin(Arc::new(TomlFormatPlugin));
    ci.add_plugin(Arc::new(ShellPlugin::new()));
    ci.add_plugin(Arc::new(WebhookOutPlugin::new()));
    ci.apply_plugins().expect("on");
    // Short waits, so trying again does not take the test 21 s.
    ci.configure_plugin("webhook-out", json!({ "retry_wait_ms": 20 })).expect("configured");
    ci
}

fn build(ci: &Ci, job: &str, notify: Value) -> (BuildStatus, Vec<String>, Duration) {
    let command = if cfg!(windows) { "exit /b 2" } else { "exit 2" };
    let spec = JobSpec::new(job, "toml", format!("[[stages]]\nname = \"T\"\nsteps = ['{command}']\n"))
        .every_request()
        .settings(json!({ "notify": [notify] }));
    ci.save_job(spec).expect("saved");
    let began = Instant::now();
    let number = ci.request_build(BuildRequest::new(job, "teste")).expect("asked").number;
    ci.work_until_idle("e1").expect("runs");
    let took = began.elapsed();
    let log = ci.build_log(job, number, 0).expect("log").into_iter().map(|l| l.text).collect();
    (ci.build(job, number).expect("build").status, log, took)
}

/// What `got` holds once it holds `count` requests, or after five seconds.
fn wait_for(got: &Got, count: usize) -> Vec<(BTreeMap<String, String>, Vec<u8>)> {
    let until = Instant::now() + Duration::from_secs(5);
    loop {
        let now = got.lock().unwrap_or_else(PoisonError::into_inner).clone();
        if now.len() >= count || Instant::now() > until {
            return now;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn the_end_of_a_build_is_posted_as_json_with_a_signature_that_checks() {
    let (url, got) = server(&[]);
    let ci = ci();
    let (status, log, _) = build(&ci, "apps/site", json!({ "id": "webhook", "url": url, "secret": "chat" }));
    assert_eq!(status, BuildStatus::Failed, "{log:#?}");
    let got = wait_for(&got, 1);
    assert_eq!(got.len(), 1);
    let (headers, body) = &got[0];
    assert_eq!(headers.get("x-raven-ci-event").map(String::as_str), Some("build.ended"));
    let signature = headers.get("x-raven-ci-signature").expect("signed");
    assert!(ravenrs_app_server::verify(SECRET.as_bytes(), body, signature), "the signature checks");
    let sent: Value = serde_json::from_slice(body).expect("json");
    assert_eq!(sent["says"], "falhou");
    assert_eq!(sent["build"]["job"], "apps/site");
    assert_eq!(sent["build"]["status"], "failed");
    assert!(sent["build"]["reason"].as_str().unwrap_or_default().contains("código 2"), "{sent}");
    assert!(sent["build"]["duration_ms"].is_u64(), "{sent}");
}

#[test]
fn an_answer_500_is_tried_again_and_the_build_does_not_wait_for_it() {
    let (url, got) = server(&[500, 500, 200]);
    let ci = ci();
    let (_, _, took) = build(&ci, "site", json!({ "id": "webhook", "url": url }));
    let got = wait_for(&got, 3);
    assert_eq!(got.len(), 3, "two answers 500, then 200");
    assert!(!got[0].0.contains_key("x-raven-ci-signature"), "no secret, no signature");
    assert!(took < Duration::from_secs(5), "the end of the build did not wait: {took:?}");
}

#[test]
fn an_answer_404_is_not_tried_again() {
    let (url, got) = server(&[404]);
    let ci = ci();
    build(&ci, "site", json!({ "id": "webhook", "url": url }));
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(wait_for(&got, 1).len(), 1);
}

#[test]
fn a_job_with_no_url_one_not_http_or_a_secret_not_in_the_vault_is_said_and_nothing_goes() {
    let (url, got) = server(&[]);
    let ci = ci();
    for (notify, said) in [
        (json!({ "id": "webhook" }), "url"),
        (json!({ "id": "webhook", "url": "ftp://x" }), "não é um endereço http"),
        (json!({ "id": "webhook", "url": url, "secret": "outro" }), "outro"),
    ] {
        let (status, log, _) = build(&ci, "site", notify);
        assert_eq!(status, BuildStatus::Failed);
        assert!(log.iter().any(|l| l.contains("aviso webhook") && l.contains(said)), "{said}: {log:#?}");
    }
    std::thread::sleep(Duration::from_millis(200));
    assert!(got.lock().unwrap_or_else(PoisonError::into_inner).is_empty());
}
