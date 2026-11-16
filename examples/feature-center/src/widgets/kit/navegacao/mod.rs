//! Part of the family `ravenrs-widgets-kit`, group "Kit": one page per subject, every public feature of
//! each component in it shown.

pub mod barras;
pub mod colecoes;
pub mod destaques;
pub mod etapas;
pub mod lateral;
pub mod novidades;

use ravenrs::prelude::IconName;

use super::super::Page;

/// The group of the navigation.
pub const GROUP: &str = super::GROUP;

/// The pages of this part, in the order of the navigation.
pub const PAGES: &[Page] = &[
    Page {
        id: lateral::ID,
        caption: lateral::PAGE,
        group: GROUP,
        icon: IconName::PanelLeft,
        make: || Box::new(lateral::NavigationPage),
    },
    Page {
        id: barras::ID,
        caption: barras::PAGE,
        group: GROUP,
        icon: IconName::Keyboard,
        make: || Box::new(barras::ToolbarsPage),
    },
    Page {
        id: colecoes::ID,
        caption: colecoes::PAGE,
        group: GROUP,
        icon: IconName::Table,
        make: || Box::new(colecoes::CollectionsPage),
    },
    Page {
        id: etapas::ID,
        caption: etapas::PAGE,
        group: GROUP,
        icon: IconName::Flag,
        make: || Box::new(etapas::StepsAndMetersPage),
    },
    Page {
        id: destaques::ID,
        caption: destaques::PAGE,
        group: GROUP,
        icon: IconName::Sparkles,
        make: || Box::new(destaques::HighlightsPage),
    },
    Page {
        id: novidades::ID,
        caption: novidades::PAGE,
        group: GROUP,
        icon: IconName::Gift,
        make: || Box::new(novidades::ReleaseNotesPage),
    },
];
