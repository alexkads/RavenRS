//! The components of the widget families, one page each, every public feature shown.
//!
//! [`PAGES`] is the one list: the center puts each page in its navigation as a dashboard, and the
//! mode `componentes` (#587) shows the same pages with no application framework around them.

pub mod graficos;
pub mod kit;

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
        id: kit::parametros::ID,
        caption: kit::parametros::PAGE,
        group: "Kit",
        icon: IconName::FileText,
        make: || Box::new(kit::parametros::ParametersPage),
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
