//! `ReleaseNotes` and `CodeBlock`: walking through versions and copying a command.
#![allow(clippy::expect_used, reason = "test helpers fail loudly")]

use ravenrs_core::{AccessTree, Event, PointerButton, Size};
use ravenrs_reactive::State;
use ravenrs_style::Theme;
use ravenrs_widgets::*;

fn notes() -> Vec<ReleaseNote> {
    vec![
        ReleaseNote::new("Version 3", "The third")
            .date("Nov 1")
            .badge("New", Tone::Success)
            .changes(["Third change"])
            .section("Why update", "Because of the third.")
            .code("run --this"),
        ReleaseNote::new("Version 2", "The second").badge("Installed", Tone::Neutral),
        ReleaseNote::new("Version 1", "The first"),
    ]
}

fn ui(root: impl View + 'static, width: f32) -> Ui {
    let mut ui = Ui::new(root, Theme::raven_dark());
    ui.set_viewport(Size::new(width, 700.0), 1.0);
    ui.update();
    ui
}

fn has(tree: &AccessTree, name: &str) -> bool {
    tree.nodes.iter().any(|n| n.name.as_deref() == Some(name))
}

fn click(ui: &mut Ui, name: &str) {
    let tree = ui.access_tree();
    let found = tree.nodes.iter().find(|n| n.name.as_deref() == Some(name)).expect(name);
    let at = found.bounds.center();
    ui.handle_event(&Event::PointerMove { position: at });
    ui.handle_event(&Event::PointerDown { position: at, button: PointerButton::Primary });
    ui.handle_event(&Event::PointerUp { position: at, button: PointerButton::Primary });
    ui.update();
}

#[test]
fn it_opens_on_the_version_it_is_told_to() {
    let selected = State::new(1);
    let mut ui = ui(ReleaseNotes::new(notes(), selected.binding()), 1000.0);
    let tree = ui.access_tree();
    assert!(has(&tree, "The second") && !has(&tree, "The third"));
    assert!(has(&tree, "2 of 3"));
}

#[test]
fn a_selection_past_the_end_shows_the_oldest() {
    let selected = State::new(9);
    let mut ui = ui(ReleaseNotes::new(notes(), selected.binding()), 1000.0);
    assert!(has(&ui.access_tree(), "The first"));
}

#[test]
fn the_list_and_the_buttons_walk_through_the_versions() {
    let selected = State::new(0);
    let mut ui = ui(ReleaseNotes::new(notes(), selected.binding()), 1000.0);
    click(&mut ui, "Older");
    assert_eq!(selected.get(), 1);
    click(&mut ui, "Newer");
    assert_eq!(selected.get(), 0);
    // At the newest there is nowhere newer to go.
    click(&mut ui, "Newer");
    assert_eq!(selected.get(), 0);
    click(&mut ui, "Version 1");
    assert_eq!(selected.get(), 2);
    assert!(has(&ui.access_tree(), "The first"));
}

#[test]
fn a_narrow_window_drops_the_list_and_keeps_the_buttons() {
    let selected = State::new(0);
    let mut ui = ui(ReleaseNotes::new(notes(), selected.binding()), 480.0);
    let tree = ui.access_tree();
    assert!(!has(&tree, "Version 1"));
    assert!(has(&tree, "Older"));
}

#[test]
fn no_versions_draws_nothing_instead_of_panicking() {
    let selected = State::new(0);
    let mut ui = ui(ReleaseNotes::new(Vec::new(), selected.binding()), 1000.0);
    assert!(!has(&ui.access_tree(), "Older"));
}

#[test]
fn a_code_block_copies_its_text_and_says_so() {
    let mut ui = ui(CodeBlock::new("run --this"), 800.0);
    click(&mut ui, "Copy");
    assert_eq!(ui.clipboard().read().as_deref(), Some("run --this"));
    assert!(has(&ui.access_tree(), "Copied"));
}

#[test]
fn a_long_command_stays_inside_a_narrow_column() {
    let code = "curl -fsSL https://example.com/a/very/long/path/that/goes/on/and/on/and/on/install.cmd -o /tmp/install.cmd";
    let mut ui = ui(CodeBlock::new(code).width(320.0), 800.0);
    let tree = ui.access_tree();
    let copy = tree.nodes.iter().find(|n| n.name.as_deref() == Some("Copy")).expect("copy");
    assert!(copy.bounds.max_x() <= 320.0, "the button stays in: {:?}", copy.bounds);
}
