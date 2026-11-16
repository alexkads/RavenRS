//! Part of the family `ravenrs-widgets-base`, group "Básicos": what holds and arranges the rest,
//! one page per subject, every public feature of each component in it shown.

pub mod camadas;
pub mod cartoes;
pub mod divisoes;
pub mod janela;
pub mod rolagem;
pub mod texto;

use ravenrs::prelude::IconName;

use super::super::Page;

/// The group of the navigation.
pub const GROUP: &str = super::GROUP;

/// The pages of this part, in the order of the navigation.
pub const PAGES: &[Page] = &[
    Page {
        id: texto::ID,
        caption: texto::PAGE,
        group: GROUP,
        icon: IconName::Type,
        make: || Box::new(texto::TextIconsPage),
    },
    Page {
        id: cartoes::ID,
        caption: cartoes::PAGE,
        group: GROUP,
        icon: IconName::LayoutDashboard,
        make: || Box::new(cartoes::CardsPage),
    },
    Page {
        id: camadas::ID,
        caption: camadas::PAGE,
        group: GROUP,
        icon: IconName::Layers,
        make: || Box::new(camadas::GridLayersPage),
    },
    Page {
        id: divisoes::ID,
        caption: divisoes::PAGE,
        group: GROUP,
        icon: IconName::Columns3,
        make: || Box::new(divisoes::SplitsPage),
    },
    Page {
        id: rolagem::ID,
        caption: rolagem::PAGE,
        group: GROUP,
        icon: IconName::ArrowUpDown,
        make: || Box::new(rolagem::ScrollingPage),
    },
    Page {
        id: janela::ID,
        caption: janela::PAGE,
        group: GROUP,
        icon: IconName::Monitor,
        make: || Box::new(janela::WindowPage),
    },
];
