//! Charts and flows: the charts, the sankey and the barcodes, `FlowCanvas` and the panels around
//! it, the cables (`CablePlug`) and the panel of jacks (`PatchPanel`).

pub mod barras;
pub mod cabos;
pub mod fluxo;
pub mod paineis;
pub mod painel;
pub mod sankey;
pub mod tipos;

use ravenrs::prelude::IconName;

use super::Page;

/// The group of the navigation for this family.
pub const GROUP: &str = "Gráficos e fluxos";

/// The pages of this family, in the order of the navigation.
pub const PAGES: &[Page] = &[
    Page {
        id: tipos::ID,
        caption: tipos::PAGE,
        group: GROUP,
        icon: IconName::ChartColumn,
        make: || Box::new(tipos::ChartsPage),
    },
    Page {
        id: sankey::ID,
        caption: sankey::PAGE,
        group: GROUP,
        icon: IconName::Shapes,
        make: || Box::new(sankey::SankeyPage),
    },
    Page {
        id: barras::ID,
        caption: barras::PAGE,
        group: GROUP,
        icon: IconName::Rows3,
        make: || Box::new(barras::BarcodePage),
    },
    Page {
        id: fluxo::ID,
        caption: fluxo::PAGE,
        group: GROUP,
        icon: IconName::GitBranch,
        make: || Box::new(fluxo::FlowPage),
    },
    Page {
        id: paineis::ID,
        caption: paineis::PAGE,
        group: GROUP,
        icon: IconName::PanelLeft,
        make: || Box::new(paineis::FlowPanelsPage),
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
