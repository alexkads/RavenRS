//! The small widgets of the second batch: each test fails if its feature is removed.
#![allow(clippy::expect_used, clippy::unwrap_used, reason = "test helpers fail loudly")]

use std::cell::Cell;
use std::rc::Rc;

use ravenrs_core::{
    AccessAction, AccessNode, AccessTree, Color, Event, Key, Modifiers, Point, PointerButton, Primitive,
    Quad, Rect, Role, Size,
};
use ravenrs_reactive::State;
use ravenrs_style::{Semantic, Theme};
use ravenrs_widgets::*;

fn ui(view: impl View + 'static) -> Ui {
    // In a column, so that a view keeps its own height instead of taking the window's.
    let mut ui = Ui::new(VStack::new((view,)).align(ravenrs_layout::Align::Stretch), Theme::raven_dark());
    ui.set_viewport(Size::new(800.0, 600.0), 1.0);
    ui.update();
    ui.update();
    ui
}

fn key(ui: &mut Ui, key: Key) {
    ui.handle_event(&Event::KeyDown { key, modifiers: Modifiers::default() });
    ui.update();
}

fn tab(ui: &mut Ui) {
    key(ui, Key::Tab);
}

fn hover(ui: &mut Ui, at: Point) {
    ui.handle_event(&Event::PointerMove { position: at });
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

fn paths(ui: &Ui) -> Vec<Color> {
    ui.scene()
        .primitives()
        .iter()
        .filter_map(|p| match p {
            Primitive::Path(path) => match path.paint {
                ravenrs_core::PathPaint::Solid(color) => Some(color),
                _ => None,
            },
            _ => None,
        })
        .collect()
}

fn glyph_runs(ui: &Ui) -> Vec<Color> {
    ui.scene()
        .primitives()
        .iter()
        .filter_map(|p| if let Primitive::Glyphs(g) = p { Some(g.color) } else { None })
        .collect()
}

fn theme() -> Theme {
    Theme::raven_dark()
}

#[test]
fn dot_draws_its_color_and_says_what_it_means() {
    let mut ui = ui(Dot::new().tone(Tone::Danger).size(10.0).label("Not saved"));
    let dot = quads(&ui).into_iter().find(|q| q.rect.width() == 10.0).expect("the dot");
    assert_eq!(dot.fill, theme().color(Semantic::Danger).into());
    assert_eq!(dot.corner_radii, [5.0; 4], "round");
    named(&ui.access_tree(), Role::Image, "Not saved");

    let mut bare = self::ui(Dot::new().color(Color::hex(0x112233)));
    let dot = quads(&bare).into_iter().find(|q| q.rect.width() == 8.0).expect("8 by default");
    assert_eq!(dot.fill, Color::hex(0x112233).into());
    assert!(bare.access_tree().nodes.iter().all(|n| n.role != Role::Image), "only decoration");
}

#[test]
fn overlay_badge_sits_on_the_chosen_corner() {
    let content = || Canvas::new(|_, _| {}).size(100.0, 60.0);
    let badge = || Dot::new().size(10.0).label("Mark");
    let frame = |ui: &mut Ui| {
        let content_frame = Rect::new(0.0, 0.0, 100.0, 60.0);
        (named(&ui.access_tree(), Role::Image, "Mark").bounds, content_frame)
    };

    let mut top_right = ui(OverlayBadge::new(content(), badge()));
    let (dot, content_frame) = frame(&mut top_right);
    assert_eq!(dot.center(), Point::new(content_frame.max_x(), content_frame.y()), "centered on it");

    let mut bottom_left = ui(OverlayBadge::new(content(), badge()).corner(FloatingCorner::BottomLeft));
    assert_eq!(frame(&mut bottom_left).0.center(), Point::new(0.0, 60.0));

    let mut inset = ui(OverlayBadge::new(content(), badge()).inset(6.0));
    assert_eq!(frame(&mut inset).0.center(), Point::new(94.0, 6.0), "toward the middle");

    let mut hidden = ui(OverlayBadge::new(content(), badge()).visible(false));
    assert!(hidden.access_tree().find(Role::Image, "Mark").is_none());

    // A wider badge keeps its size around the corner: it is not squeezed by the box it is in.
    let mut wide = ui(OverlayBadge::new(content(), Badge::new("12 new").height(20.0)));
    let chip = quads(&wide).into_iter().find(|q| q.rect.height() == 20.0).expect("the chip");
    assert!(chip.rect.width() > 30.0, "at {}", chip.rect.width());
    assert!((chip.rect.center().x - 100.0).abs() < 1.0);
    assert!(wide.access_tree().nodes.iter().any(|n| n.name.as_deref() == Some("12 new")));
}

#[test]
fn rating_read_only_is_one_element_with_its_value_in_words() {
    let mut ui = ui(Localized::new(Labels::pt_br(), Rating::read_only(9).max(5).color(Color::WHITE)));
    let colors = paths(&ui);
    assert_eq!(colors.len(), 5);
    assert!(colors.iter().all(|c| *c == Color::WHITE), "more than the maximum lights them all");
    let tree = ui.access_tree();
    let image = named(&tree, Role::Image, "Nota");
    assert_eq!(image.value.as_deref(), Some("5 de 5"));
    assert_eq!(image.bounds.width(), 90.0, "five of 18");

    let two = self::ui(Rating::read_only(2).color(Color::WHITE));
    assert_eq!(paths(&two).iter().filter(|c| **c == Color::WHITE).count(), 2);
    let lit_again = self::ui(Rating::read_only(2).tone(Tone::Success));
    assert_eq!(paths(&lit_again)[0], theme().color(Semantic::Success));
}

#[test]
fn rating_takes_clicks_previews_and_clears() {
    let score = State::new(2u32);
    let mut ui = ui(Rating::new(score.binding()).label("Photo").clearable().color(Color::WHITE));
    let lit = |ui: &Ui| paths(ui).iter().filter(|c| **c == Color::WHITE).count();
    let bounds = named(&ui.access_tree(), Role::Slider, "Photo").bounds;
    let star = |n: f32| Point::new(bounds.x() + (n - 0.5) * 18.0, bounds.center().y);
    assert_eq!(lit(&ui), 2);

    hover(&mut ui, star(4.0));
    assert_eq!(lit(&ui), 4, "shows what a click would give");
    assert_eq!(score.get(), 2, "without giving it");
    hover(&mut ui, Point::new(700.0, 500.0));
    assert_eq!(lit(&ui), 2, "and goes back when the pointer leaves");

    click(&mut ui, star(4.0));
    assert_eq!(score.get(), 4);
    click(&mut ui, star(4.0));
    assert_eq!(score.get(), 0, "clicking the same score again takes it away");

    let kept = State::new(3u32);
    let mut plain = self::ui(Rating::new(kept.binding()).label("Kept"));
    let bounds = named(&plain.access_tree(), Role::Slider, "Kept").bounds;
    click(&mut plain, Point::new(bounds.x() + 2.5 * 18.0, bounds.center().y));
    assert_eq!(kept.get(), 3, "unless it was asked for, a score stays");
}

#[test]
fn rating_works_from_the_keyboard_and_screen_readers() {
    let score = State::new(2u32);
    let mut ui = ui(Rating::new(score.binding()).label("Photo").max(4));
    tab(&mut ui);
    key(&mut ui, Key::ArrowRight);
    assert_eq!(score.get(), 3);
    key(&mut ui, Key::ArrowLeft);
    key(&mut ui, Key::ArrowDown);
    assert_eq!(score.get(), 1);
    key(&mut ui, Key::End);
    assert_eq!(score.get(), 4);
    key(&mut ui, Key::ArrowRight);
    assert_eq!(score.get(), 4, "stops at the maximum");
    key(&mut ui, Key::Home);
    assert_eq!(score.get(), 0);
    key(&mut ui, Key::ArrowLeft);
    assert_eq!(score.get(), 0, "and at nothing");
    key(&mut ui, Key::Character("3".into()));
    assert_eq!(score.get(), 3);
    key(&mut ui, Key::Character("9".into()));
    assert_eq!(score.get(), 3, "a digit above the maximum means nothing");

    let tree = ui.access_tree();
    let slider = named(&tree, Role::Slider, "Photo");
    assert_eq!(slider.value.as_deref(), Some("3 of 4"));
    let id = slider.id;
    assert!(ui.perform_action(id, AccessAction::Increment));
    ui.update();
    assert_eq!(score.get(), 4);
}

#[test]
fn icon_tile_tints_a_square() {
    let mut ui = ui(IconTile::new(IconName::Star).tone(Tone::Success).size(40.0).label("Done"));
    let tile = quads(&ui).into_iter().find(|q| q.rect.width() == 40.0).expect("the square");
    assert_eq!(tile.rect.height(), 40.0);
    assert_eq!(tile.fill, theme().color(Semantic::Success).with_alpha(0.14).into());
    assert_eq!(glyph_runs(&ui), [theme().color(Semantic::Success)], "the icon in the same color");
    named(&ui.access_tree(), Role::Image, "Done");
}

#[test]
fn empty_state_shows_text_and_runs_its_actions() {
    let clicks = Rc::new(Cell::new(0));
    let count = clicks.clone();
    let mut ui = ui(EmptyState::new("Nothing here")
        .icon(IconName::Inbox)
        .description("Import a folder")
        .action(Button::new("Import").on_click(move || count.set(count.get() + 1)))
        .action(Button::new("Help")));
    let tree = ui.access_tree();
    let title = named(&tree, Role::Heading, "Nothing here");
    assert_eq!(title.level, Some(3));
    assert!((title.bounds.center().x - 400.0).abs() < 1.0, "centered in its area");
    assert!(tree.nodes.iter().any(|n| n.name.as_deref() == Some("Import a folder")));
    named(&tree, Role::Button, "Help");
    let import = named(&tree, Role::Button, "Import").bounds.center();
    click(&mut ui, import);
    assert_eq!(clicks.get(), 1);
    assert_eq!(glyph_runs(&ui).len(), 5, "icon, title, sentence and two buttons");
}

#[test]
fn hero_banner_fills_the_width_in_a_strong_color() {
    let mut ui = ui(HeroBanner::new("Remove the card").tone(Tone::Success).icon(IconName::CircleCheck));
    let band = quads(&ui).into_iter().find(|q| q.rect.width() == 800.0).expect("across the area");
    assert_eq!(band.fill, theme().color(Semantic::Success).into());
    named(&ui.access_tree(), Role::Status, "Remove the card");

    let mut alert = self::ui(
        HeroBanner::new("Card stuck")
            .tone(Tone::Danger)
            .description("It is in use")
            .action(Button::new("Retry")),
    );
    let tree = alert.access_tree();
    named(&tree, Role::Alert, "Card stuck");
    named(&tree, Role::Button, "Retry");

    // The ink follows the fill, so that a light color stays readable.
    let dark = self::ui(HeroBanner::new("On dark").color(Color::hex(0x101010)));
    assert_eq!(glyph_runs(&dark), [Color::WHITE]);
    let light = self::ui(HeroBanner::new("On light").color(Color::hex(0xf0f0f0)));
    assert_eq!(glyph_runs(&light), [Color::BLACK]);
}

#[test]
fn multi_progress_stacks_meters_and_survives_odd_counts() {
    let mut ui = ui(MultiProgress::new()
        .label("Import")
        .meter("Copy", 1, 4)
        .meter("Previews", 0, 0)
        .meter("Over", 9, 3)
        .bar(ProgressBar::new(0.5).label("Own")));
    let tree = ui.access_tree();
    named(&tree, Role::Group, "Import");
    let value = |name: &str| named(&tree, Role::ProgressIndicator, name).numeric.map(|n| n.value);
    assert_eq!(value("Copy"), Some(25.0));
    assert_eq!(value("Previews"), Some(0.0), "nothing to do is not a division by zero");
    assert_eq!(value("Over"), Some(100.0));
    let (copy, own) = (
        named(&tree, Role::ProgressIndicator, "Copy").bounds,
        named(&tree, Role::ProgressIndicator, "Own").bounds,
    );
    assert!(own.y() > copy.y(), "stacked");
}

#[test]
fn shimmer_text_lights_up_only_when_motion_is_allowed() {
    let lit = |ui: &Ui| {
        let ink = theme().color(Semantic::Ink);
        glyph_runs(ui).iter().filter(|c| c.r == ink.r && c.g == ink.g && c.b == ink.b).count()
    };
    let mut moving = Ui::new(ShimmerText::new("Looking for updates"), theme());
    moving.set_viewport(Size::new(800.0, 600.0), 1.0);
    moving.set_time(0.0);
    moving.update();
    moving.set_time(0.7);
    moving.update();
    assert!(moving.next_wake().is_some(), "it keeps asking for frames");
    let during = lit(&moving);
    assert!(during > 0, "the light is on some glyphs");
    moving.set_time(1.2);
    moving.update();
    let frame_later = moving.scene().primitives().to_vec();
    moving.set_time(1.5);
    moving.update();
    assert_ne!(frame_later, moving.scene().primitives().to_vec(), "and it moves");
    assert!(moving.access_tree().nodes.iter().any(|n| n.name.as_deref() == Some("Looking for updates")));

    let mut calm = Ui::new(ShimmerText::new("Looking for updates"), theme());
    calm.set_viewport(Size::new(800.0, 600.0), 1.0);
    calm.set_reduced_motion(true);
    calm.set_time(0.7);
    calm.update();
    assert_eq!(lit(&calm), 0, "plain text with reduced motion");
    assert_eq!(glyph_runs(&calm).len(), 1);
    assert!(calm.next_wake().is_none(), "and no frames");

    let mut off = Ui::new(ShimmerText::new("Done").active(false), theme());
    off.set_viewport(Size::new(800.0, 600.0), 1.0);
    off.set_time(0.7);
    off.update();
    assert_eq!(lit(&off), 0, "nor once the work has ended");
    assert!(off.next_wake().is_none());
}

#[test]
fn day_separator_names_the_day() {
    let today = Date::new(2026, 11, 4).unwrap();
    let name = |view: DaySeparator| {
        let mut ui = ui(Localized::new(Labels::pt_br(), view));
        let tree = ui.access_tree();
        tree.nodes.iter().find(|n| n.role == Role::Separator).and_then(|n| n.name.clone()).unwrap()
    };
    assert_eq!(name(DaySeparator::of(today, today)), "Hoje");
    assert_eq!(name(DaySeparator::of(today.add_days(-1), today)), "Ontem");
    assert_eq!(name(DaySeparator::of(today.add_days(-2), today)), "02/11/2026");
    assert_eq!(name(DaySeparator::new("Last week")), "Last week");
    let lines = ui(DaySeparator::new("Today"));
    let rules: Vec<_> = quads(&lines).into_iter().filter(|q| q.rect.height() == 1.0).collect();
    assert_eq!(rules.len(), 2, "a line to each side");
    assert_eq!(rules[0].rect.x(), 0.0);
    assert_eq!(rules[1].rect.max_x(), 800.0);
}

#[test]
fn shortcut_sheet_lists_groups_and_closes_with_escape() {
    let open = State::new(false);
    let mut ui = ui(Localized::new(
        Labels::pt_br(),
        ShortcutSheet::new(open.binding())
            .group("Navigate", [("←", "Previous"), ("→", "Next")])
            .group("Rate", [("1–5", "Give a score")]),
    ));
    assert!(ui.access_tree().nodes.iter().all(|n| n.role != Role::Dialog), "closed, it shows nothing");
    open.set(true);
    ui.update();
    let tree = ui.access_tree();
    named(&tree, Role::Dialog, "Atalhos de teclado");
    let (navigate, rate) = (named(&tree, Role::Heading, "Navigate"), named(&tree, Role::Heading, "Rate"));
    assert_eq!(navigate.bounds.y(), rate.bounds.y(), "groups side by side while they fit");
    assert!(rate.bounds.x() > navigate.bounds.max_x());
    let spoken = |name: &str| tree.nodes.iter().find(|n| n.name.as_deref() == Some(name)).map(|n| n.bounds);
    let (cap, does) = (spoken("←").expect("the key"), spoken("Previous").expect("what it does"));
    assert!(does.x() > cap.max_x(), "the key, then what it does");
    assert_eq!(spoken("Next").unwrap().x(), does.x(), "lined up");
    key(&mut ui, Key::Escape);
    assert!(!open.get(), "Escape closes it");
}

#[test]
fn shortcut_sheet_scrolls_a_list_that_cannot_fit() {
    let open = State::new(true);
    let rows: Vec<(String, String)> = (0..60).map(|i| (format!("F{i}"), format!("Does {i}"))).collect();
    let mut ui = ui(ShortcutSheet::new(open.binding()).title("All").group("Long", rows));
    let tree = ui.access_tree();
    assert!(named(&tree, Role::Dialog, "All").bounds.height() < 600.0, "stays in the window");
    assert!(tree.nodes.iter().any(|n| n.role == Role::ScrollView), "and scrolls");
}

#[test]
fn shortcut_hint_bar_puts_keys_before_what_they_do() {
    let mut ui = ui(ShortcutHintBar::new().leading("3 of 40").hint(["←", "→"], "move").hint(["P"], "pick"));
    let tree = ui.access_tree();
    let at = |name: &str| {
        tree.nodes.iter().find(|n| n.name.as_deref() == Some(name)).map(|n| n.bounds.x()).expect(name)
    };
    assert!(at("3 of 40") < at("←"));
    assert!(at("←") < at("→"));
    assert!(at("→") < at("move"));
    assert!(at("move") < at("P"));
    assert!(at("P") < at("pick"));
}
