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

fn main() -> Result<(), ravenrs::native::PlatformError> {
    let app = App::new("Minhas tarefas")
        .language(Language::BrazilianPortuguese)
        .entity::<Task>()
        .into_view();

    ravenrs::native::run(
        app,
        ravenrs::native::WindowOptions::new("Minhas tarefas").size(1200.0, 760.0),
    )
}
