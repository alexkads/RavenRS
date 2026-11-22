//! The live channel over real HTTP: what is published reaches who listens, coming back gets what
//! was missed, and a token that ends ends its stream.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use std::io::{BufRead, BufReader};
use std::net::SocketAddr;
use std::rc::Rc;
use std::sync::mpsc::{Receiver, RecvTimeoutError, channel};
use std::thread;
use std::time::{Duration, Instant};

use ravenrs_app::{
    MemoryDatabase, ObjectSpace, Query, Role, SECURITY_ROLE, SECURITY_USER, SECURITY_USER_ROLE, SpaceMaker,
    UserDirectory, Value, hash_password, security_entities,
};
use ravenrs_app_server::{ApiOptions, ApiService, LiveChannels};
use serde_json::{Value as Json, json};

const PASSWORD: &str = "uma-senha-de-teste";
/// How often an open stream looks at its token in these tests.
const CHECK: Duration = Duration::from_millis(200);

fn service() -> Result<ApiService, String> {
    let database = MemoryDatabase::new(security_entities()).map_err(|e| e.to_string())?;
    let maker: SpaceMaker = {
        let database = database.clone();
        Rc::new(move || Box::new(database.space()) as Box<dyn ObjectSpace>)
    };
    let roles = vec![Role::new("Administradores").administrative()];
    UserDirectory::new(maker.clone(), roles.clone()).map_err(|e| e.to_string())?;
    let mut space = database.space();
    let found = space.query(SECURITY_ROLE, &Query::all()).unwrap();
    let role = found.iter().find(|r| r.get("name") == Value::Text("Administradores".into())).unwrap();
    let mut user = space.new_object(SECURITY_USER).unwrap();
    let key = user.get("id");
    user.set("name", "ana");
    user.set("password_hash", hash_password(PASSWORD).unwrap());
    user.set("must_change_password", false);
    space.save(user).unwrap();
    let mut link = space.new_object(SECURITY_USER_ROLE).unwrap();
    link.set("user_id", key);
    link.set("role_id", role.get("id"));
    space.save(link).unwrap();
    space.commit().unwrap();
    ApiService::new(maker, roles).map_err(|e| e.to_string())
}

struct Server {
    address: SocketAddr,
    channels: LiveChannels,
    runtime: Option<tokio::runtime::Runtime>,
}

impl Drop for Server {
    fn drop(&mut self) {
        if let Some(runtime) = self.runtime.take() {
            runtime.shutdown_background();
        }
    }
}

fn start() -> Server {
    let channels = LiveChannels::checked_every(CHECK);
    channels.open("builds");
    let runtime = tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap();
    let listener = runtime.block_on(tokio::net::TcpListener::bind("127.0.0.1:0")).unwrap();
    let address = listener.local_addr().unwrap();
    let options = ApiOptions::new().live(channels.clone());
    runtime.spawn(ravenrs_app_server::serve_with(listener, options, service));
    Server { address, channels, runtime: Some(runtime) }
}

/// What a stream sent, one SSE line at a time; it ends when the stream does.
struct Listening {
    status: u16,
    lines: Receiver<String>,
}

impl Listening {
    /// The next event, as (id, event, data), within `within`.
    fn event(&self, within: Duration) -> Option<(String, String, Json)> {
        let until = Instant::now() + within;
        let (mut id, mut kind, mut data) = (String::new(), String::new(), String::new());
        loop {
            let left = until.checked_duration_since(Instant::now())?;
            let line = self.lines.recv_timeout(left).ok()?;
            if line.is_empty() {
                if !data.is_empty() || !kind.is_empty() {
                    return Some((id, kind, serde_json::from_str(&data).unwrap_or(Json::Null)));
                }
                continue;
            }
            if let Some(value) = line.strip_prefix("id:") {
                id = value.trim().to_owned();
            } else if let Some(value) = line.strip_prefix("event:") {
                kind = value.trim().to_owned();
            } else if let Some(value) = line.strip_prefix("data:") {
                data.push_str(value.trim());
            }
        }
    }

    /// Whether the stream ended within `within`.
    fn ends_within(&self, within: Duration) -> bool {
        let until = Instant::now() + within;
        loop {
            let Some(left) = until.checked_duration_since(Instant::now()) else { return false };
            match self.lines.recv_timeout(left) {
                Ok(_) => {}
                Err(RecvTimeoutError::Disconnected) => return true,
                Err(RecvTimeoutError::Timeout) => return false,
            }
        }
    }
}

impl Server {
    fn agent() -> ureq::Agent {
        ureq::Agent::config_builder().http_status_as_error(false).build().new_agent()
    }

    fn token(&self) -> String {
        let mut answer = Self::agent()
            .post(&format!("http://{}/api/auth/token", self.address))
            .header("Content-Type", "application/json")
            .send(json!({ "name": "ana", "password": PASSWORD }).to_string())
            .unwrap();
        let body: Json = serde_json::from_str(&answer.body_mut().read_to_string().unwrap()).unwrap();
        body["token"].as_str().unwrap().to_owned()
    }

