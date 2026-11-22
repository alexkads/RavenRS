//! The link to the server: JSON and bytes over HTTP, with the agent's key, and what to do when the
//! server does not answer.
//!
//! A request either gets an answer the server meant ([`Answer::Ok`], or [`Answer::Refused`] with
//! its status and words) or nothing ([`Answer::Down`]): no connection, or a 5xx. What has to reach
//! the server (a step, a line of log, the end of a build) is sent again after a wait that grows
//! from half a second to ten ([`Link::deliver`]) until it gets an answer the server meant.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, PoisonError, RwLock};
use std::time::Duration;

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

/// The longest a request waits for its answer.
const ANSWER_WITHIN: Duration = Duration::from_secs(60);
/// The first wait before sending again, doubled up to [`LONGEST_WAIT`].
const FIRST_WAIT: Duration = Duration::from_millis(500);
const LONGEST_WAIT: Duration = Duration::from_secs(10);

/// What came back from the server.
#[derive(Debug)]
pub enum Answer {
    /// It answered 2xx, with this body.
    Ok(Value),
    /// It answered no, with this status and these words: sending again would get the same.
    Refused(u16, String),
    /// Nothing came back, or a 5xx: it may work later.
    Down(String),
}

/// The agent's link to the server.
pub struct Link {
    base: String,
    http: ureq::Agent,
    key: RwLock<Option<String>>,
    /// The life of this process, sent with what depends on it.
    pub session: String,
    /// Set when the agent stops: the waits of [`deliver`](Self::deliver) end.
    stopping: Arc<AtomicBool>,
}

impl std::fmt::Debug for Link {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Link")
            .field("base", &self.base)
            .field("session", &self.session)
            .finish_non_exhaustive()
    }
}

impl Link {
    /// A link to the server at `base` (`http://ci.local:8080`), for a process whose life is
    /// `session`.
    pub fn new(base: &str, session: String, stopping: Arc<AtomicBool>) -> Self {
        let http = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .timeout_global(Some(ANSWER_WITHIN))
            .build()
            .new_agent();
        Self { base: base.trim_end_matches('/').to_owned(), http, key: RwLock::default(), session, stopping }
    }

    /// The server's address.
    pub fn base(&self) -> &str {
        &self.base
    }

    /// Presents `key` from now on.
    pub fn use_key(&self, key: &str) {
        *self.key.write().unwrap_or_else(PoisonError::into_inner) = Some(key.to_owned());
    }

    /// The key it presents.
    pub fn key(&self) -> Option<String> {
        self.key.read().unwrap_or_else(PoisonError::into_inner).clone()
    }

    /// Sends `body` (`content_type`) to `path` once.
    pub fn send(&self, path: &str, content_type: &str, body: &[u8]) -> Answer {
        let url = format!("{}{path}", self.base);
        let mut asked = self.http.post(&url).header("Content-Type", content_type);
        if let Some(key) = self.key() {
            asked = asked.header("Authorization", &format!("Bearer {key}"));
        }
        match asked.send(body) {
            Ok(mut response) => {
                let status = response.status().as_u16();
                let text = response.body_mut().read_to_string().unwrap_or_default();
                let value: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
                match status {
                    200..=299 => Answer::Ok(value),
                    500.. => Answer::Down(format!("o servidor respondeu {status}")),
                    _ => {
                        let said = value.get("message").and_then(Value::as_str).unwrap_or(&text).to_owned();
                        Answer::Refused(status, said)
                    }
                }
            }
            Err(e) => Answer::Down(e.to_string()),
        }
    }

    /// Sends `body` as JSON to the route `what` of agents once.
    pub fn post(&self, what: &str, body: &impl Serialize) -> Answer {
        let bytes = serde_json::to_vec(body).unwrap_or_default();
        self.send(&format!("/api/ci/agentes/{what}"), "application/json", &bytes)
    }

    /// Sends `body` to `what` until the server answers what it means; `None` only when the agent
    /// stops before that.
    pub fn deliver(&self, what: &str, body: &impl Serialize) -> Option<Result<Value, (u16, String)>> {
        let bytes = serde_json::to_vec(body).unwrap_or_default();
        self.deliver_bytes(&format!("/api/ci/agentes/{what}"), "application/json", &bytes)
    }

    /// Sends `bytes` to `path` until the server answers what it means.
    pub fn deliver_bytes(
        &self,
        path: &str,
        content_type: &str,
        bytes: &[u8],
    ) -> Option<Result<Value, (u16, String)>> {
        let mut wait = FIRST_WAIT;
        loop {
            match self.send(path, content_type, bytes) {
                Answer::Ok(value) => return Some(Ok(value)),
                Answer::Refused(status, said) => return Some(Err((status, said))),
                Answer::Down(_) => {
                    if self.rest(wait) {
                        return None;
                    }
                    wait = (wait * 2).min(LONGEST_WAIT);
                }
            }
        }
    }

    /// [`deliver`](Self::deliver), read as `T`.
    pub fn call<T: DeserializeOwned>(
        &self,
        what: &str,
        body: &impl Serialize,
    ) -> Option<Result<T, (u16, String)>> {
        self.deliver(what, body).map(|answer| {
            answer.and_then(|value| {
                serde_json::from_value(value).map_err(|e| (0, format!("a resposta de {what}: {e}")))
            })
        })
    }

    /// Waits `wait`, a little at a time; whether the agent is stopping.
    pub fn rest(&self, wait: Duration) -> bool {
        let step = Duration::from_millis(50);
        let mut slept = Duration::ZERO;
        while slept < wait {
            if self.stopping.load(Ordering::SeqCst) {
                return true;
            }
            std::thread::sleep(step);
            slept += step;
        }
        self.stopping.load(Ordering::SeqCst)
    }
}
