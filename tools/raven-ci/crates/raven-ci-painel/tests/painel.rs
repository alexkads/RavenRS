//! The server of the tests as the panel reads it: what each page asks for answers, a build asked
//! for goes to the queue, and a reader may not act on builds.
#![allow(clippy::expect_used, reason = "tests fail loudly")]
#![cfg(not(target_arch = "wasm32"))]

use std::cell::RefCell;
use std::rc::Rc;

use raven_ci_painel::RemoteClient;
use raven_ci_painel::e2e::{PASSWORD, folder, server};
use serde_json::{Value, json};

fn signed(client: &RemoteClient, name: &str) {
    client.sign_in(name, PASSWORD, |answer| {
        answer.expect("signed in");
    });
}

fn get(client: &RemoteClient, path: &str) -> (u16, Value) {
    let got = Rc::new(RefCell::new((0, Value::Null)));
    let keep = got.clone();
    client.get_json(path, move |answer| *keep.borrow_mut() = answer.expect("an answer"));
    got.borrow().clone()
}

fn post(client: &RemoteClient, path: &str, body: Value) -> (u16, Value) {
    let got = Rc::new(RefCell::new((0, Value::Null)));
    let keep = got.clone();
    client.send_json("POST", path, &body, move |answer| *keep.borrow_mut() = answer.expect("an answer"));
    got.borrow().clone()
}

#[test]
fn every_page_reads_what_it_shows() {
    let dir = folder("paginas");
    let (_ci, client) = server(&dir).expect("the server of the tests");
    signed(&client, "ana");
    for path in [
        "/api/me",
        "/api/ci/server",
        "/api/ci/screens",
        "/api/ci/overview",
        "/api/ci/occupancy?since_ms=0",
        "/api/ci/jobs",
        "/api/ci/plugins",
        "/api/ci/credentials",
        "/api/ci/users",
        "/api/ci/audit?limit=5",
        "/api/ci/build?job=apps%2Fsite&number=2",
        "/api/ci/log?job=apps%2Fsite&number=2&after=0",
        "/api/ci/demonstracao/travas",
    ] {
        let (status, body) = get(&client, path);
        assert_eq!(status, 200, "{path}: {body}");
    }
    let (_, overview) = get(&client, "/api/ci/overview");
    assert_eq!(overview["jobs"].as_array().map(Vec::len), Some(4), "{overview}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_build_asked_for_goes_to_the_queue_and_a_reader_may_not_act() {
    let dir = folder("disparo");
    let (_ci, client) = server(&dir).expect("the server of the tests");
    signed(&client, "bia");
    let (status, said) = post(&client, "/api/ci/run", json!({ "job": "imagens-base", "parameters": {} }));
    assert_eq!(status, 201, "{said}");
    assert_eq!(said["number"], 1);
    let (_, overview) = get(&client, "/api/ci/overview");
    assert_eq!(overview["queued"], 1, "{overview}");
    let reader = server(&folder("disparo-leitor")).expect("another").1;
    signed(&reader, "caio");
    let (status, _) = post(&reader, "/api/ci/rebuild", json!({ "job": "apps/site", "number": 1 }));
    assert_eq!(status, 403);
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(folder("disparo-leitor"));
}