    fn get(&self, path: &str, token: Option<&str>) -> u16 {
        let bearer = token.map(|t| format!("Bearer {t}")).unwrap_or_default();
        let agent = Self::agent();
        let answer =
            agent.get(&format!("http://{}{path}", self.address)).header("Authorization", &bearer).call();
        answer.unwrap().status().as_u16()
    }

    fn listen(&self, topic: &str, token: Option<&str>, last: Option<u64>) -> Listening {
        let url = format!("http://{}/api/live/{topic}", self.address);
        let bearer = token.map(|t| format!("Bearer {t}")).unwrap_or_default();
        let mut asked =
            Self::agent().get(&url).header("Authorization", &bearer).header("Accept", "text/event-stream");
        if let Some(last) = last {
            asked = asked.header("Last-Event-ID", last.to_string());
        }
        let answer = asked.call().unwrap();
        let status = answer.status().as_u16();
        let (send, lines) = channel();
        thread::spawn(move || {
            let reader = BufReader::new(answer.into_body().into_reader());
            for line in reader.lines() {
                let Ok(line) = line else { return };
                if send.send(line).is_err() {
                    return;
                }
            }
        });
        Listening { status, lines }
    }
}

#[test]
fn what_is_published_reaches_who_listens_at_once() {
    let server = start();
    let token = server.token();
    let listening = server.listen("builds", Some(&token), None);
    assert_eq!(listening.status, 200);
    // The stream is open once the server answered; publish from another thread.
    let channels = server.channels.clone();
    let published = Instant::now();
    thread::spawn(move || channels.publish("builds", "state", json!({ "build": 42, "state": "running" })));
    let (id, kind, data) = listening.event(Duration::from_secs(2)).expect("an event");
    assert!(published.elapsed() < Duration::from_secs(1), "took {:?}", published.elapsed());
    assert_eq!((id.as_str(), kind.as_str()), ("1", "state"));
    assert_eq!(data, json!({ "build": 42, "state": "running" }));
}

#[test]
fn open_streams_do_not_take_the_thread_that_answers() {
    let server = start();
    let token = server.token();
    let streams: Vec<Listening> = (0..10).map(|_| server.listen("builds", Some(&token), None)).collect();
    assert!(streams.iter().all(|stream| stream.status == 200));
    let began = Instant::now();
    assert_eq!(server.get("/api/me", Some(&token)), 200);
    assert!(began.elapsed() < Duration::from_secs(1), "took {:?}", began.elapsed());
    server.channels.publish("builds", "line", json!("todos ouvem"));
    for stream in &streams {
        assert_eq!(stream.event(Duration::from_secs(2)).map(|e| e.2), Some(json!("todos ouvem")));
    }
}

#[test]
fn coming_back_gets_what_was_missed() {
    let server = start();
    let token = server.token();
    for n in 1..=6 {
        server.channels.publish("builds", "line", json!(n));
    }
    let listening = server.listen("builds", Some(&token), Some(3));
    let ids: Vec<String> =
        (0..3).filter_map(|_| listening.event(Duration::from_secs(2))).map(|e| e.0).collect();
    assert_eq!(ids, ["4", "5", "6"]);
    // What comes next follows them, with no repeat.
    server.channels.publish("builds", "line", json!(7));
    assert_eq!(listening.event(Duration::from_secs(2)).map(|e| e.0), Some("7".to_owned()));
}

#[test]
fn coming_back_too_late_is_told_some_are_gone() {
    let server = start();
    let token = server.token();
    for n in 1..=300 {
        server.channels.publish("builds", "line", json!(n));
    }
    let listening = server.listen("builds", Some(&token), Some(1));
    let (_, kind, _) = listening.event(Duration::from_secs(2)).expect("an event");
    assert_eq!(kind, "gap", "the first thing it hears is that some are gone");
    assert_eq!(
        listening.event(Duration::from_secs(2)).map(|e| e.0),
        Some("45".to_owned()),
        "then the oldest kept"
    );
}

#[test]
fn a_topic_nobody_opened_and_no_token_are_refused() {
    let server = start();
    let token = server.token();
    assert_eq!(server.listen("ninguem-abriu", Some(&token), None).status, 404);
    assert_eq!(server.listen("builds", None, None).status, 401);
    assert_eq!(server.listen("builds", Some("token-que-nao-existe"), None).status, 401);
}

#[test]
fn signing_out_ends_the_stream() {
    let server = start();
    let token = server.token();
    let listening = server.listen("builds", Some(&token), None);
    assert_eq!(listening.status, 200);
    let agent = Server::agent();
    let out = agent
        .post(&format!("http://{}/api/auth/revoke", server.address))
        .header("Authorization", &format!("Bearer {token}"))
        .send("")
        .unwrap();
    assert_eq!(out.status().as_u16(), 200);
    assert!(listening.ends_within(CHECK * 5), "the stream ends within a check or so");
}
