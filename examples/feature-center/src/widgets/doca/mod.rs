//! The family `ravenrs-widgets-dock`, group "Doca e faixa": panels that dock around a center, and
//! the ribbon of commands.

pub mod area;
pub mod faixa;

use ravenrs::prelude::IconName;

use super::Page;

/// The group of the navigation for this family.
pub const GROUP: &str = "Doca e faixa";

/// The pages of this family, in the order of the navigation.
pub const PAGES: &[Page] = &[
    Page {
        id: area::ID,
        caption: area::PAGE,
        group: GROUP,
        icon: IconName::LayoutDashboard,
        make: || Box::new(area::DockAreaPage),
    },
    Page {
        id: faixa::ID,
        caption: faixa::PAGE,
        group: GROUP,
        icon: IconName::Rows3,
        make: || Box::new(faixa::RibbonDemoPage),
    },
];
