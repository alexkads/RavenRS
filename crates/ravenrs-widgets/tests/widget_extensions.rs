//! The optional features of the basic controls: each test fails if its feature is removed.
#![allow(clippy::expect_used, clippy::unwrap_used, reason = "test helpers fail loudly")]

use std::cell::Cell;
use std::rc::Rc;

use ravenrs_core::{
    AccessAction, AccessNode, AccessTree, Color, Event, ImageData, Key, Modifiers, Point, PointerButton,
    Primitive, Quad, Role, Size, Toggled,
};
use ravenrs_layout::Align;
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

fn hover(ui: &mut Ui, at: Point) {
    ui.handle_event(&Event::PointerMove { position: at });
    ui.update();
}

fn click_node(ui: &mut Ui, role: Role, name: &str) {
    let tree = ui.access_tree();
    let at = named(&tree, role, name).bounds.center();
    click(ui, at);
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

#[test]
fn a_progress_bar_can_say_what_it_is_and_how_far_along() {
    let mut full = ui(VStack::new((ProgressBar::new(0.4)
        .label("Copying")
        .percent()
        .detail("12 of 30")
        .height(10.0)
        .color(Color::rgb(0.1, 0.8, 0.3)),))
    .align(Align::Stretch));
    let tree = full.access_tree();
    let node = named(&tree, Role::ProgressIndicator, "Copying");
    assert_eq!(node.description.as_deref(), Some("12 of 30"));
    assert_eq!(node.value.as_deref(), Some("40%"));
    assert!(node.children.is_empty(), "one node, not its parts");
    assert_eq!(glyph_colors(&full).len(), 3, "label, percentage and detail are drawn");
    assert!(quads(&full).iter().any(|q| q.rect.height() == 10.0), "the bar takes the height it is given");

    let mut plain = ui(VStack::new((ProgressBar::new(0.4),)).align(Align::Stretch));
    assert!(glyph_colors(&plain).is_empty(), "no text unless asked for");
    assert!(quads(&plain).iter().all(|q| q.rect.height() == 6.0), "6 px by default");
    let tree = plain.access_tree();
    assert!(
        tree.nodes.iter().any(|n| n.role == Role::ProgressIndicator && n.value.as_deref() == Some("40%"))
    );
}

#[test]
fn an_avatar_can_show_a_picture_and_a_ring() {
    let picture = ImageData::from_fn(8, 4, |_, _| Color::WHITE).expect("not empty");
    let mut with =
        ui(Avatar::new("Ana Lima").size(40.0).image(Image::new(picture)).border(3.0, Semantic::Accent));
    let image = with
        .scene()
        .primitives()
        .iter()
        .find_map(|p| if let Primitive::Image(i) = p { Some(i.clone()) } else { None })
        .expect("the picture is drawn");
    assert_eq!((image.rect.width(), image.corner_radii[0]), (34.0, 17.0), "inside the ring, round");
    assert!(glyph_colors(&with).is_empty(), "instead of the initials");
    let accent = Theme::raven_dark().color(Semantic::Accent);
    assert!(quads(&with).iter().any(|q| q.border.is_some_and(|b| b.width == 3.0 && b.color == accent)));
    assert_eq!(named(&with.access_tree(), Role::Image, "Ana Lima").children.len(), 0);

    let ringed = ui(Avatar::new("Ana Lima").border(2.0, Semantic::Accent));
    assert!(quads(&ringed)[0].border.is_some_and(|b| b.width == 2.0), "initials take the ring too");
}

#[test]
fn a_small_toggle_is_smaller_and_still_toggles() {
    let on = State::new(false);
    let mut small = ui(Toggle::new(on.binding()).small());
    let track = quads(&small)[0];
    assert_eq!((track.rect.width(), track.rect.height()), (32.0, 18.0));
    let at = track.rect.center();
    click(&mut small, at);
    assert!(on.get());
    assert_eq!(named_switch(&mut small).toggled, Some(Toggled::True));
    let regular = ui(Toggle::new(on.binding()));
    assert_eq!(quads(&regular)[0].rect.width(), 40.0, "40 by default");
}

fn named_switch(ui: &mut Ui) -> AccessNode {
    ui.access_tree().nodes.iter().find(|n| n.role == Role::Switch).expect("a switch").clone()
}

#[test]
fn a_tooltip_can_show_a_key_and_any_view_can_have_one() {
    let mut keyed = ui(Tooltip::new(Button::new("Save"), "Save the file").shortcut("⌘S"));
    let tree = keyed.access_tree();
    let save = named(&tree, Role::Button, "Save the file");
    assert_eq!(save.description.as_deref(), Some("⌘S"), "the key is announced too");
    let at = save.bounds.center();
    let before = glyph_colors(&keyed).len();
    hover(&mut keyed, at);
    let canvas = Theme::raven_dark().color(Semantic::Canvas);
    let drawn = glyph_colors(&keyed);
    assert_eq!(drawn.len(), before + 2, "text and key");
    assert_eq!(drawn.iter().filter(|c| **c == canvas).count(), 2, "the key takes the bubble's ink");

    let mut short = ui(IconButton::new(IconName::Pencil).tooltip("Edit"));
    let tree = short.access_tree();
    let at = named(&tree, Role::Button, "Edit").bounds.center();
    hover(&mut short, at);
    assert_eq!(glyph_colors(&short).iter().filter(|c| **c == canvas).count(), 1, "the bubble opens");
}

fn input(ui: &mut Ui) -> AccessNode {
    ui.access_tree().nodes.iter().find(|n| n.role == Role::TextInput).expect("a text input").clone()
}

#[test]
fn a_text_field_holds_views_before_and_after_the_text() {
    let value = State::new(String::from("12"));
    let mut ui = ui(TextField::new(value.binding())
        .prefix(Text::new("R$"))
        .suffix(Button::new("Apply").size(ButtonSize::Mini)));
    let tree = ui.access_tree();
    let field = input_of(&tree);
    let apply = named(&tree, Role::Button, "Apply");
    assert!(field.children.contains(&apply.id), "the suffix is inside the field");
    assert!(field.bounds.contains(apply.bounds.center()));
    assert!(labels(&tree).iter().any(|n| n == "R$"), "and so is the prefix");
    assert_eq!(field.value.as_deref(), Some("12"), "neither joins the value");
}

fn input_of(tree: &AccessTree) -> &AccessNode {
    tree.nodes.iter().find(|n| n.role == Role::TextInput).expect("a text input")
}

#[test]
fn a_clearable_field_empties_itself_and_keeps_the_keyboard() {
    let value = State::new(String::new());
    let mut ui = ui(TextField::new(value.binding()).clearable());
    assert!(ui.access_tree().find(Role::Button, "Clear").is_none(), "nothing to clear yet");
    value.set(String::from("draft"));
    ui.update();
    click_node(&mut ui, Role::Button, "Clear");
    ui.update();
    assert_eq!(value.get(), "");
    assert!(ui.access_tree().find(Role::Button, "Clear").is_none(), "gone once empty");
    assert_eq!(ui.access_tree().focused().map(|n| n.role), Some(Role::TextInput), "focus is back");
    key(&mut ui, Key::Character("x".into()));
    assert_eq!(value.get(), "x", "typing goes on");

    value.set(String::from("kept"));
    let mut read_only = self::ui(TextField::new(value.binding()).clearable().read_only(true));
    assert!(read_only.access_tree().find(Role::Button, "Clear").is_none(), "only if it can change");
}

#[test]
fn a_text_field_comes_in_sizes_and_compact_is_the_small_one() {
    let height =
        |field: TextField| input(&mut ui(VStack::new((field,)).align(Align::Stretch))).bounds.height();
    let value = State::new(String::new());
    assert_eq!(height(TextField::new(value.binding())), 40.0);
    assert_eq!(height(TextField::new(value.binding()).compact()), 32.0);
    assert_eq!(height(TextField::new(value.binding()).size(ButtonSize::Small)), 32.0);
    assert_eq!(height(TextField::new(value.binding()).size(ButtonSize::Mini)), 22.0);
    assert_eq!(height(TextField::new(value.binding()).size(ButtonSize::Large)), 48.0);
}

#[test]
fn a_text_area_grows_with_the_text_up_to_its_limit() {
    let value = State::new(String::new());
    let mut ui = ui(VStack::new((TextArea::new(value.binding()).grows(2, 4),)).align(Align::Stretch));
    let two = input(&mut ui).bounds.height();
    value.set(String::from("a\nb"));
    ui.update();
    assert_eq!(input(&mut ui).bounds.height(), two, "the minimum holds up to two lines");
    value.set(String::from("a\nb\nc"));
    ui.update();
    let three = input(&mut ui).bounds.height();
    assert!(three > two, "a third line makes it taller");
    value.set(String::from("a\nb\nc\nd"));
    ui.update();
    let four = input(&mut ui).bounds.height();
    assert_eq!(four - three, three - two, "by one line each");
    value.set(String::from("a\nb\nc\nd\ne\nf\ng"));
    ui.update();
    assert_eq!(input(&mut ui).bounds.height(), four, "it stops at the maximum");
    value.set(String::from("a"));
    ui.update();
    assert_eq!(input(&mut ui).bounds.height(), two, "and shrinks back");

    let fixed = State::new(String::from("a\nb\nc\nd\ne\nf\ng"));
    let mut plain = self::ui(VStack::new((TextArea::new(fixed.binding()).rows(2),)).align(Align::Stretch));
    assert_eq!(input(&mut plain).bounds.height(), two, "without it, the height is fixed");
}
