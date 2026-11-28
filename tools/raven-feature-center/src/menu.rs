//! The navigation of an application gathered in sections (#840): groups under one title, each an
//! item whose screens unfold below it while one of them is showing. The page holds a small
//! application of its own, so its menu stays apart from the center's.
use ravenrs::prelude::*;
use ravenrs_app::{EntityInfo, FieldInfo, FieldKind};
use ravenrs_app_ui::{App, Language};

use crate::actions::Kept;

/// The control registered in the center.
pub const ID: &str = "SectionsMenuDemo";
/// The page's navigation caption.
pub const PAGE: &str = "Menu com seções";

fn entity(name: &str, caption: &str, group: &str, icon: &str) -> EntityInfo {
    EntityInfo::new(name)
        .caption(caption)
        .navigation(group)
        .icon(icon)
        .field(FieldInfo::new("id", FieldKind::Integer).key())
        .field(FieldInfo::new("name", FieldKind::Text).caption("Nome").default_property())
}

/// Two groups in the section "Cadastros da igreja" and one in none, which keeps its own title.
pub fn sections_app() -> App {
    App::new("Igreja de exemplo")
        .language(Language::BrazilianPortuguese)
        .entity_info(entity("ChurchMember", "Membros da igreja", "Membresia", "users"))
        .entity_info(entity("ChurchClass", "Turmas da escola", "Escola dominical", "book-open"))
        .entity_info(entity("ChurchOffering", "Ofertas do culto", "Tesouraria da igreja", "wallet"))
        .navigation_section("Membresia", "Cadastros da igreja")
        .navigation_section("Escola dominical", "Cadastros da igreja")
        // Inside the center, whose window keeps ⌘K and ⌘B.
        .inside_another(true)
}

/// What makes the control that shows [`sections_app`] inside the center.
pub fn control() -> impl Fn() -> AnyView + 'static {
    let kept = Kept(std::rc::Rc::new(sections_app().into_view()));
    move || Box::new(kept.clone()) as AnyView
}
