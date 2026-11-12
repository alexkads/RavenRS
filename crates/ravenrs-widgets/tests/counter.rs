//! Behavior of `KeyBar` and `ScanField`, driven through the public `Ui` API.
#![allow(clippy::expect_used)]

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use ravenrs_core::{AccessAction, Event, Key, Modifiers, Point, PointerButton, Primitive, Role, Size};
use ravenrs_reactive::{Binding, State};
use ravenrs_style::{Semantic, Theme};
use ravenrs_widgets::*;

fn ui(view: impl View + 'static) -> Ui {
    let mut ui = Ui::new(view, Theme::raven_dark());
    ui.set_viewport(Size::new(800.0, 600.0), 1.0);
    settle(&mut ui);
    ui
}

fn settle(ui: &mut Ui) {
    for _ in 0..3 {
        ui.update();
    }
}

fn press(ui: &mut Ui, key: Key, modifiers: Modifiers) {
    ui.handle_event(&Event::KeyDown { key, modifiers });
    settle(ui);
}

fn type_text(ui: &mut Ui, text: &str) {
    for letter in text.chars() {
        press(ui, Key::Character(letter.to_string()), Modifiers::default());
    }
}

fn click(ui: &mut Ui, at: Point) {
    ui.handle_event(&Event::PointerMove { position: at });
    ui.handle_event(&Event::PointerDown { position: at, button: PointerButton::Primary });
    ui.handle_event(&Event::PointerUp { position: at, button: PointerButton::Primary });
    settle(ui);
}

fn count_of(count: &Rc<Cell<u32>>) -> impl Fn() + 'static {
    let count = count.clone();
    move || count.set(count.get() + 1)
}

fn has_button(ui: &mut Ui, name: &str) -> bool {
    let tree = ui.access_tree();
    tree.nodes.iter().any(|node| node.role == Role::Button && node.name.as_deref() == Some(name))
}

fn field_of(state: &State<String>) -> TextField {
    let (read, write) = (state.clone(), state.clone());
    TextField::new(Binding::new(move || read.get(), move |value: String| write.set(value)))
}

fn f9() -> Hotkey {
    Hotkey::key(Key::Function(9))
}

#[test]
fn a_key_of_the_bar_works_wherever_the_focus_is_and_says_what_it_does() {
    let count = Rc::new(Cell::new(0));
    let other = State::new(String::new());
    let bar = KeyBar::new().item(KeyBarItem::new(f9(), "Pay", count_of(&count)));
    let mut ui = ui(VStack::new((field_of(&other).autofocus(), bar)));
    assert!(has_button(&mut ui, "F9 Pay"), "the button shows the key and what it does");

    type_text(&mut ui, "a");
    assert_eq!(other.get(), "a", "the other field has the focus");
    press(&mut ui, Key::Function(9), Modifiers::default());
    assert_eq!(count.get(), 1);
    press(&mut ui, Key::Function(8), Modifiers::default());
    assert_eq!(count.get(), 1, "another key does nothing");
}

#[test]
fn a_key_of_the_bar_also_takes_a_click() {
    let count = Rc::new(Cell::new(0));
    let mut ui = ui(KeyBar::new().item(KeyBarItem::new(f9(), "Pay", count_of(&count))));
    click(&mut ui, Point::new(400.0, 16.0));
    assert_eq!(count.get(), 1);
}

#[test]
fn alt_and_a_letter_can_be_a_key_of_the_bar() {
    let count = Rc::new(Cell::new(0));
    let bar = KeyBar::new().item(KeyBarItem::new(Hotkey::alt("c"), "Customer", count_of(&count)));
    let mut ui = ui(bar);
    let label = Hotkey::alt("c").label();
    assert!(has_button(&mut ui, &format!("{label} Customer")));
    press(&mut ui, Key::Character("c".into()), Modifiers::default());
    assert_eq!(count.get(), 0, "without Alt it is only a letter");
    press(&mut ui, Key::Character("c".into()), Modifiers { alt: true, ..Modifiers::default() });
    assert_eq!(count.get(), 1);
    let composed = Modifiers { alt: true, base_key: Some('c'), ..Modifiers::default() };
    press(&mut ui, Key::Character("ç".into()), composed);
    assert_eq!(count.get(), 2, "on a Mac, Option composes an accent, and it is still the same key");
}

#[test]
fn a_disabled_key_does_nothing_by_key_or_by_click() {
    let count = Rc::new(Cell::new(0));
    let bar = KeyBar::new().item(KeyBarItem::new(f9(), "Pay", count_of(&count)).disabled(true));
    let mut ui = ui(bar);
    assert!(has_button(&mut ui, "F9 Pay"), "it still shows");
    press(&mut ui, Key::Function(9), Modifiers::default());
    click(&mut ui, Point::new(400.0, 16.0));
    assert_eq!(count.get(), 0);
}

