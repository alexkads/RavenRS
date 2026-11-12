//! A destination for events that behaves as a test says, with no network and no account.

use std::cell::RefCell;
use std::collections::{BTreeSet, VecDeque};
use std::rc::Rc;

use crate::{EventSink, SendError};

/// How [`FakeSink`] answers the next send.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Behavior {
    /// Keeps it and says so.
    Accept,
    /// Refuses it for good.
    Refuse,
    /// Cannot be reached; nothing is kept.
    Down,
    /// Keeps it, and the answer is lost on the way back.
    LoseReply,
}

#[derive(Debug, Default)]
struct State {
    script: VecDeque<Behavior>,
    kept: BTreeSet<String>,
    sends: usize,
    down: bool,
}

/// A destination that keeps one effect per key, as a destination honoring idempotency keys does,
/// and counts every send so a test sees what was sent again.
///
/// Clones share what it kept.
#[derive(Clone, Debug, Default)]
pub struct FakeSink(Rc<RefCell<State>>);

impl FakeSink {
    /// One that accepts everything.
    pub fn new() -> Self {
        Self::default()
    }

    /// Answers the next send this way; once the script runs out, it accepts.
    pub fn then(&self, behavior: Behavior) -> &Self {
        self.0.borrow_mut().script.push_back(behavior);
        self
    }

    /// Whether [`status`](EventSink::status) cannot be reached either.
    pub fn status_down(&self, down: bool) {
        self.0.borrow_mut().down = down;
    }

    /// How many effects it kept: one per key.
    pub fn effects(&self) -> usize {
        self.0.borrow().kept.len()
    }

    /// How many times something was sent to it, kept or not.
    pub fn sends(&self) -> usize {
        self.0.borrow().sends
    }

    /// The keys it kept, in order.
    pub fn keys(&self) -> Vec<String> {
        self.0.borrow().kept.iter().cloned().collect()
    }
}

impl EventSink for FakeSink {
    fn send(&self, key: &str, _payload: &str) -> Result<(), SendError> {
        let mut state = self.0.borrow_mut();
        state.sends += 1;
        match state.script.pop_front().unwrap_or(Behavior::Accept) {
            Behavior::Accept => {
                state.kept.insert(key.to_owned());
                Ok(())
            }
            Behavior::Refuse => Err(SendError::Refused("refused by the fake destination".to_owned())),
            Behavior::Down => Err(SendError::Unavailable),
            Behavior::LoseReply => {
                state.kept.insert(key.to_owned());
                Err(SendError::Unknown)
            }
        }
    }

    fn status(&self, key: &str) -> Result<bool, SendError> {
        let state = self.0.borrow();
        if state.down { Err(SendError::Unavailable) } else { Ok(state.kept.contains(key)) }
    }
}
