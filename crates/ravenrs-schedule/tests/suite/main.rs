//! The tests of ravenrs-schedule, in one binary. Every clock here is fixed: nothing waits.

mod check;
mod running;
mod trigger;

use ravenrs_schedule::Timestamp;

/// A timestamp from RFC 3339 text.
pub fn at(text: &str) -> Timestamp {
    text.parse().unwrap_or_else(|e| panic!("{text}: {e}"))
}
