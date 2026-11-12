//! What a generated list says when it has no records.
#![allow(clippy::expect_used, clippy::panic)]

use ravenrs_app::{EntityInfo, FieldInfo, FieldKind, ObjectSpace};
use ravenrs_app_ui::{App, Language};
use ravenrs_core::{AccessAction, Event, Key, Modifiers, Role, Size};
use ravenrs_style::Theme;
use ravenrs_widgets::Ui;

fn class(empty: Option<&str>, access: ravenrs_app::DataAccess) -> EntityInfo {
    let entity = EntityInfo::new("Class")
        .caption("Classes")
        .navigation("School")
        .data_access(access)
        .field(FieldInfo::new("id", FieldKind::Integer).key())
        .field(FieldInfo::new("name", FieldKind::Text).default_property());
    match empty {
        Some(text) => entity.empty_text(text),
        None => entity,
    }
}

fn world(entity: EntityInfo, language: Language, records: &'static [&'static str]) -> Ui {
    let app =
        App::new("School").language(language).entity_info(entity).seed(move |space: &mut dyn ObjectSpace| {
            for name in records {
                let mut object = space.new_object("Class").map_err(|e| e.to_string())?;
                object.set("name", *name);
                space.save(object).map_err(|e| e.to_string())?;
            }
            Ok(())
        });
    let mut ui = Ui::new(app.build().expect("valid"), Theme::raven_dark());
    ui.set_viewport(Size::new(1200.0, 800.0), 1.0);
    for _ in 0..6 {
        ui.update();
    }
    // A remote list asks for its rows once scrolling and typing have rested.
    for step in 1..=4 {
        ui.set_time(f64::from(step));
        for _ in 0..3 {
            ui.update();
        }
    }
    ui
}

fn says(ui: &mut Ui, text: &str) -> bool {
    for _ in 0..4 {
        ui.update();
    }
    ui.access_tree().nodes.iter().any(|n| n.name.as_deref() == Some(text) || n.value.as_deref() == Some(text))
}

fn search(ui: &mut Ui, label: &str, text: &str) {
    let tree = ui.access_tree();
    let id = tree
        .nodes
        .iter()
        .find(|n| n.role == Role::TextInput && n.name.as_deref() == Some(label))
        .unwrap_or_else(|| panic!("no field {label}"))
        .id;
    ui.perform_action(id, AccessAction::Focus);
    for character in text.chars() {
        ui.handle_event(&Event::KeyDown {
            key: Key::Character(character.to_string()),
            modifiers: Modifiers::default(),
        });
    }
    ui.set_time(1.0);
}

const OWN: &str = "Nenhuma classe ainda: crie a primeira em Novo.";

#[test]
fn an_empty_list_says_what_the_model_declares() {
    for access in [ravenrs_app::DataAccess::Client, ravenrs_app::DataAccess::Server] {
        let mut ui = world(class(Some(OWN), access), Language::BrazilianPortuguese, &[]);
        assert!(
            says(&mut ui, OWN),
            "{access:?}: {:?}",
            ui.access_tree().nodes.iter().filter_map(|n| n.name.clone()).collect::<Vec<_>>()
        );
    }
}

#[test]
fn without_one_it_says_the_language_s_own_words_and_not_that_filters_hid_everything() {
    let mut ui = world(class(None, ravenrs_app::DataAccess::Client), Language::BrazilianPortuguese, &[]);
    assert!(says(&mut ui, "Nada por aqui ainda."));
    assert!(!says(&mut ui, "Nenhuma linha atende aos filtros."));
    let mut ui = world(class(None, ravenrs_app::DataAccess::Client), Language::English, &[]);
    assert!(says(&mut ui, "Nothing here yet."));
}

#[test]
fn a_search_that_finds_nothing_says_so_and_not_the_empty_words() {
    let mut ui =
        world(class(Some(OWN), ravenrs_app::DataAccess::Client), Language::BrazilianPortuguese, &["Adultos"]);
    search(&mut ui, "Buscar em todas as colunas", "zzz");
    assert!(says(&mut ui, "Nenhuma linha atende aos filtros."));
    assert!(!says(&mut ui, OWN));
}