#[test]
fn the_primary_key_stands_out_and_takes_more_room() {
    let bar = KeyBar::new()
        .item(KeyBarItem::new(Hotkey::key(Key::Function(1)), "Help", || {}))
        .item(KeyBarItem::new(f9(), "Pay", || {}).primary());
    let ui = ui(bar);
    let accent = Theme::raven_dark().color(Semantic::Accent);
    let mut filled = ui.scene().primitives().iter().filter_map(|primitive| match primitive {
        Primitive::Quad(quad) if quad.fill == accent.into() => Some(quad.rect),
        _ => None,
    });
    let primary = filled.next().expect("only the primary key has the accent");
    assert!(primary.x() > 200.0, "it is the one on the right: {primary:?}");
    assert!(primary.width() > 800.0 * 0.5, "and takes more than half of the bar: {primary:?}");
}

/// A field that writes down what it takes in.
fn scans(change: impl Fn(ScanField) -> ScanField) -> (ScanField, Rc<RefCell<Vec<Scan>>>) {
    let seen: Rc<RefCell<Vec<Scan>>> = Rc::default();
    let keep = seen.clone();
    (change(ScanField::new(move |scan| keep.borrow_mut().push(scan)).placeholder("Code")), seen)
}

fn scan_value(ui: &mut Ui) -> String {
    let tree = ui.access_tree();
    tree.nodes
        .iter()
        .find(|node| node.role == Role::TextInput)
        .and_then(|node| node.value.clone())
        .unwrap_or_default()
}

#[test]
fn the_scan_field_takes_a_code_and_is_empty_again() {
    let (field, seen) = scans(|field| field);
    let mut ui = ui(field);
    type_text(&mut ui, "789");
    assert_eq!(scan_value(&mut ui), "789", "it has the focus from the start");
    press(&mut ui, Key::Enter, Modifiers::default());
    assert_eq!(seen.borrow()[0].text, "789");
    assert_eq!(scan_value(&mut ui), "", "it empties after each code");
    type_text(&mut ui, "123");
    press(&mut ui, Key::Enter, Modifiers::default());
    assert_eq!(seen.borrow()[1].text, "123", "the next one needs no click");
}

#[test]
fn an_empty_scan_is_no_scan_and_spaces_do_not_count() {
    let (field, seen) = scans(|field| field);
    let mut ui = ui(field);
    press(&mut ui, Key::Enter, Modifiers::default());
    type_text(&mut ui, "   ");
    press(&mut ui, Key::Enter, Modifiers::default());
    assert!(seen.borrow().is_empty());
    type_text(&mut ui, " 42 ");
    press(&mut ui, Key::Enter, Modifiers::default());
    assert_eq!(seen.borrow()[0].text, "42");
}

#[test]
fn the_pace_tells_a_scanner_from_fingers() {
    let (field, seen) = scans(|field| field);
    let mut ui = ui(field);
    let mut at = 1.0;
    let mut typed_at_pace = |ui: &mut Ui, gap: f64| {
        for letter in "7891000100103".chars() {
            at += gap;
            ui.set_time(at);
            press(ui, Key::Character(letter.to_string()), Modifiers::default());
        }
    };
    typed_at_pace(&mut ui, 0.01);
    press(&mut ui, Key::Enter, Modifiers::default());
    typed_at_pace(&mut ui, 0.3);
    press(&mut ui, Key::Enter, Modifiers::default());
    let seen = seen.borrow();
    assert!(seen[0].from_reader, "a scanner types fast");
    assert!(!seen[1].from_reader, "fingers do not");
}

#[test]
fn the_scan_field_takes_the_focus_back_when_nothing_has_it() {
    let (field, seen) = scans(|field| field);
    let mut ui = ui(field);
    click(&mut ui, Point::new(400.0, 500.0));
    type_text(&mut ui, "55");
    press(&mut ui, Key::Enter, Modifiers::default());
    assert_eq!(seen.borrow()[0].text, "55", "a click on nothing does not take a scanner away");
}

#[test]
fn the_scan_field_leaves_the_focus_with_a_field_that_took_it() {
    let (field, seen) = scans(|field| field);
    let other = State::new(String::new());
    let mut ui = ui(VStack::new((field, field_of(&other).placeholder("Note"))));
    let tree = ui.access_tree();
    let note =
        tree.nodes.iter().find(|node| node.name.as_deref() == Some("Note")).expect("the other field").id;
    ui.perform_action(note, AccessAction::Focus);
    settle(&mut ui);
    type_text(&mut ui, "x");
    assert_eq!(other.get(), "x", "the keys go to where the person went");
    press(&mut ui, Key::Enter, Modifiers::default());
    assert!(seen.borrow().is_empty());
}

#[test]
fn a_disabled_scan_field_takes_nothing_in() {
    let (field, seen) = scans(|field| field.disabled(true));
    let mut ui = ui(field);
    type_text(&mut ui, "789");
    press(&mut ui, Key::Enter, Modifiers::default());
    assert!(seen.borrow().is_empty());
}
