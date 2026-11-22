//! Scheduling for RavenRS apps and servers, with no runtime of its own.
//!
//! The crate decides *when*; whoever calls it decides *how* to wait and *what* to run. A server
//! calls [`check`] from a timer, an app from `cx.wake_at`, a test with a fixed [`Timestamp`]. That
//! keeps it free of tokio and of the GPU, so the report server, and next the app framework, Raven
//! Flows and Raven CI, share one engine instead of each copying the last.
//!
//! * [`cron`]: five cron fields, with names, ranges, steps and time zones.
//! * [`Trigger`]: a cron schedule in a zone, or a fixed period.
//! * [`check`] and [`Check`]: given the stored next run and the time now, whether to wait, run or
//!   record a miss, and what to store as the next run.
//! * [`Misfire`]: what to do with a run that came due while the program was not running.
//! * [`Running`]: keeps one job from running twice at once.
//!
//! The rule that keeps a crash from repeating a run is the caller's half of [`check`]: store the
//! `next` it returns *before* starting the run.

pub mod cron;

mod running;
mod trigger;

pub use cron::Schedule;
pub use jiff::{SignedDuration, Timestamp};
pub use running::{Running, RunningGuard};
pub use trigger::{Check, Misfire, Trigger, check};
