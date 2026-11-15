//! The components of `ravenrs-widgets-kit`, one page each.

pub mod expressoes;
pub mod parametros;

use ravenrs::prelude::IconName;

use super::Page;

/// The group of the navigation for this family.
pub const GROUP: &str = "Kit";

/// The pages of this family, in the order of the navigation.
pub const PAGES: &[Page] = &[
    Page {
        id: expressoes::ID,
        caption: expressoes::PAGE,
        group: GROUP,
        icon: IconName::Sigma,
        make: || Box::new(expressoes::ExpressionsPage),
    },
    Page {
        id: parametros::ID,
        caption: parametros::PAGE,
        group: GROUP,
        icon: IconName::FileText,
        make: || Box::new(parametros::ParametersPage),
    },
];
