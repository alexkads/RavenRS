//! More than one thread answering: requests at the same time, and who is signed in seen the same
//! way by every thread.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use std::collections::BTreeSet;
use std::net::SocketAddr;
use std::rc::Rc;
use std::thread;
use std::time::{Duration, Instant};

use ravenrs_app::{
    MemoryDatabase, ObjectSpace, Query, Role, SECURITY_ROLE, SECURITY_USER, SECURITY_USER_ROLE, SpaceMaker,
    UserDirectory, Value, hash_password, security_entities,
};
use ravenrs_app_server::{ApiOptions, ApiService};
use serde_json::{Value as Json, json};

const PASSWORD: &str = "uma-senha-de-teste";
const NEW_PASSWORD: &str = "outra-senha-de-teste";
/// How long the slow endpoint takes.
const SLOW: Duration = Duration::from_millis(400);

/// A service over records of its own, with the same people in it: each thread makes one, and the
/// people are the same everywhere, which is all signing in needs. Records kept in memory cannot be
/// shared by threads; a real deployment opens the same database in each.
fn service(sign_ins: u32) -> Result<ApiService, String> {
    let database = MemoryDatabase::new(security_entities()).map_err(|e| e.to_string())?;
    let maker: SpaceMaker = {
        let database = database.clone();
        Rc::new(move || Box::new(database.space()) as Box<dyn ObjectSpace>)
    };
    let roles = vec![Role::new("Administradores").administrative()];
    UserDirectory::new(maker.clone(), roles.clone()).map_err(|e| e.to_string())?;
    let mut space = database.space();
    // The same key in every thread's records, as one shared database would have it.
    for (name, id) in
        [("ana", "00000000-0000-4000-8000-00000000000a"), ("bia", "00000000-0000-4000-8000-00000000000b")]
    {
        let found = space.query(SECURITY_ROLE, &Query::all()).unwrap();
        let role = found.iter().find(|r| r.get("name") == Value::Text("Administradores".into())).unwrap();
        let mut user = space.new_object(SECURITY_USER).unwrap();
        user.set("id", Value::Text(id.to_owned()));
        let key = user.get("id");
        user.set("name", name);
        user.set("password_hash", hash_password(PASSWORD).unwrap());
        user.set("must_change_password", false);
        space.save(user).unwrap();
        let mut link = space.new_object(SECURITY_USER_ROLE).unwrap();
        link.set("user_id", key);
        link.set("role_id", role.get("id"));
        space.save(link).unwrap();
    }
    space.commit().unwrap();
    Ok(ApiService::new(maker, roles).map_err(|e| e.to_string())?.sign_ins_a_minute(sign_ins).endpoint(
        "slow",
        |_, _| {
            thread::sleep(SLOW);
            Ok(json!({ "thread": thread::current().name().unwrap_or("").to_owned() }))
        },
    ))
}

struct Server {
    address: SocketAddr,
    runtime: tokio::runtime::Runtime,
}

impl Drop for Server {
    fn drop(&mut self) {
        let runtime = std::mem::replace(
            &mut self.runtime,
            tokio::runtime::Builder::new_current_thread().build().unwrap(),
        );
        runtime.shutdown_background();
    }
}

fn start(workers: usize, sign_ins: u32) -> Server {
    let runtime = tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap();
    let listener = runtime.block_on(tokio::net::TcpListener::bind("127.0.0.1:0")).unwrap();
    let address = listener.local_addr().unwrap();
    let options = ApiOptions::new().workers(workers);
    runtime.spawn(ravenrs_app_server::serve_with(listener, options, move || service(sign_ins)));
    Server { address, runtime }
}

impl Server {
    fn send(&self, method: &str, path: &str, token: Option<&str>, body: Option<Json>) -> (u16, Json) {
        let agent = ureq::Agent::config_builder().http_status_as_error(false).build().new_agent();
        let url = format!("http://{}{path}", self.address);
        let bearer = token.map(|token| format!("Bearer {token}")).unwrap_or_default();
        let mut answer = match method {
            "GET" => agent.get(&url).header("Authorization", &bearer).call(),
            _ => agent
                .post(&url)
                .header("Authorization", &bearer)
                .header("Content-Type", "application/json")
                .send(body.map(|body| body.to_string()).unwrap_or_default()),
        }
        .unwrap();
        let status = answer.status().as_u16();
        let text = answer.body_mut().read_to_string().unwrap_or_default();
        (status, serde_json::from_str(&text).unwrap_or(Json::Null))
    }

    fn sign_in(&self, name: &str, password: &str) -> (u16, Json) {
        self.send("POST", "/api/auth/token", None, Some(json!({ "name": name, "password": password })))
    }

    fn token(&self, name: &str) -> String {
        let (status, body) = self.sign_in(name, PASSWORD);
        assert_eq!(status, 200, "{body}");
        body["token"].as_str().unwrap().to_owned()
    }

    /// Runs the slow endpoint `times` times at once, and returns how long it all took and which
    /// threads answered.
    fn slow_at_once(&self, token: &str, times: usize) -> (Duration, BTreeSet<String>) {
        let began = Instant::now();
        let answers: Vec<(u16, Json)> = thread::scope(|scope| {
            let asked: Vec<_> = (0..times)
                .map(|_| scope.spawn(|| self.send("POST", "/api/run/slow", Some(token), None)))
                .collect();
            asked.into_iter().map(|one| one.join().unwrap()).collect()
        });
        let took = began.elapsed();
        let mut threads = BTreeSet::new();
        for (status, body) in answers {
            assert_eq!(status, 200, "{body}");
            threads.insert(body["thread"].as_str().unwrap().to_owned());
        }
        (took, threads)
    }
}

