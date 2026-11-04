//! The optional features of the basic controls: each test fails if its feature is removed.
#![allow(clippy::expect_used, clippy::unwrap_used, reason = "test helpers fail loudly")]

use std::cell::Cell;
use std::rc::Rc;

use ravenrs_core::{
    AccessAction, AccessNode, AccessTree, Color, Event, Key, Modifiers, Point, PointerButton, Primitive,
    Quad, Role, Size, Toggled,
};
use ravenrs_reactive::State;
use ravenrs_style::{Semantic, Theme};
use ravenrs_widgets::*;

fn ui(view: impl View + 'static) -> Ui {
    let mut ui = Ui::new(view, Theme::raven_dark());
    ui.set_viewport(Size::new(800.0, 600.0), 1.0);
    ui.update();
    ui
}

fn key(ui: &mut Ui, key: Key) {
    ui.handle_event(&Event::KeyDown { key, modifiers: Modifiers::default() });
    ui.update();
}

fn click(ui: &mut Ui, at: Point) {
    ui.handle_event(&Event::PointerMove { position: at });
    ui.handle_event(&Event::PointerDown { position: at, button: PointerButton::Primary });
    ui.handle_event(&Event::PointerUp { position: at, button: PointerButton::Primary });
    ui.update();
}

fn named<'a>(tree: &'a AccessTree, role: Role, name: &str) -> &'a AccessNode {
    tree.find(role, name).unwrap_or_else(|| {
        let all: Vec<_> = tree.nodes.iter().map(|n| format!("{:?} {:?}", n.role, n.name)).collect();
        panic!("no {role:?} named {name:?} in {all:#?}")
    })
}

fn quads(ui: &Ui) -> Vec<Quad> {
    ui.scene()
        .primitives()
        .iter()
        .filter_map(|p| if let Primitive::Quad(q) = p { Some(*q) } else { None })
        .collect()
}

fn glyph_colors(ui: &Ui) -> Vec<Color> {
    ui.scene()
        .primitives()
        .iter()
        .filter_map(|p| if let Primitive::Glyphs(g) = p { Some(g.color) } else { None })
        .collect()
}

fn labels(tree: &AccessTree) -> Vec<String> {
    tree.nodes.iter().filter_map(|n| n.name.clone()).collect()
}

#[test]
fn a_loading_button_keeps_its_width_and_ignores_clicks_and_keys() {
    let (loading, clicks) = (State::new(false), Rc::new(Cell::new(0)));
    let (flag, count) = (loading.clone(), clicks.clone());
    let mut ui = ui(ravenrs_widgets::responsive(move |_| {
        let count = count.clone();
        Button::new("Save changes").loading(flag.get()).on_click(move || count.set(count.get() + 1))
    }));
    let tree = ui.access_tree();
    let idle = named(&tree, Role::Button, "Save changes").clone();
    assert_eq!(ui.next_wake(), None, "still while it is not loading");
    click(&mut ui, idle.bounds.center());
    assert_eq!(clicks.get(), 1);

    loading.set(true);
    ui.update();
    let tree = ui.access_tree();
    let busy = named(&tree, Role::Button, "Save changes");
    assert_eq!(busy.bounds.width(), idle.bounds.width(), "the width does not jump");
    assert!(busy.disabled, "announced as unavailable");
    assert_eq!(busy.description.as_deref(), Some("Loading"), "and as loading");
    assert!(busy.focusable, "the keyboard stays where it was");
    assert!(!busy.actions.contains(&AccessAction::Click));
    assert!(ui.next_wake().is_some(), "the indicator animates");
    assert!(glyph_colors(&ui).iter().all(|c| c.a == 0.0), "the label holds its place without showing");
    click(&mut ui, idle.bounds.center());
    key(&mut ui, Key::Enter);
    assert_eq!(clicks.get(), 1, "ignores the click and Enter");
}

