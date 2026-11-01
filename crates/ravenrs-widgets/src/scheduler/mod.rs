//! A timeline of resources: who or what is busy when, and moving that around.

mod time;
mod view;

pub use time::{TimeAxis, TimePoint, TimeScale};
pub use view::{Resource, Scheduler, SchedulerChange, SchedulerEvent, SchedulerRange};