#[test]
fn two_threads_answer_two_slow_requests_together() {
    let server = start(2, 20);
    let token = server.token("ana");
    let (took, threads) = server.slow_at_once(&token, 2);
    assert!(took < SLOW + Duration::from_millis(300), "took {took:?}");
    assert_eq!(threads.len(), 2, "both threads answered: {threads:?}");
}

#[test]
fn one_thread_answers_them_one_after_the_other() {
    let server = start(1, 20);
    let token = server.token("ana");
    let (took, threads) = server.slow_at_once(&token, 2);
    assert!(took >= SLOW * 2, "took {took:?}");
    assert_eq!(threads.len(), 1);
}

#[test]
fn zero_workers_is_one() {
    let server = start(0, 20);
    let token = server.token("ana");
    assert_eq!(server.send("GET", "/api/me", Some(&token), None).0, 200);
}

#[test]
fn a_token_from_one_thread_is_good_on_every_other() {
    let server = start(3, 20);
    let token = server.token("ana");
    for _ in 0..20 {
        assert_eq!(server.send("GET", "/api/me", Some(&token), None).0, 200);
    }
    // Three at once land on three threads, all with the same token.
    let (_, threads) = server.slow_at_once(&token, 3);
    assert_eq!(threads.len(), 3, "{threads:?}");
}

#[test]
fn signing_out_counts_on_every_thread() {
    let server = start(3, 20);
    let token = server.token("ana");
    // Every thread has seen the session, and keeps what it built of it.
    let (_, threads) = server.slow_at_once(&token, 3);
    assert_eq!(threads.len(), 3);
    assert_eq!(server.send("POST", "/api/auth/revoke", Some(&token), None).0, 200);
    for _ in 0..20 {
        assert_eq!(server.send("GET", "/api/me", Some(&token), None).0, 401);
    }
}

#[test]
fn a_new_password_ends_the_other_sessions_on_every_thread() {
    let server = start(3, 20);
    let other = server.token("bia");
    let changing = server.token("bia");
    let untouched = server.token("ana");
    let (_, threads) = server.slow_at_once(&other, 3);
    assert_eq!(threads.len(), 3);
    let (status, body) = server.send(
        "POST",
        "/api/auth/password",
        Some(&changing),
        Some(json!({ "current": PASSWORD, "new": NEW_PASSWORD })),
    );
    assert_eq!(status, 200, "{body}");
    for _ in 0..10 {
        assert_eq!(server.send("GET", "/api/me", Some(&other), None).0, 401, "the other session ended");
        assert_eq!(server.send("GET", "/api/me", Some(&changing), None).0, 200, "the one that changed stays");
        assert_eq!(server.send("GET", "/api/me", Some(&untouched), None).0, 200, "someone else's stays");
    }
}

#[test]
fn tries_to_sign_in_add_up_across_threads() {
    let server = start(3, 4);
    for _ in 0..4 {
        assert_eq!(server.sign_in("ana", "errada").0, 401);
    }
    assert_eq!(server.sign_in("ana", "errada").0, 429, "the fifth try, on whichever thread, is refused");
}

#[test]
fn an_open_path_comes_with_no_token_its_headers_and_a_raw_answer_keeps_its_type() {
    let runtime = tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap();
    let listener = runtime.block_on(tokio::net::TcpListener::bind("127.0.0.1:0")).unwrap();
    let address = listener.local_addr().unwrap();
    let options = ApiOptions::new().open("/hooks/").pass_header("x-hub-");
    runtime.spawn(ravenrs_app_server::serve_with(listener, options, || {
        Ok(service(20)?.fallback(|request, _| {
            let path: Vec<&str> = request.path.iter().map(String::as_str).collect();
            match path.as_slice() {
                ["hooks", "github"] => Some(ravenrs_app_server::ApiResponse::raw(
                    200,
                    "text/plain; charset=utf-8",
                    request.headers.get("x-hub-signature-256").cloned().unwrap_or_default().into_bytes(),
                )),
                ["fechado"] => Some(ravenrs_app_server::ApiResponse::json(200, json!({}))),
                _ => None,
            }
        }))
    }));
    let server = Server { address, runtime };
    let agent = ureq::Agent::config_builder().http_status_as_error(false).build().new_agent();
    let mut hook = agent
        .post(&format!("http://{address}/hooks/github"))
        .header("X-Hub-Signature-256", "sha256=abc")
        .header("X-Outro", "não passa")
        .send("{}")
        .unwrap();
    assert_eq!(hook.status().as_u16(), 200);
    assert_eq!(hook.headers().get("content-type").unwrap(), "text/plain; charset=utf-8");
    assert_eq!(hook.body_mut().read_to_string().unwrap(), "sha256=abc");
    let closed = agent.get(&format!("http://{address}/fechado")).call().unwrap();
    assert_eq!(closed.status().as_u16(), 401, "a path that is not open still needs a token");
    drop(server);
}
