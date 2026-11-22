//! Events the server sends as they happen, over Server-Sent Events: a build that changed state, a
//! line of log, a record someone else saved.
//!
//! The application publishes to a topic from any thread with [`LiveChannels::publish`]. Whoever
//! listens at `GET /api/live/{topic}` gets each event once, with an id; coming back with
//! `Last-Event-ID` (or `?after=`) gets what it missed, while the topic still keeps it. The streams
//! live on the async side of the server, so they never take one of the threads that answer.

use std::collections::{HashMap, VecDeque};
use std::fmt;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use serde_json::Value as Json;
use tokio::sync::broadcast;

/// How many events each topic keeps for whoever comes back.
const KEPT: usize = 256;
/// How many events can wait for a slow listener before it is told it missed some.
const BACKLOG: usize = 1024;
/// How often an open stream checks its token is still good and says it is still there.
const CHECK_EVERY: Duration = Duration::from_secs(15);

/// One event of a topic.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct LiveEvent {
    /// Its place in the topic, from 1.
    pub id: u64,
    /// What happened, as `line` or `state`.
    pub kind: String,
    /// What it carries.
    pub data: Json,
}

#[derive(Debug)]
struct Topic {
    sender: broadcast::Sender<LiveEvent>,
    kept: VecDeque<LiveEvent>,
    last: u64,
}

/// What came back for a listener: what it missed, and the events to come.
pub(crate) struct Listening {
    /// Kept events after the id it gave, oldest first.
    pub(crate) missed: Vec<LiveEvent>,
    /// The id it gave is older than anything kept, so some are gone.
    pub(crate) gap: bool,
    /// The events from now on.
    pub(crate) coming: broadcast::Receiver<LiveEvent>,
}

/// The topics of one server, shared by everything that publishes and the server that streams them.
/// Clones share the same topics.
#[derive(Clone)]
pub struct LiveChannels {
    topics: Arc<Mutex<HashMap<String, Topic>>>,
    check: Duration,
}

impl fmt::Debug for LiveChannels {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut names: Vec<String> = self.topics().keys().cloned().collect();
        names.sort();
        f.debug_struct("LiveChannels").field("topics", &names).field("check", &self.check).finish()
    }
}

impl Default for LiveChannels {
    fn default() -> Self {
        Self::checked_every(CHECK_EVERY)
    }
}

impl LiveChannels {
    /// No topics yet. An open stream checks its token every 15 s.
    pub fn new() -> Self {
        Self::default()
    }

    /// No topics yet, with open streams checking their token every `check`. A token signed out,
    /// run out or of someone turned off ends its streams within that time. The same tick keeps
    /// proxies from closing a stream that has nothing to say.
    pub fn checked_every(check: Duration) -> Self {
        Self { topics: Arc::default(), check: check.max(Duration::from_millis(10)) }
    }

    fn topics(&self) -> MutexGuard<'_, HashMap<String, Topic>> {
        // A panic between two whole changes leaves the topics as they were.
        self.topics.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Lets people listen to `topic`. Listening to a topic nobody opened is refused with 404.
    pub fn open(&self, topic: impl Into<String>) {
        self.topics().entry(topic.into()).or_insert_with(|| Topic {
            sender: broadcast::channel(BACKLOG).0,
            kept: VecDeque::new(),
            last: 0,
        });
    }

    /// Whether `topic` was opened.
    pub fn is_open(&self, topic: &str) -> bool {
        self.topics().contains_key(topic)
    }

    /// Sends an event to everyone listening to `topic`, from any thread, and keeps it for whoever
    /// comes back. Returns its id, or `None` when the topic was not opened.
    pub fn publish(&self, topic: &str, kind: impl Into<String>, data: Json) -> Option<u64> {
        let mut topics = self.topics();
        let topic = topics.get_mut(topic)?;
        topic.last += 1;
        let event = LiveEvent { id: topic.last, kind: kind.into(), data };
        if topic.kept.len() == KEPT {
            topic.kept.pop_front();
        }
        topic.kept.push_back(event.clone());
        // Nobody listening is not a failure: the event is kept.
        let _ = topic.sender.send(event);
        Some(topic.last)
    }

    /// The id of the last event of `topic`, or 0 when it has none yet.
    pub fn last_id(&self, topic: &str) -> Option<u64> {
        self.topics().get(topic).map(|topic| topic.last)
    }

    pub(crate) fn check(&self) -> Duration {
        self.check
    }

    /// Starts listening to `topic` after the event `after`. What it missed and what comes next are
    /// taken under the same lock, so nothing falls between them.
    pub(crate) fn listen(&self, topic: &str, after: Option<u64>) -> Option<Listening> {
        let topics = self.topics();
        let topic = topics.get(topic)?;
        let coming = topic.sender.subscribe();
        let Some(after) = after else { return Some(Listening { missed: Vec::new(), gap: false, coming }) };
        let oldest = topic.kept.front().map_or(topic.last + 1, |event| event.id);
        let gap = after.saturating_add(1) < oldest && after < topic.last;
        let missed = topic.kept.iter().filter(|event| event.id > after).cloned().collect();
        Some(Listening { missed, gap, coming })
    }
}
