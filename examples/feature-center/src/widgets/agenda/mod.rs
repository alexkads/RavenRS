//! The family `ravenrs-widgets-scheduler`, group "Agenda": a timeline by resource, a calendar of
//! events with its rule of recurrence, and a timeline of tracks.

pub mod calendario;
pub mod linha;
pub mod recurso;

use ravenrs::prelude::IconName;

use super::Page;

/// The group of the navigation for this family.
pub const GROUP: &str = "Agenda";

/// The pages of this family, in the order of the navigation.
pub const PAGES: &[Page] = &[
    Page {
        id: recurso::ID,
        caption: recurso::PAGE,
        group: GROUP,
        icon: IconName::Users,
        make: || Box::new(recurso::SchedulerPage),
    },
    Page {
        id: calendario::ID,
        caption: calendario::PAGE,
        group: GROUP,
        icon: IconName::Calendar,
        make: || Box::new(calendario::EventCalendarPage),
    },
    Page {
        id: linha::ID,
        caption: linha::PAGE,
        group: GROUP,
        icon: IconName::Video,
        make: || Box::new(linha::TimelinePage),
    },
];
