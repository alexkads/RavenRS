//! Listening to a topic of the server's live channel (`GET /api/live/{topic}`), on the desktop and in
//! the browser.
//!
//! [`RemoteClient::listen`] opens it and keeps it open: lost, it comes back after a wait that grows
//! from 1 s to 30 s, from the last event it got, so nothing is heard twice. Refused (401, 403, 404),
//! it stops. Events wait in the [`LiveFeed`] until a build takes them with [`LiveFeed::drain`]; the
//! feed asks for that build itself. In the browser the stream is read with `fetch`, since an
//! `EventSource` cannot send the `Authorization` header.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

#[cfg(feature = "ui")]
use ravenrs_widgets::UiWaker;
use serde_json::Value as Json;

use crate::RemoteClient;

/// The first wait before coming back, doubled each time up to [`LONGEST_WAIT`].
const FIRST_WAIT: Duration = Duration::from_secs(1);
/// The longest wait before coming back.
const LONGEST_WAIT: Duration = Duration::from_secs(30);

/// One event of a topic.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct LiveEvent {
    /// Its place in the topic; 0 for one the server sends with no id, as `gap`.
    pub id: u64,
    /// What happened, as `line` or `state`. `gap` says events were missed: load again.
    pub kind: String,
    /// What it carries; text that is not JSON comes as a JSON string.
    pub data: Json,
}

/// How a feed is doing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum LiveState {
    /// Opening, or coming back after losing the stream.
    Connecting,
    /// Open, hearing what comes.
    Connected,
    /// The server said no, with this status (401, 403 or 404); it does not try again.
    Refused(u16),
    /// Closed by [`LiveFeed::close`].
    Closed,
}

/// Reads Server-Sent Events from bytes as they come, in pieces of any size.
#[derive(Debug, Default)]
pub struct SseParser {
    pending: Vec<u8>,
    id: Option<u64>,
    kind: String,
    data: Vec<String>,
}

impl SseParser {
    /// Nothing read yet.
    pub fn new() -> Self {
        Self::default()
    }

    /// Reads `bytes`, and returns the events they completed. A line cut in two waits for the rest.
    pub fn feed(&mut self, bytes: &[u8]) -> Vec<LiveEvent> {
        self.pending.extend_from_slice(bytes);
        let mut done = Vec::new();
        while let Some(end) = self.pending.iter().position(|byte| *byte == b'\n') {
            let line: Vec<u8> = self.pending.drain(..=end).collect();
            let line = String::from_utf8_lossy(&line);
            let line = line.trim_end_matches(['\n', '\r']);
            if let Some(event) = self.line(line) {
                done.push(event);
            }
        }
        done
    }

    fn line(&mut self, line: &str) -> Option<LiveEvent> {
        if line.is_empty() {
            let (id, kind, data) =
                (self.id.take(), std::mem::take(&mut self.kind), std::mem::take(&mut self.data));
            if data.is_empty() && kind.is_empty() {
                return None;
            }
            let text = data.join("\n");
            let data = serde_json::from_str(&text).unwrap_or(Json::String(text));
            let kind = if kind.is_empty() { "message".to_owned() } else { kind };
            return Some(LiveEvent { id: id.unwrap_or(0), kind, data });
        }
        // A line that starts with a colon is a comment, sent to keep the stream open.
        if line.starts_with(':') {
            return None;
        }
        let (field, value) = line.split_once(':').unwrap_or((line, ""));
        let value = value.strip_prefix(' ').unwrap_or(value);
        match field {
            "id" => self.id = value.parse().ok(),
            "event" => self.kind = value.to_owned(),
            "data" => self.data.push(value.to_owned()),
            _ => {}
        }
        None
    }
}

struct Inner {
    events: Mutex<VecDeque<LiveEvent>>,
    state: Mutex<LiveState>,
    stop: AtomicBool,
    last: AtomicU64,
    /// What is told that events came, or that the stream was refused.
    wake: Box<dyn Fn() + Send + Sync>,
}

impl std::fmt::Debug for Inner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Inner")
            .field("state", &*lock(&self.state))
            .field("last", &self.last.load(Ordering::Acquire))
            .finish_non_exhaustive()
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Inner {
    fn set(&self, state: LiveState) {
        *lock(&self.state) = state;
    }

    fn stopped(&self) -> bool {
        self.stop.load(Ordering::Acquire)
    }

    /// Keeps what came and asks for a build. True when something came.
    fn heard(&self, events: Vec<LiveEvent>) -> bool {
        if events.is_empty() {
            return false;
        }
        let mut kept = lock(&self.events);
        for event in events {
            if event.id > 0 {
                self.last.fetch_max(event.id, Ordering::AcqRel);
            }
            kept.push_back(event);
        }
        drop(kept);
        (self.wake)();
        true
    }
}

