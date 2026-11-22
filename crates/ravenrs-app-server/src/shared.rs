//! What every thread that answers has to agree on: who is signed in, and how many tries to sign in
//! each address made this minute.
//!
//! Each thread keeps its own [`crate::ApiService`], which holds what belongs to one thread (the
//! security context of a session, made of `Rc`s). Those are only a cache: this table says whether a
//! token is still good, so signing out, a session running out or a new password counts for every
//! thread at its next request.

use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};

use ravenrs_app::Value;

/// How often expired sessions are swept, and how long a count of tries lasts.
const MINUTE: Duration = Duration::from_secs(60);

/// A session someone opened by signing in.
#[derive(Clone, Debug)]
pub(crate) struct Signed {
    /// The key of the user it belongs to.
    pub(crate) user: Value,
    /// The name the audit trail writes for it.
    pub(crate) author: String,
    /// When it stops being good.
    pub(crate) expires: Instant,
}

#[derive(Debug)]
struct Table {
    sessions: HashMap<String, Signed>,
    swept: Instant,
    tries: HashMap<String, u32>,
    tries_since: Instant,
}

/// The sessions and tries, shared by every thread of one service.
#[derive(Debug)]
pub(crate) struct Shared {
    table: Mutex<Table>,
}

impl Default for Shared {
    fn default() -> Self {
        let now = Instant::now();
        Self {
            table: Mutex::new(Table {
                sessions: HashMap::new(),
                swept: now,
                tries: HashMap::new(),
                tries_since: now,
            }),
        }
    }
}

impl Shared {
    /// The table. A thread that panicked while holding it left it as it was between two whole
    /// changes, since none of them can stop halfway, so it is still good to use.
    fn table(&self) -> MutexGuard<'_, Table> {
        self.table.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Opens a session.
    pub(crate) fn open(&self, token: String, signed: Signed) {
        let mut table = self.table();
        let now = Instant::now();
        table.sessions.retain(|_, session| session.expires > now);
        table.sessions.insert(token, signed);
    }

    /// The session of `token`, if it is still good at `now`. Expired ones are swept once a minute.
    pub(crate) fn session(&self, token: &str, now: Instant) -> Option<Signed> {
        let mut table = self.table();
        if now.duration_since(table.swept) >= MINUTE {
            table.sessions.retain(|_, session| session.expires > now);
            table.swept = now;
        }
        match table.sessions.get(token) {
            Some(session) if session.expires > now => Some(session.clone()),
            Some(_) => {
                table.sessions.remove(token);
                None
            }
            None => None,
        }
    }

    /// Ends the session of `token`.
    pub(crate) fn close(&self, token: &str) {
        self.table().sessions.remove(token);
    }

    /// Ends every session of `user` but `kept`.
    pub(crate) fn close_others(&self, user: &Value, kept: &str) {
        self.table().sessions.retain(|token, session| token == kept || session.user != *user);
    }

    /// Counts a try from `client`, or says no when that address already made `most` this minute,
    /// or every address together made `most_of_all`.
    pub(crate) fn count_try(&self, client: &str, most: u32, most_of_all: u32) -> bool {
        let mut table = self.table();
        let now = Instant::now();
        if now.duration_since(table.tries_since) >= MINUTE {
            table.tries.clear();
            table.tries_since = now;
        }
        let all: u32 = table.tries.values().sum();
        let mine = table.tries.entry(client.to_owned()).or_insert(0);
        if *mine >= most || all >= most_of_all {
            return false;
        }
        *mine += 1;
        true
    }
}
