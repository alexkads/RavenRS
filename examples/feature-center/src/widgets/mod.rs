//! The components of the widget families, one page each, every public feature shown.
//!
//! [`PAGES`] is the one list: the center puts each page in its navigation as a dashboard, and the
//! mode `componentes` (#587) shows the same pages with no application framework around them.

pub mod agenda;
pub mod basicos;
pub mod conversa;
pub mod doca;
pub mod grade;
pub mod graficos;
pub mod imagem;
pub mod kit;
pub mod listas;

use std::sync::LazyLock;

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

/// The pages of each family, in the order of the navigation (skill ravenrs-feature-center).
const FAMILIES: &[&[Page]] = &[
    basicos::PAGES,
    kit::PAGES,
    listas::PAGES,
    grade::PAGES,
    graficos::PAGES,
    conversa::PAGES,
    doca::PAGES,
    agenda::PAGES,
    imagem::PAGES,
];

/// Every page of a component, in the order of the navigation: each family's, one after another.
pub static PAGES: LazyLock<Vec<Page>> = LazyLock::new(|| FAMILIES.concat());

/// The page with this id.
pub fn page(id: &str) -> Option<&'static Page> {
    PAGES.iter().find(|page| page.id == id)
}

/// Registers the control of every component page with the session.
#[cfg(feature = "app")]
pub fn register(session: &ravenrs_app_ui::Session) {
    for page in PAGES.iter() {
        session.register_control(page.id, page.make);
    }
}
