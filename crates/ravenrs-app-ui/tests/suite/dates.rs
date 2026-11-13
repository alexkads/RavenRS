//! Dates in generated lists show the language's way and still sort and filter as dates.
#![allow(clippy::expect_used, clippy::panic)]

use ravenrs_app::{EntityInfo, FieldInfo, FieldKind, ObjectSpace, Value};
use ravenrs_app_ui::{App, Language};
use ravenrs_core::{AccessAction, Event, Key, Modifiers, Role, Size};
use ravenrs_criteria::DateTime as Moment;
use ravenrs_style::Theme;
use ravenrs_widgets::{Date, Ui};

fn meeting() -> EntityInfo {
    EntityInfo::new("Meeting")
        .caption("Meetings")
        .navigation("Church")
        .field(FieldInfo::new("id", FieldKind::Integer).key())
        .field(FieldInfo::new("name", FieldKind::Text).caption("Name").default_property())
        .field(FieldInfo::new("day", FieldKind::Date).caption("Day"))
        .field(FieldInfo::new("starts", FieldKind::DateTime).caption("Starts"))
}

fn moment(year: i32, month: u8, day: u8, hour: u32, minute: u32) -> Value {
    let date = Date::new(year, month, day).expect("a date");
    Value::Date(Moment { days: date.to_days(), seconds: hour * 3600 + minute * 60 })
}

/// Alphabetically by the day first, these would come out in another order than by time.
const MEETINGS: [(&str, (i32, u8, u8), u32); 3] =
    [("Easter", (2026, 4, 5), 9), ("Advent", (2025, 12, 1), 19), ("Harvest", (2026, 3, 30), 10)];

fn world(language: Language) -> Ui {
    let app =
        App::new("Church").language(language).entity_info(meeting()).seed(|space: &mut dyn ObjectSpace| {
            for (name, (year, month, day), hour) in MEETINGS {
                let mut object = space.new_object("Meeting").map_err(|e| e.to_string())?;
                object.set("name", name);
                object.set("day", moment(year, month, day, 0, 0));
                object.set("starts", moment(year, month, day, hour, 30));
                space.save(object).map_err(|e| e.to_string())?;
            }
            Ok(())
        });
    let mut ui = Ui::new(app.build().expect("valid"), Theme::raven_dark());
    ui.set_viewport(Size::new(1200.0, 800.0), 1.0);
    settle(&mut ui);
    ui
}

fn settle(ui: &mut Ui) {
    for _ in 0..6 {
        ui.update();
    }
}

fn cells(ui: &mut Ui) -> Vec<String> {
    settle(ui);
    ui.access_tree()
        .nodes
        .iter()
        .filter(|n| n.role == Role::Cell)
        .filter_map(|n| n.value.clone().or_else(|| n.name.clone()))
        .collect()
}

/// The names in the order the list shows them.
fn names(ui: &mut Ui) -> Vec<String> {
    cells(ui).into_iter().filter(|cell| MEETINGS.iter().any(|(name, ..)| name == cell)).collect()
}

fn click_header(ui: &mut Ui, title: &str) {
    settle(ui);
    let tree = ui.access_tree();
    let id = tree
        .nodes
        .iter()
        .find(|n| n.role == Role::ColumnHeader && n.name.as_deref().is_some_and(|t| t.starts_with(title)))
        .unwrap_or_else(|| panic!("no column {title}"))
        .id;
    ui.perform_action(id, AccessAction::Click);
    settle(ui);
}

fn search(ui: &mut Ui, label: &str, text: &str) {
    settle(ui);
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
    settle(ui);
}

#[test]
fn in_portuguese_days_and_moments_show_day_first() {
    let mut ui = world(Language::BrazilianPortuguese);
    let shown = cells(&mut ui);
    assert!(shown.iter().any(|c| c == "05/04/2026"), "{shown:?}");
    assert!(shown.iter().any(|c| c == "01/12/2025 19:30"), "{shown:?}");
    assert!(!shown.iter().any(|c| c.starts_with("2026-")), "no ISO left: {shown:?}");
}

#[test]
fn in_english_they_stay_year_first() {
    let mut ui = world(Language::English);
    let shown = cells(&mut ui);
    assert!(shown.iter().any(|c| c == "2026-04-05"), "{shown:?}");
    assert!(shown.iter().any(|c| c == "2025-12-01 19:30"), "{shown:?}");
}

#[test]
fn they_sort_by_time_and_not_by_the_text() {
    for column in ["Day", "Starts"] {
        let mut ui = world(Language::BrazilianPortuguese);
        click_header(&mut ui, column);
        assert_eq!(names(&mut ui), ["Advent", "Harvest", "Easter"], "{column}, ascending");
        click_header(&mut ui, column);
        assert_eq!(names(&mut ui), ["Easter", "Harvest", "Advent"], "{column}, descending");
    }
}

#[test]
fn searching_finds_what_is_shown() {
    let mut ui = world(Language::BrazilianPortuguese);
    search(&mut ui, "Buscar em todas as colunas", "/12/2025");
    assert_eq!(names(&mut ui), ["Advent"]);
    let mut ui = world(Language::BrazilianPortuguese);
    search(&mut ui, "Buscar em todas as colunas", "2026 10:30");
    assert_eq!(names(&mut ui), ["Harvest"]);
}
