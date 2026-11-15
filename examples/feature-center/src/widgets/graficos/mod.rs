//! Charts and flows: `FlowCanvas`, the cables (`CablePlug`) and the panel of jacks (`PatchPanel`).

pub mod cabos;
pub mod fluxo;
pub mod painel;

use ravenrs::prelude::IconName;

use super::Page;

/// The group of the navigation for this family.
pub const GROUP: &str = "Gráficos e fluxos";

/// The pages of this family, in the order of the navigation.
pub const PAGES: &[Page] = &[
    Page {
        id: fluxo::ID,
        caption: fluxo::PAGE,
        group: GROUP,
        icon: IconName::GitBranch,
        make: || Box::new(fluxo::FlowPage),
    },
    Page {
        id: cabos::ID,
        caption: cabos::PAGE,
        group: GROUP,
        icon: IconName::Plug,
        make: || Box::new(cabos::CablesPage),
    },
    Page {
        id: painel::ID,
        caption: painel::PAGE,
        group: GROUP,
        icon: IconName::Server,
        make: || Box::new(painel::PatchPanelPage),
    },
];
