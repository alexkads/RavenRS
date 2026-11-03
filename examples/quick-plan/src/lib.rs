//! One list, shown as a grid, a chart, a month, a timeline and a tree. Nothing but the model changes.

use ravenrs_app::{EntityInfo, EnumOption, FieldInfo, FieldKind, ObjectSpace, Value, ViewVariant};
use ravenrs_app_ui::{App, CALENDAR_EDITOR, CHART_EDITOR, Language, SCHEDULER_EDITOR, TREE_EDITOR};
use ravenrs_criteria::DateTime;

fn person() -> EntityInfo {
    EntityInfo::new("Person")
        .caption("Pessoas")
        .navigation("Plano")
        .icon("users")
        .field(FieldInfo::new("id", FieldKind::Integer).key())
        .field(FieldInfo::new("name", FieldKind::Text).caption("Nome").default_property())
}

fn task() -> EntityInfo {
    let state = vec![EnumOption::new("open", "Aberta"), EnumOption::new("late", "Atrasada")];
    EntityInfo::new("Task")
        .caption("Tarefas")
        .navigation("Plano")
        .icon("list")
        .field(FieldInfo::new("id", FieldKind::Integer).key())
        .field(FieldInfo::new("title", FieldKind::Text).caption("Título").default_property())
        .field(FieldInfo::new("owner", FieldKind::Reference("Person".into())).caption("Responsável"))
        .field(FieldInfo::new("parent", FieldKind::Reference("Task".into())).caption("Dentro de"))
        .field(FieldInfo::new("starts", FieldKind::DateTime).caption("Início"))
        .field(FieldInfo::new("ends", FieldKind::DateTime).caption("Fim"))
        .field(FieldInfo::new("cost", FieldKind::Money).caption("Custo"))
        .field(FieldInfo::new("state", FieldKind::Enum(state)).caption("Situação"))
        .editor_setting("ChartSettings", "Argument", "owner")
        .editor_setting("ChartSettings", "Series", "cost:Sum")
        .editor_setting("CalendarSettings", "Tone", "state")
        .editor_setting("SchedulerSettings", "Tone", "state")
        .variant(ViewVariant::new("grid", "Lista"))
        .variant(ViewVariant::new("chart", "Gráfico").editor(CHART_EDITOR))
        .variant(ViewVariant::new("month", "Mês").editor(CALENDAR_EDITOR))
        .variant(ViewVariant::new("timeline", "Linha do tempo").editor(SCHEDULER_EDITOR))
        .variant(ViewVariant::new("tree", "Árvore").editor(TREE_EDITOR))
}

fn seed(space: &mut dyn ObjectSpace) -> Result<(), String> {
    let wrong = |error: ravenrs_app::SpaceError| error.to_string();
    for (id, name) in [(1.0, "Ana"), (2.0, "Bia")] {
        let mut person = space.new_object("Person").map_err(wrong)?;
        person.set("id", id);
        person.set("name", name);
        space.save(person).map_err(wrong)?;
    }
    let first =
        DateTime::now().days - i64::from(ravenrs_widgets::Date::from_days(DateTime::now().days).day()) + 1;
    let tasks = [
        (10.0, "Obra", 1.0, None, 1, 20, 500_000.0, "open"),
        (11.0, "Fundação", 1.0, Some(10.0), 2, 8, 200_000.0, "late"),
        (12.0, "Paredes", 2.0, Some(10.0), 9, 15, 150_000.0, "open"),
        (13.0, "Telhado", 2.0, Some(10.0), 16, 20, 150_000.0, "open"),
    ];
    for (id, title, owner, parent, from, to, cost, state) in tasks {
        let moment = |day: i64| Value::Date(DateTime { days: first + day - 1, seconds: 9 * 3600 });
        let mut task = space.new_object("Task").map_err(wrong)?;
        task.set("id", id);
        task.set("title", title);
        task.set("owner", owner);
        task.set("parent", parent.map_or(Value::Null, Value::Number));
        task.set("starts", moment(from));
        task.set("ends", moment(to));
        task.set("cost", cost);
        task.set("state", state);
        space.save(task).map_err(wrong)?;
    }
    Ok(())
}

/// The window, with some records to start from.
pub fn app() -> ravenrs::prelude::AnyView {
    App::new("Raven Plano")
        .language(Language::BrazilianPortuguese)
        .entity_info(task())
        .entity_info(person())
        .seed(seed)
        .into_view()
}

#[cfg(target_arch = "wasm32")]
mod web_entry {
    use wasm_bindgen::prelude::*;

    #[wasm_bindgen(start)]
    pub fn start() {
        console_error_panic_hook::set_once();
        wasm_bindgen_futures::spawn_local(async {
            match ravenrs::web::start(super::app(), ravenrs::web::WebOptions::new("raven-canvas")).await {
                Ok(app) => std::mem::forget(app),
                Err(e) => web_sys::console::error_1(&e.to_string().into()),
            }
        });
    }
}