/// A topic being listened to. Dropping it closes it.
#[derive(Debug)]
pub struct LiveFeed {
    inner: Arc<Inner>,
    /// What tells a browser build that events came, since a waker cannot wake it there.
    #[cfg(target_arch = "wasm32")]
    heard: ravenrs_reactive::State<u64>,
}

impl LiveFeed {
    /// The events that came since the last call, oldest first. Call it while building: in the
    /// browser that is what has the build run again when more come.
    pub fn drain(&self) -> Vec<LiveEvent> {
        #[cfg(target_arch = "wasm32")]
        let _ = self.heard.get();
        lock(&self.inner.events).drain(..).collect()
    }

    /// How it is doing.
    pub fn state(&self) -> LiveState {
        *lock(&self.inner.state)
    }

    /// The id of the last event heard, 0 before any. Coming back starts after it.
    pub fn last_id(&self) -> u64 {
        self.inner.last.load(Ordering::Acquire)
    }

    /// Stops listening. The stream lets go at its next line, which the server sends at least once
    /// a check (15 s unless it says otherwise).
    pub fn close(&self) {
        self.inner.stop.store(true, Ordering::Release);
        self.inner.set(LiveState::Closed);
    }
}

impl Drop for LiveFeed {
    fn drop(&mut self) {
        self.close();
    }
}

impl RemoteClient {
    /// Listens to `topic` of the server's live channel, from after the event `after`, as whoever
    /// signed in. `waker` asks for a build when events come; take them with [`LiveFeed::drain`].
    #[cfg(feature = "ui")]
    pub fn listen(&self, topic: &str, after: Option<u64>, waker: UiWaker) -> LiveFeed {
        self.listen_with(topic, after, move || waker.wake())
    }

    /// Listens to `topic` as [`listen`](Self::listen) does, telling `wake` (from the thread that
    /// reads the stream) when events come or the stream is refused: what a program with no window
    /// uses, such as a build agent that waits on a channel.
    pub fn listen_with(
        &self,
        topic: &str,
        after: Option<u64>,
        wake: impl Fn() + Send + Sync + 'static,
    ) -> LiveFeed {
        let inner = Arc::new(Inner {
            events: Mutex::new(VecDeque::new()),
            state: Mutex::new(LiveState::Connecting),
            stop: AtomicBool::new(false),
            last: AtomicU64::new(after.unwrap_or(0)),
            wake: Box::new(wake),
        });
        let url = format!("{}/api/live/{}", self.base(), topic);
        let token = self.token();
        #[cfg(not(target_arch = "wasm32"))]
        {
            let running = inner.clone();
            let started = std::thread::Builder::new()
                .name(format!("ravenrs-live-{topic}"))
                .spawn(move || desktop::run(&running, &url, token.as_deref()));
            if started.is_err() {
                inner.set(LiveState::Closed);
            }
            LiveFeed { inner }
        }
        #[cfg(target_arch = "wasm32")]
        {
            let heard = ravenrs_reactive::State::new(0_u64);
            web::start(inner.clone(), url, token, heard.clone());
            LiveFeed { inner, heard }
        }
    }
}

/// The next wait before coming back.
fn longer(wait: Duration) -> Duration {
    (wait * 2).min(LONGEST_WAIT)
}

#[cfg(not(target_arch = "wasm32"))]
mod desktop {
    use std::io::Read;
    use std::time::{Duration, Instant};

    use super::{FIRST_WAIT, Inner, LiveState, SseParser, longer};

