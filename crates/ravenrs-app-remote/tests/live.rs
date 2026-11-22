//! Listening to the live channel: reading the stream, coming back where it left off, stopping when
//! refused, and the whole way from a real server.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::rc::Rc;
use std::sync::mpsc::channel;
use std::thread;
use std::time::{Duration, Instant};

use ravenrs_app::{
    MemoryDatabase, ObjectSpace, Query, Role, SECURITY_ROLE, SECURITY_USER, SECURITY_USER_ROLE, SpaceMaker,
    UserDirectory, Value, hash_password, security_entities,
};
use ravenrs_app_remote::{LiveEvent, LiveFeed, LiveState, RemoteClient, SseParser};
use ravenrs_app_server::{ApiOptions, ApiService, LiveChannels};
use ravenrs_widgets::UiWaker;
use serde_json::json;

fn kinds(events: &[LiveEvent]) -> Vec<(u64, &str)> {
    events.iter().map(|event| (event.id, event.kind.as_str())).collect()
}

#[test]
fn the_parser_joins_data_lines_skips_comments_and_waits_for_the_rest() {
    let mut parser = SseParser::new();
    assert!(parser.feed(b": still here\n\n").is_empty(), "a comment is no event");
    // One event cut in two, with Windows line ends.
    assert!(parser.feed(b"id: 7\r\nevent: line\r\ndata: {\"a\":").is_empty());
    let done = parser.feed(b" 1}\r\n\r\n");
    assert_eq!(done.len(), 1);
    assert_eq!((done[0].id, done[0].kind.as_str(), &done[0].data), (7, "line", &json!({ "a": 1 })));
    // Two data lines are one value, joined by a line end; text that is not JSON stays text.
    let done = parser.feed(b"data: primeira\ndata: segunda\n\n");
    assert_eq!(done[0].data, json!("primeira\nsegunda"));
    assert_eq!(done[0].kind, "message", "no event line means a message");
}

/// Waits until `feed` has `count` events, or `within` passes.
fn wait_for(feed: &LiveFeed, count: usize, within: Duration) -> Vec<LiveEvent> {
    let until = Instant::now() + within;
    let mut got = Vec::new();
    while got.len() < count && Instant::now() < until {
        got.extend(feed.drain());
        thread::sleep(Duration::from_millis(20));
    }
    got
}

#[test]
fn a_lost_stream_comes_back_from_the_last_event() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let (asked, heard) = channel::<Option<String>>();
    thread::spawn(move || {
        for (round, stream) in listener.incoming().take(2).enumerate() {
            let mut stream = stream.unwrap();
            let mut last = None;
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            loop {
                let mut line = String::new();
                if reader.read_line(&mut line).unwrap() == 0 || line == "\r\n" {
                    break;
                }
                if let Some(value) = line.to_ascii_lowercase().strip_prefix("last-event-id:") {
                    last = Some(value.trim().to_owned());
                }
            }
            asked.send(last).unwrap();
            let id = round + 1;
            let body = format!("id: {id}\nevent: line\ndata: {id}\n\n");
            write!(
                stream,
                "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\nconnection: close\r\n\r\n{body}"
            )
            .unwrap();
            // The first stream drops right after its event; the second stays.
            if round == 1 {
                thread::sleep(Duration::from_secs(5));
            }
        }
    });
    let client = RemoteClient::http(format!("http://{address}"));
    let waker = UiWaker::default();
    let feed = client.listen("builds", None, waker.clone());
    let got = wait_for(&feed, 2, Duration::from_secs(5));
    assert_eq!(kinds(&got), [(1, "line"), (2, "line")]);
    assert_eq!(heard.recv().unwrap(), None, "the first time it starts from nothing");
    assert_eq!(heard.recv().unwrap().as_deref(), Some("1"), "coming back, from the last it heard");
    assert!(waker.is_dirty(), "each event asks for a build");
    assert_eq!(feed.last_id(), 2);
}

#[test]
fn a_refusal_stops_it_for_good() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let (asked, heard) = channel::<()>();
    thread::spawn(move || {
        for stream in listener.incoming() {
            let mut stream = stream.unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut line = String::new();
            while reader.read_line(&mut line).unwrap_or(0) > 0 && line != "\r\n" {
                line.clear();
            }
            asked.send(()).unwrap();
            write!(stream, "HTTP/1.1 404 Not Found\r\ncontent-length: 0\r\nconnection: close\r\n\r\n")
                .unwrap();
        }
    });
    let client = RemoteClient::http(format!("http://{address}"));
    let feed = client.listen("ninguem-abriu", None, UiWaker::default());
    let until = Instant::now() + Duration::from_secs(3);
    while feed.state() != LiveState::Refused(404) && Instant::now() < until {
        thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(feed.state(), LiveState::Refused(404));
    heard.recv().unwrap();
    assert!(heard.recv_timeout(Duration::from_millis(1500)).is_err(), "it does not ask again");
    feed.close();
    assert_eq!(feed.state(), LiveState::Closed);
}

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
    user.set("password_hash", hash_password("uma-senha-de-teste").unwrap());
    user.set("must_change_password", false);
    space.save(user).unwrap();
    let mut link = space.new_object(SECURITY_USER_ROLE).unwrap();
    link.set("user_id", key);
    link.set("role_id", role.get("id"));
    space.save(link).unwrap();
    space.commit().unwrap();
    ApiService::new(maker, roles).map_err(|e| e.to_string())
}

#[test]
fn from_a_real_server_what_is_published_reaches_the_feed() {
    let channels = LiveChannels::new();
    channels.open("builds");
    let runtime = tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap();
    let listener = runtime.block_on(tokio::net::TcpListener::bind("127.0.0.1:0")).unwrap();
    let address = listener.local_addr().unwrap();
    runtime.spawn(ravenrs_app_server::serve_with(
        listener,
        ApiOptions::new().live(channels.clone()),
        service,
    ));

    let client = RemoteClient::http(format!("http://{address}"));
    let signed = std::rc::Rc::new(std::cell::Cell::new(false));
    let done = signed.clone();
    client.sign_in("ana", "uma-senha-de-teste", move |answer| done.set(answer.is_ok()));
    assert!(signed.get(), "signed in");
    let feed = client.listen("builds", None, UiWaker::default());
    let until = Instant::now() + Duration::from_secs(3);
    while feed.state() != LiveState::Connected && Instant::now() < until {
        thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(feed.state(), LiveState::Connected);
    channels.publish("builds", "state", json!({ "build": 42, "state": "running" }));
    let got = wait_for(&feed, 1, Duration::from_secs(3));
    assert_eq!(kinds(&got), [(1, "state")]);
    assert_eq!(got[0].data, json!({ "build": 42, "state": "running" }));
    drop(feed);
    runtime.shutdown_background();
}
