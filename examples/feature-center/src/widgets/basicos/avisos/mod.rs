//! Part of the family `ravenrs-widgets-base`, group "Básicos": what shows over the rest or tells
//! about it, one page per subject, every public feature of each component in it shown.

pub mod alertas;
pub mod camadas;
pub mod menus;
pub mod numeros_vivos;
pub mod progresso;

use ravenrs::prelude::IconName;

use super::super::Page;

/// The group of the navigation.
pub const GROUP: &str = super::GROUP;

/// The pages of this part, in the order of the navigation.
pub const PAGES: &[Page] = &[
    Page {
        id: alertas::ID,
        caption: alertas::PAGE,
        group: GROUP,
        icon: IconName::Bell,
        make: || Box::new(alertas::AlertsPage),
    },
    Page {
        id: camadas::ID,
        caption: camadas::PAGE,
        group: GROUP,
        icon: IconName::Layers,
        make: || Box::new(camadas::OverlaysPage),
    },
    Page {
        id: menus::ID,
        caption: menus::PAGE,
        group: GROUP,
        icon: IconName::Menu,
        make: || Box::new(menus::MenusPage),
    },
    Page {
        id: progresso::ID,
        caption: progresso::PAGE,
        group: GROUP,
        icon: IconName::LoaderCircle,
        make: || Box::new(progresso::ProgressPage),
    },
    Page {
        id: numeros_vivos::ID,
        caption: numeros_vivos::PAGE,
        group: GROUP,
        icon: IconName::TrendingUp,
        make: || Box::new(numeros_vivos::LiveNumbersPage),
    },
];
