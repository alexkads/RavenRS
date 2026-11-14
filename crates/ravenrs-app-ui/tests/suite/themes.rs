//! An application's own themes in the menu, kept for each person and followed with the system.
#![allow(clippy::expect_used, clippy::panic)]

use ravenrs_app::{EntityInfo, FieldInfo, FieldKind, MemoryDatabase, MemorySettings};
use ravenrs_app_ui::App;
use ravenrs_core::{AccessAction, Color, Role, Size};
use ravenrs_style::{Semantic, Theme};
use ravenrs_widgets::Ui;

fn morning_star() -> Theme {
    let mut theme = Theme { name: "Morning Star", ..Theme::raven_light() };
    theme.palette.set(Semantic::Accent, Color::hex(0x0f5e66));
    theme
}

fn vigil() -> Theme {
    let mut theme = Theme { name: "Vigil", ..Theme::raven_dark() };
    theme.palette.set(Semantic::Accent, Color::hex(0x5fc4c9));
    // Dark text on the light teal: a kept theme someone cannot read is not brought back.
    theme.palette.set(Semantic::OnAccent, Color::hex(0x08262a));
    theme
}

fn open(store: &MemorySettings, system_dark: Option<bool>) -> Ui {
    let entity = EntityInfo::new("Note")
        .navigation("Records")
        .field(FieldInfo::new("id", FieldKind::Integer).key())
        .field(FieldInfo::new("name", FieldKind::Text));
    let database = MemoryDatabase::new(vec![entity.clone()]).expect("valid");
    let view = App::new("Church")
        .entity_info(entity)
        .storage(move || database.space())
        .settings(store.clone())
        .themes([morning_star(), vigil()])
        .build()
        .expect("it puts together");
    let mut ui = Ui::new(view, Theme::raven_glass());
    ui.set_viewport(Size::new(1200.0, 800.0), 1.0);
    ui.update();
    if let Some(dark) = system_dark {
        ui.set_system_dark(dark);
    }
    settle(&mut ui);
    ui
}

fn settle(ui: &mut Ui) {
    for _ in 0..4 {
        ui.update();
    }
}

fn press(ui: &mut Ui, name: &str) {
    settle(ui);
    let tree = ui.access_tree();
    let names: Vec<_> = tree.nodes.iter().filter_map(|n| n.name.clone()).collect();
    let id = tree
        .nodes
        .iter()
        .find(|n| matches!(n.role, Role::Button | Role::MenuItem) && n.name.as_deref() == Some(name))
        .unwrap_or_else(|| panic!("no {name} in {names:?}"))
        .id;
    ui.perform_action(id, AccessAction::Click);
    settle(ui);
}

fn menu_items(ui: &mut Ui) -> Vec<String> {
    settle(ui);
    ui.access_tree().nodes.into_iter().filter(|n| n.role == Role::MenuItem).filter_map(|n| n.name).collect()
}

#[test]
fn the_menu_offers_only_the_application_s_themes_and_starts_on_the_first() {
    let mut ui = open(&MemorySettings::default(), None);
    assert_eq!(ui.theme().name, "Morning Star");
    press(&mut ui, "Theme");
    assert_eq!(menu_items(&mut ui), ["Morning Star", "Vigil"]);
}

#[test]
fn the_theme_chosen_comes_back_the_next_time() {
    let store = MemorySettings::default();
    let mut ui = open(&store, None);
    press(&mut ui, "Theme");
    press(&mut ui, "Vigil");
    assert_eq!(ui.theme(), vigil());
    let again = open(&store, None);
    assert_eq!(again.theme().name, "Vigil");
    assert_eq!(again.theme().palette.accent, vigil().palette.accent, "with its own colors");
}

#[test]
fn following_the_system_switches_between_the_application_s_light_and_dark() {
    let store = MemorySettings::default();
    let mut ui = open(&store, Some(false));
    press(&mut ui, "Theme");
    assert!(menu_items(&mut ui).contains(&"Follow the system".to_owned()), "{:?}", menu_items(&mut ui));
    press(&mut ui, "Follow the system");
    assert_eq!(ui.theme().name, "Morning Star");
    ui.set_system_dark(true);
    settle(&mut ui);
    assert_eq!(ui.theme(), vigil());
    let again = open(&store, Some(false));
    assert_eq!(again.theme().name, "Morning Star", "it still follows, with the same pair");
}
