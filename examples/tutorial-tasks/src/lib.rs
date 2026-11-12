//! O app mostrado no primeiro tutorial do RavenDevLab.

use ravenrs_app::BusinessObject;
use ravenrs_app_ui::{App, Language};

#[derive(BusinessObject)]
#[object(caption = "Tarefas", navigation = "Organização", icon = "list")]
struct Task {
    #[field(key)]
    id: i64,
    #[field(caption = "Título", default_property, required)]
    title: String,
    #[field(caption = "Concluída")]
    done: bool,
}

/// Gera a navegação, a lista e o formulário. Os dados ficam em memória.
pub fn app() -> ravenrs::prelude::AnyView {
    App::new("Minhas tarefas").language(Language::BrazilianPortuguese).entity::<Task>().into_view()
}
