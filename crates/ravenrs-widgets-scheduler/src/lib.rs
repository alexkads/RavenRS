//! RavenRS time on screen: the `Scheduler` and its time axis, the `EventCalendar` with its
//! `RecurrenceEditor`, the `YearView`, and the `Timeline`.
//!
//! Applications depend on `ravenrs-widgets` (or `ravenrs`), which re-exports all of this by the
//! same names.

// The base and the kit, by the names the components use: `crate::Button`, `crate::Tabs`,
// `crate::context::BuildCx`.
/// The stories of the components of this crate (feature `book`).
#[cfg(feature = "book")]
pub mod book;

#[allow(unused_imports)]
use ravenrs_widgets_base::*;
#[allow(unused_imports)]
use ravenrs_widgets_kit::*;

#[doc(hidden)]
pub mod event_calendar;
#[doc(hidden)]
pub mod scheduler;
#[cfg(feature = "testing")]
pub mod testing;
#[doc(hidden)]
pub mod timeline;
#[doc(hidden)]
pub mod year_view;

pub use event_calendar::{CalendarEvent, CalendarView, EventCalendar, Hemisphere, RecurrenceEditor};
pub use scheduler::{
    Resource, Scheduler, SchedulerChange, SchedulerEvent, SchedulerRange, TimeAxis, TimePoint, TimeScale,
};
pub use timeline::{Timeline, TimelineEdit, TimelineItem, TimelineMarker, TimelineTrack};
pub use year_view::{DayMark, YearView};
