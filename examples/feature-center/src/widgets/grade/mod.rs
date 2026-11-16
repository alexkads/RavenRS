//! The family `ravenrs-widgets-grid`, group "Grade": the data grid.

pub mod dados;

use ravenrs::prelude::IconName;

use super::Page;

/// The group of the navigation for this family.
pub const GROUP: &str = "Grade";

/// The pages of this family, in the order of the navigation.
pub const PAGES: &[Page] = &[Page {
    id: dados::ID,
    caption: dados::PAGE,
    group: GROUP,
    icon: IconName::Table2,
    make: || Box::new(dados::DataGridPage),
}];