#[test]
fn a_selected_button_lights_up_and_announces_it() {
    let plain = ui(Button::new("Grid").style(ButtonStyle::Ghost));
    let (mut on, mut off) = (
        ui(Button::new("Grid").style(ButtonStyle::Ghost).selected(true)),
        ui(Button::new("Grid").style(ButtonStyle::Ghost).selected(false)),
    );
    let (mut plain_ui, tree_on, tree_off) = (plain, on.access_tree(), off.access_tree());
    assert_eq!(named(&tree_on, Role::Button, "Grid").toggled, Some(Toggled::True));
    assert_eq!(named(&tree_off, Role::Button, "Grid").toggled, Some(Toggled::False));
    assert_eq!(named(&plain_ui.access_tree(), Role::Button, "Grid").toggled, None, "only when asked for");
    assert!(quads(&on).iter().any(|q| q.border.is_some()), "more than color: an outline too");
    assert!(quads(&off).iter().all(|q| q.border.is_none()), "off looks like a plain ghost button");
}

#[test]
fn a_selected_icon_button_is_a_toggle_for_screen_readers() {
    let mut ui = ui(IconButton::new(IconName::Pencil).label("Edit").selected(true));
    assert_eq!(named(&ui.access_tree(), Role::Button, "Edit").toggled, Some(Toggled::True));
}

#[test]
fn a_toggle_button_counts_and_can_be_a_pill() {
    let value = State::new(false);
    let mut with = ui(ToggleButton::new(value.binding(), "Open").count(7).pill());
    let tree = with.access_tree();
    let button = named(&tree, Role::Button, "Open");
    assert_eq!(button.value.as_deref(), Some("7"), "the count is the value, not part of the name");
    let at = button.bounds.center();
    assert!(glyph_colors(&with).len() >= 2, "label and count are both drawn");
    assert!(quads(&with).iter().any(|q| q.corner_radii[0] == 17.0), "pill ends");
    click(&mut with, at);
    assert!(value.get());
    let plain = ui(ToggleButton::new(value.binding(), "Open"));
    assert!(quads(&plain).iter().all(|q| q.corner_radii[0] == 8.0), "corners by default");
}

#[test]
fn a_badge_can_have_an_outline_its_own_color_and_a_height() {
    let color = Color::rgb(0.2, 0.4, 0.9);
    let tinted = ui(Badge::new("Tag").color(color));
    let quad = quads(&tinted)[0];
    assert_eq!(quad.fill, color.with_alpha(0.14).into(), "its own color, not a tone");
    let outlined = ui(Badge::new("Tag").color(color).outline());
    let quad = quads(&outlined)[0];
    assert_eq!(quad.fill, Color::TRANSPARENT.into(), "no fill");
    assert_eq!(quad.border.map(|b| b.color), Some(color.with_alpha(0.6)));
    assert_eq!(quads(&ui(Badge::new("Tag").height(30.0)))[0].rect.height(), 30.0);
}

#[test]
fn a_key_cap_takes_the_color_of_the_button_it_is_in() {
    let button = ui(Button::new("Open").shortcut("F8"));
    let on_accent = Theme::raven_dark().color(Semantic::OnAccent);
    let colors = glyph_colors(&button);
    assert_eq!(colors, [on_accent, on_accent], "the key is the color of the label");
    assert_eq!(
        quads(&button)
            .iter()
            .filter(|q| q.fill == Theme::raven_dark().color(Semantic::Control).into())
            .count(),
        0,
        "and not a gray box inside the button"
    );
    let mut button = button;
    let tree = button.access_tree();
    let node = named(&tree, Role::Button, "Open");
    assert_eq!(node.description.as_deref(), Some("F8"), "the key stays out of the name");

    let muted = Theme::raven_dark().color(Semantic::InkMuted);
    assert_eq!(glyph_colors(&ui(Kbd::new("F8").inherit_color())), [muted], "nothing around it to inherit");
    assert_eq!(glyph_colors(&ui(Kbd::new("F8"))), [muted], "and the usual cap by default");
}

#[test]
fn a_compact_alert_is_a_thin_strip_with_a_stripe() {
    let mut tall = ui(Alert::message("Originals are deleted").tone(Tone::Danger));
    let mut thin = ui(Alert::message("Originals are deleted").tone(Tone::Danger).compact());
    let (a, b) = (tall.access_tree(), thin.access_tree());
    let height = |t: &AccessTree| t.nodes.iter().find(|n| n.role == Role::Alert).unwrap().bounds.height();
    assert!(height(&b) < height(&a) - 10.0, "{} is not under {}", height(&b), height(&a));
    assert!(labels(&b).iter().any(|n| n.contains("Originals")), "the message is still read");
    assert!(quads(&thin).iter().any(|q| q.rect.width() == 2.0), "the stripe at the edge");
}
