//! The components of the widget families, one page each, every public feature shown.
//!
//! [`PAGES`] is the one list: the center puts each page in its navigation as a dashboard, and the
//! mode `componentes` (#587) shows the same pages with no application framework around them.

pub mod atualizacao;
pub mod basicos;
pub mod doca;
pub mod graficos;
pub mod kit;
pub mod listas;

use ravenrs::prelude::{AnyView, IconName};

/// A page of a component.
#[derive(Clone, Copy, Debug)]
pub struct Page {
    /// Its name for the session and for `pagina`.
    pub id: &'static str,
    /// Its name in the navigation.
    pub caption: &'static str,
    /// The group of the navigation, one per family (skill ravenrs-feature-center).
    pub group: &'static str,
    /// Its icon.
    pub icon: IconName,
    /// What it shows.
    pub make: fn() -> AnyView,
}

/// Every page of a component, in the order of the navigation.
pub const PAGES: &[Page] = &[
    Page {
        id: basicos::botoes::ID,
        caption: basicos::botoes::PAGE,
        group: "Básicos",
        icon: IconName::MousePointer,
        make: || Box::new(basicos::botoes::ButtonsPage),
    },
    Page {
        id: basicos::texto::ID,
        caption: basicos::texto::PAGE,
        group: "Básicos",
        icon: IconName::FileText,
        make: || Box::new(basicos::texto::TextPage),
    },
    Page {
        id: kit::expressoes::ID,
        caption: kit::expressoes::PAGE,
        group: "Kit",
        icon: IconName::Sigma,
        make: || Box::new(kit::expressoes::ExpressionsPage),
    },
    Page {
        id: kit::parametros::ID,
        caption: kit::parametros::PAGE,
        group: "Kit",
        icon: IconName::FileText,
        make: || Box::new(kit::parametros::ParametersPage),
    },
    Page {
        id: listas::inspetor::ID,
        caption: listas::inspetor::PAGE,
        group: "Listas e árvores",
        icon: IconName::Search,
        make: || Box::new(listas::inspetor::InspectorPage),
    },
    Page {
        id: graficos::fluxo::ID,
        caption: graficos::fluxo::PAGE,
        group: "Gráficos e fluxos",
        icon: IconName::GitBranch,
        make: || Box::new(graficos::fluxo::FlowPage),
    },
    Page {
        id: graficos::cabos::ID,
        caption: graficos::cabos::PAGE,
        group: "Gráficos e fluxos",
        icon: IconName::Plug,
        make: || Box::new(graficos::cabos::CablesPage),
    },
    Page {
        id: graficos::painel::ID,
        caption: graficos::painel::PAGE,
        group: "Gráficos e fluxos",
        icon: IconName::Server,
        make: || Box::new(graficos::painel::PatchPanelPage),
    },
    Page {
        id: doca::area::ID,
        caption: doca::area::PAGE,
        group: "Doca e faixa",
        icon: IconName::LayoutDashboard,
        make: || Box::new(doca::area::DockAreaPage),
    },
    Page {
        id: doca::faixa::ID,
        caption: doca::faixa::PAGE,
        group: "Doca e faixa",
        icon: IconName::Rows3,
        make: || Box::new(doca::faixa::RibbonDemoPage),
    },
    Page {
        id: atualizacao::novidades::ID,
        caption: atualizacao::novidades::PAGE,
        group: "Atualização",
        icon: IconName::Download,
        make: || Box::new(atualizacao::novidades::NewsPage),
    },
];

/// The page with this id.
pub fn page(id: &str) -> Option<&'static Page> {
    PAGES.iter().find(|page| page.id == id)
}

/// Registers the control of every component page with the session.
#[cfg(feature = "app")]
pub fn register(session: &ravenrs_app_ui::Session) {
    for page in PAGES {
        session.register_control(page.id, page.make);
    }
}
