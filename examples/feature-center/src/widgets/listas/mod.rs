//! The components of `ravenrs-widgets-lists`, one page each.

pub mod inspetor;

use ravenrs::prelude::IconName;

use super::Page;

/// The group of the navigation for this family.
pub const GROUP: &str = "Listas e árvores";

/// The pages of this family, in the order of the navigation.
pub const PAGES: &[Page] = &[Page {
    id: inspetor::ID,
    caption: inspetor::PAGE,
    group: GROUP,
    icon: IconName::Search,
    make: || Box::new(inspetor::InspectorPage),
}];