    /// Waits `wait`, a little at a time, so closing does not wait for all of it.
    fn rest(inner: &Inner, wait: Duration) {
        let until = Instant::now() + wait;
        while !inner.stopped() && Instant::now() < until {
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    pub(super) fn run(inner: &Inner, url: &str, token: Option<&str>) {
        let agent = ureq::Agent::config_builder().http_status_as_error(false).build().new_agent();
        let mut wait = FIRST_WAIT;
        while !inner.stopped() {
            inner.set(LiveState::Connecting);
            let mut asked = agent.get(url).header("Accept", "text/event-stream");
            if let Some(token) = token {
                asked = asked.header("Authorization", &format!("Bearer {token}"));
            }
            let last = inner.last.load(std::sync::atomic::Ordering::Acquire);
            if last > 0 {
                asked = asked.header("Last-Event-ID", &last.to_string());
            }
            match asked.call() {
                Ok(answer) if answer.status() == 200 => {
                    inner.set(LiveState::Connected);
                    wait = FIRST_WAIT;
                    let mut reader = answer.into_body().into_reader();
                    let mut parser = SseParser::new();
                    let mut buffer = [0_u8; 4096];
                    loop {
                        if inner.stopped() {
                            return;
                        }
                        match reader.read(&mut buffer) {
                            Ok(0) | Err(_) => break,
                            Ok(read) => {
                                inner.heard(parser.feed(&buffer[..read]));
                            }
                        }
                    }
                }
                Ok(answer) if matches!(answer.status().as_u16(), 401 | 403 | 404) => {
                    inner.set(LiveState::Refused(answer.status().as_u16()));
                    (inner.wake)();
                    return;
                }
                Ok(_) | Err(_) => {}
            }
            if inner.stopped() {
                return;
            }
            inner.set(LiveState::Connecting);
            rest(inner, wait);
            wait = longer(wait);
        }
    }
}

#[cfg(target_arch = "wasm32")]
mod web {
    use std::sync::Arc;
    use std::time::Duration;

    use wasm_bindgen::{JsCast, JsValue};
    use wasm_bindgen_futures::JsFuture;

    use super::{FIRST_WAIT, Inner, LiveState, SseParser, longer};

    async fn rest(wait: Duration) {
        let promise = js_sys::Promise::new(&mut |resolve, _| {
            if let Some(window) = web_sys::window() {
                let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(
                    &resolve,
                    i32::try_from(wait.as_millis()).unwrap_or(i32::MAX),
                );
            }
        });
        let _ = JsFuture::from(promise).await;
    }

    /// One connection: `Ok(true)` when it was open and then ended, `Ok(false)` when refused.
    async fn once(
        inner: &Inner,
        url: &str,
        token: Option<&str>,
        heard: &ravenrs_reactive::State<u64>,
    ) -> Result<bool, JsValue> {
        let init = web_sys::RequestInit::new();
        init.set_method("GET");
        let asked = web_sys::Request::new_with_str_and_init(url, &init)?;
        asked.headers().set("Accept", "text/event-stream")?;
        if let Some(token) = token {
            asked.headers().set("Authorization", &format!("Bearer {token}"))?;
        }
        let last = inner.last.load(std::sync::atomic::Ordering::Acquire);
        if last > 0 {
            asked.headers().set("Last-Event-ID", &last.to_string())?;
        }
        let window = web_sys::window().ok_or_else(|| JsValue::from_str("no window"))?;
        let response: web_sys::Response =
            JsFuture::from(window.fetch_with_request(&asked)).await?.dyn_into()?;
        let status = response.status();
        if matches!(status, 401 | 403 | 404) {
            inner.set(LiveState::Refused(status));
            heard.update(|n| *n += 1);
            return Ok(false);
        }
        if status != 200 {
            return Err(JsValue::from_str("not open"));
        }
        let body = response.body().ok_or_else(|| JsValue::from_str("no body"))?;
        let reader: web_sys::ReadableStreamDefaultReader = body.get_reader().dyn_into()?;
        inner.set(LiveState::Connected);
        let mut parser = SseParser::new();
        while !inner.stopped() {
            let chunk = JsFuture::from(reader.read()).await?;
            let done = js_sys::Reflect::get(&chunk, &JsValue::from_str("done"))?.as_bool().unwrap_or(true);
            if done {
                break;
            }
            let value = js_sys::Reflect::get(&chunk, &JsValue::from_str("value"))?;
            let bytes = js_sys::Uint8Array::new(&value).to_vec();
            if inner.heard(parser.feed(&bytes)) {
                heard.update(|n| *n += 1);
            }
        }
        let _ = reader.cancel();
        Ok(true)
    }

    pub(super) fn start(
        inner: Arc<Inner>,
        url: String,
        token: Option<String>,
        heard: ravenrs_reactive::State<u64>,
    ) {
        wasm_bindgen_futures::spawn_local(async move {
            let mut wait = FIRST_WAIT;
            while !inner.stopped() {
                inner.set(LiveState::Connecting);
                match once(&inner, &url, token.as_deref(), &heard).await {
                    Ok(false) => return,
                    Ok(true) => wait = FIRST_WAIT,
                    Err(_) => {}
                }
                if inner.stopped() {
                    return;
                }
                rest(wait).await;
                wait = longer(wait);
            }
        });
    }
}
