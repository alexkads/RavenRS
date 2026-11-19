//! The stories of the components of this crate, for RavenComponentBook: see
//! `ravenrs_widgets_base::book`.

#[allow(unused_imports)]
use ravenrs_widgets_base::book::{ComponentBook, Meta};

/// The stories of the components of this crate that give them, in the order the book lists
/// them.
pub fn stories() -> Vec<Meta> {
    generated()
}

// rcb-scaffold: begin
mod event_calendar;
mod recurrence_editor;
mod scheduler;
mod timeline;
#[allow(unused_imports)]
use ravenrs_widgets_base::AnyView;

/// The stories written by scripts/rcb-scaffold.py, in name order.
pub(crate) fn generated() -> Vec<Meta> {
    let mut all = Vec::new();
    all.push(<crate::EventCalendar as ravenrs_widgets_base::book::ComponentBook>::book());
    all.push(<crate::RecurrenceEditor as ravenrs_widgets_base::book::ComponentBook>::book());
    all.push(<crate::Scheduler as ravenrs_widgets_base::book::ComponentBook>::book());
    all.push(<crate::Timeline as ravenrs_widgets_base::book::ComponentBook>::book());
    all
}
// rcb-scaffold: end
