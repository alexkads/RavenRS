//! The calculator through its interface: the buttons and the keyboard reach the display.
#![allow(clippy::expect_used, reason = "test helpers fail loudly")]

use ravenrs::core::{AccessAction, Event, Key, Modifiers, Size};
use ravenrs::prelude::*;

fn opened() -> Ui {
    let mut ui = Ui::new(calculator::app(), Theme::raven_light());
    ui.set_viewport(Size::new(340.0, 370.0), 1.0);
    ui.update();
    ui
}

fn press(ui: &mut Ui, name: &str) {
    let tree = ui.access_tree();
    let button = tree.nodes.iter().find(|n| n.name.as_deref() == Some(name)).expect("the button").id;
    ui.perform_action(button, AccessAction::Click);
    ui.update();
}

fn shows(ui: &mut Ui, text: &str) -> bool {
    ui.access_tree().nodes.iter().any(|n| n.name.as_deref() == Some(text))
}

fn key(ui: &mut Ui, key: Key) {
    ui.handle_event(&Event::KeyDown { key, modifiers: Modifiers::default() });
    ui.update();
}

#[test]
fn every_button_fits_the_window() {
    let mut ui = opened();
    let tree = ui.access_tree();
    let buttons: Vec<_> = tree.nodes.iter().filter(|n| n.role == Role::Button).collect();
    assert_eq!(buttons.len(), 20);
    for button in buttons {
        assert!(button.bounds.max_x() <= 340.0 && button.bounds.max_y() <= 370.0, "{:?}", button.name);
    }
}

#[test]
fn the_buttons_calculate() {
    let mut ui = opened();
    for name in ["1", "2", "Add", "3", "Equals"] {
        press(&mut ui, name);
    }
    assert!(shows(&mut ui, "15"));
}

#[test]
fn the_keyboard_calculates() {
    let mut ui = opened();
    for text in ["8", "*", "5"] {
        key(&mut ui, Key::Character(text.into()));
    }
    key(&mut ui, Key::Enter);
    assert!(shows(&mut ui, "40"));
    key(&mut ui, Key::Escape);
    assert!(shows(&mut ui, "0"));
}

#[test]
fn the_number_stands_on_the_right() {
    let mut ui = opened();
    press(&mut ui, "7");
    let tree = ui.access_tree();
    let number = tree.nodes.iter().find(|n| n.role != Role::Button && n.name.as_deref() == Some("7"));
    let number = number.expect("the display");
    assert!(number.bounds.max_x() > 320.0, "{:?}", number.bounds);
}
