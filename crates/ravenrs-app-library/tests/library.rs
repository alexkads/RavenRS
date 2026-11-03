//! The records of the library, on their own and in an application.
#![allow(clippy::expect_used, clippy::unwrap_used)]

use ravenrs_app::*;
use ravenrs_app_library::*;
use ravenrs_app_ui::{App, Language};
use ravenrs_core::Size;
use ravenrs_style::Theme;
use ravenrs_widgets::Ui;

fn everything() -> LibraryModule {
    LibraryModule::new()
        .person()
        .organization()
        .address()
        .country()
        .task()
        .event()
        .note()
        .file_record()
        .navigation("Records")
}

fn space_of(module: LibraryModule) -> (Application, MemoryObjectSpace) {
    let application = Application::builder("Test").module(module).build().expect("valid");
    let database = MemoryDatabase::new(application.entities().to_vec()).expect("valid");
    (application, database.space())
}

#[test]
fn every_record_is_valid_and_what_they_point_to_comes_along() {
    let (application, _) = space_of(everything());
    assert_eq!(application.entities().len(), 8);
    let (application, _) = space_of(LibraryModule::new().task());
    let names: Vec<&str> = application.entities().iter().map(|e| e.name.as_str()).collect();
    assert_eq!(names, ["Task", "Person", "Address", "Country"]);
    assert!(application.entities()[1].navigation.is_none(), "not asked for, so out of the navigation");
}

#[test]
fn a_record_can_be_extended_and_replaces_the_one_of_its_name() {
    let extended = person().field(FieldInfo::new("badge", FieldKind::Text));
    let (application, _) = space_of(LibraryModule::new().person().entity(extended));
    let people: Vec<&EntityInfo> = application.entities().iter().filter(|e| e.name == "Person").collect();
    assert_eq!(people.len(), 1);
    assert!(people[0].field_named("badge").is_some());
}

#[test]
fn a_record_of_the_same_name_in_the_application_says_so() {
    let own = EntityInfo::new("Person").field(FieldInfo::new("id", FieldKind::Integer).key());
    let error = Application::builder("T")
        .entity(own)
        .module(LibraryModule::new().person())
        .build()
        .map(|_| ())
        .expect_err("refused");
    assert!(error.0.contains("Person"), "{}", error.0);
}

fn rules_of(application: &Application, name: &str) -> EntityInfo {
    application.entities().iter().find(|e| e.name == name).expect("there").clone()
}

#[test]
fn a_person_is_named_by_first_and_last_name() {
    let (application, mut space) = space_of(everything());
    let mut ana = space.new_object("Person").expect("known");
    ana.set("first_name", "Ana");
    ana.set("last_name", "Lima");
    let key = ana.get("id");
    space.save(ana).expect("saves");
    let found = space.find("Person", &key).expect("reads").expect("there");
    assert_eq!(found.get("full_name"), Value::Text("Ana Lima".into()));
    let info = rules_of(&application, "Person");
    assert_eq!(info.default_property().map(|f| f.name.as_str()), Some("full_name"));
    let mut bad = found;
    bad.set("email", "not an address");
    let wrong = validate(&info, &bad, CONTEXT_SAVE, &space, &RuleWords::default());
    assert_eq!(wrong.len(), 1, "{wrong:?}");
    assert_eq!(wrong[0].field.as_deref(), Some("email"));
}

#[test]
fn an_event_cannot_end_before_it_starts_and_a_task_stays_within_its_percent() {
    let (application, mut space) = space_of(everything());
    let words = RuleWords::default();
    let info = rules_of(&application, "Event");
    let mut event = space.new_object("Event").expect("known");
    event.set("subject", "Reunião");
    event.set("starts", Value::Date(moment(10)));
    assert!(validate(&info, &event, CONTEXT_SAVE, &space, &words).is_empty(), "an end is not needed");
    event.set("ends", Value::Date(moment(9)));
    assert_eq!(validate(&info, &event, CONTEXT_SAVE, &space, &words)[0].field.as_deref(), Some("ends"));
    event.set("ends", Value::Date(moment(11)));
    assert!(validate(&info, &event, CONTEXT_SAVE, &space, &words).is_empty());

    let info = rules_of(&application, "Task");
    let mut task = space.new_object("Task").expect("known");
    task.set("subject", "Ligar");
    assert_eq!(task.get("status"), Value::Text("not_started".into()));
    task.set("percent_done", 150.0);
    assert_eq!(validate(&info, &task, CONTEXT_SAVE, &space, &words).len(), 1);
}

fn moment(days: i64) -> ravenrs_criteria::DateTime {
    ravenrs_criteria::DateTime { days, seconds: 0 }
}

#[test]
fn its_words_in_portuguese_fit_every_record() {
    let mut application =
        Application::builder("T").module(everything()).translation("pt-BR", PT_BR).build().expect("valid");
    application.set_language("pt-BR").expect("valid");
    assert!(application.unusable().is_empty(), "{:?}", application.unusable());
    let task = rules_of(&application, "Task");
    assert_eq!(task.caption, "Tarefas");
    let Some(FieldKind::Enum(options)) = task.field_named("status").map(|f| f.kind.clone()) else {
        panic!("options");
    };
    assert_eq!(options[3].caption, "Concluída");
}

#[test]
fn an_application_shows_them_with_nothing_else_declared() {
    let window = App::new("Contatos")
        .language(Language::BrazilianPortuguese)
        .translation("pt-BR", PT_BR)
        .module(LibraryModule::new().person().task().navigation("Registros"))
        .build()
        .expect("valid");
    let mut ui = Ui::new(window, Theme::raven_dark());
    ui.set_viewport(Size::new(1200.0, 800.0), 1.0);
    ui.update();
    ui.update();
    let tree = ui.access_tree();
    let says = |text: &str| tree.nodes.iter().any(|n| n.name.as_deref() == Some(text));
    assert!(says("Pessoas") && says("Tarefas"));
}
