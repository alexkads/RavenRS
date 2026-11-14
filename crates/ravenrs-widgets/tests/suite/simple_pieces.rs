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
fn hero_banner_text_reads_on_every_tone_of_every_theme() {
    // A theme by tokens whose accent is a light teal, with dark text on it.
    let vigil = Theme::from_tokens_json(
        r##"{"base": "Raven Dark", "name": "Vigil", "color": {"accent": "#5fc4c9", "on-accent": "#08262a"}}"##,
    )
    .expect("reads");
    let themes = Theme::builtin().into_iter().chain([vigil]);
    for theme in themes {
        for tone in [Tone::Accent, Tone::Neutral, Tone::Success, Tone::Warning, Tone::Danger] {
            let mut ui = Ui::new(HeroBanner::new("Ready").tone(tone), theme);
            ui.set_viewport(Size::new(800.0, 600.0), 1.0);
            ui.update();
            let fill = tone.color(&theme).with_alpha(1.0);
            let ink = glyph_runs(&ui)[0];
            if tone == Tone::Accent {
                assert_eq!(ink, theme.palette.on_accent, "{}: the accent's own text", theme.name);
            } else {
                let ratio = ink.contrast_ratio(fill);
                assert!(ratio >= 4.5, "{} {tone:?}: {ratio:.2}:1", theme.name);
            }
        }
    }
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

/// A box of a known size, named for finding it in the tree.
fn tile(name: &'static str, width: f32, height: f32) -> impl View {
    struct Tile(&'static str, f32, f32);
    impl View for Tile {
        fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
            let node = cx.container(
                ravenrs_layout::LayoutStyle {
                    width: Some(self.1),
                    height: Some(self.2),
                    ..ravenrs_layout::LayoutStyle::leaf()
                },
                &[],
                Paint::None,
            );
            cx.semantics(node, Semantics::new(Role::Image).name(self.0));
            node
        }
    }
    Tile(name, width, height)
}

#[test]
fn z_stack_overlaps_and_takes_the_size_of_the_largest() {
    let below = || tile("Below", 20.0, 20.0);
    let view = ZStack::new((tile("Back", 100.0, 40.0), tile("Front", 30.0, 90.0)));
    let row = HStack::new((view, tile("Beside", 10.0, 10.0))).spacing(0.0);
    let mut ui = ui(VStack::new((row, below())).spacing(0.0));
    let tree = ui.access_tree();
    let bounds = |name: &str| named(&tree, Role::Image, name).bounds;
    assert_eq!(bounds("Front").origin, bounds("Back").origin, "in the same place");
    assert_eq!(bounds("Beside").x(), 100.0, "as wide as the widest");
    assert_eq!(bounds("Below").y(), 90.0, "as tall as the tallest");
    let order: Vec<_> = tree.nodes.iter().filter_map(|n| n.name.clone()).collect();
    assert!(order.iter().position(|n| n == "Back") < order.iter().position(|n| n == "Front"));
}

#[test]
fn z_stack_follows_its_content_when_it_shrinks() {
    let tall = State::new(true);
    let source = tall.clone();
    let mut ui = ui(VStack::new((
        ZStack::new((view_fn(move || tile("Layer", 50.0, if source.get() { 120.0 } else { 30.0 })),)),
        tile("Below", 20.0, 20.0),
    ))
    .spacing(0.0));
    let below = |ui: &mut Ui| named(&ui.access_tree(), Role::Image, "Below").bounds.y();
    assert_eq!(below(&mut ui), 120.0);
    tall.set(false);
    ui.update();
    assert_eq!(below(&mut ui), 30.0, "no taller than what it has now");
}

#[test]
fn z_stack_places_the_smaller_layers() {
    let mut ui = ui(ZStack::new((tile("Back", 100.0, 80.0), tile("Front", 20.0, 20.0)))
        .align(ravenrs_layout::Align::Center));
    let tree = ui.access_tree();
    let back = named(&tree, Role::Image, "Back").bounds;
    let front = named(&tree, Role::Image, "Front").bounds;
    assert_eq!(front.y(), back.y() + 30.0, "centered down");
    assert_eq!(front.center().x, 400.0, "and across its width");
}

#[test]
fn z_stack_top_layer_takes_the_click() {
    let (back, front) = (Rc::new(Cell::new(0)), Rc::new(Cell::new(0)));
    let (b, f) = (back.clone(), front.clone());
    let mut ui = ui(ZStack::new((
        Button::new("Back").on_click(move || b.set(b.get() + 1)),
        Button::new("Front").on_click(move || f.set(f.get() + 1)),
    )));
    let at = named(&ui.access_tree(), Role::Button, "Front").bounds.center();
    click(&mut ui, at);
    assert_eq!((back.get(), front.get()), (0, 1));
}

#[test]
fn property_row_shows_label_and_value_at_the_ends() {
    let mut ui = ui(VStack::new((
        PropertyRow::new("Camera", "NIKON"),
        PropertyRow::new("Lens", "  "),
        PropertyRow::new("Note", "word ".repeat(400)),
        PropertyRow::with("Status", Button::new("Edit")),
    ))
    .align(ravenrs_layout::Align::Stretch));
    let tree = ui.access_tree();
    let camera = named(&tree, Role::Label, "Camera");
    assert_eq!(camera.value.as_deref(), Some("NIKON"));
    assert_eq!(camera.bounds.width(), 800.0, "across its parent");
    assert_eq!(named(&tree, Role::Label, "Lens").value.as_deref(), Some("—"), "a dash for nothing");
    assert_eq!(named(&tree, Role::Button, "Edit").bounds.max_x(), 800.0, "the value on the right");
    let note = named(&tree, Role::Label, "Note");
    assert_eq!(note.bounds.width(), 800.0, "a long value does not widen the row");
    assert_eq!(note.bounds.height(), camera.bounds.height(), "nor wrap");
    let runs = glyph_runs(&ui);
    assert!(
        runs.contains(&theme().color(Semantic::InkMuted)) && runs.contains(&theme().color(Semantic::Ink))
    );
}

#[test]
fn page_header_holds_title_description_badges_and_actions() {
    let clicks = Rc::new(Cell::new(0));
    let count = clicks.clone();
    let mut ui = ui(PageHeader::new("Sessions")
        .description("Every shoot")
        .badge(Badge::new("12"))
        .action(Button::new("New").on_click(move || count.set(count.get() + 1))));
    let tree = ui.access_tree();
    let title = named(&tree, Role::Heading, "Sessions");
    assert_eq!(title.level, Some(1));
    let line = tree.nodes.iter().find(|n| n.name.as_deref() == Some("Every shoot")).expect("the line");
    assert!(line.bounds.y() >= title.bounds.max_y(), "under the title");
    let new = named(&tree, Role::Button, "New").bounds;
    assert_eq!(new.max_x(), 800.0, "actions at the end");
    click(&mut ui, new.center());
    assert_eq!(clicks.get(), 1);

    let mut compact = self::ui(PageHeader::new("Tools").description("Inline").compact());
    let tree = compact.access_tree();
    let title = named(&tree, Role::Heading, "Tools");
    let line = tree.nodes.iter().find(|n| n.name.as_deref() == Some("Inline")).expect("the line");
    assert!(line.bounds.x() >= title.bounds.max_x(), "beside the title");
}

#[test]
fn button_group_glues_its_buttons_and_keeps_them_working() {
    let picked = Rc::new(Cell::new(""));
    let on = |name: &'static str| {
        let picked = picked.clone();
        move || picked.set(name)
    };
    let mut ui = ui(ButtonGroup::new([
        Button::new("Day").style(ButtonStyle::Secondary).on_click(on("day")),
        Button::new("Week").style(ButtonStyle::Secondary).on_click(on("week")),
        Button::new("Month").style(ButtonStyle::Secondary).on_click(on("month")),
    ])
    .label("Period"));
    let tree = ui.access_tree();
    named(&tree, Role::Group, "Period");
    let (day, week, month) = (
        named(&tree, Role::Button, "Day").bounds,
        named(&tree, Role::Button, "Week").bounds,
        named(&tree, Role::Button, "Month").bounds,
    );
    assert_eq!(day.max_x(), week.x(), "no gap");
    assert_eq!(week.max_x(), month.x(), "no gap");
    let radius = theme().radii.md;
    let shape = |at: Rect| {
        quads(&ui)
            .into_iter()
            .find(|q| q.rect.max_x() == at.max_x() && q.rect.y() == at.y())
            .expect("a button")
    };
    assert_eq!(shape(day).corner_radii, [radius, 0.0, 0.0, radius], "outer corners only");
    assert_eq!(shape(week).corner_radii, [0.0; 4]);
    assert_eq!(shape(week).rect.x(), week.x() - 1.0, "over the line of the one before");
    assert_eq!(shape(month).corner_radii, [0.0, radius, radius, 0.0]);

    click(&mut ui, week.center());
    assert_eq!(picked.get(), "week");
    tab(&mut ui);
    key(&mut ui, Key::Enter);
    assert_eq!(picked.get(), "month", "from the keyboard too");

    let alone = self::ui(ButtonGroup::new([Button::new("Only")]));
    let quad = quads(&alone).into_iter().next().expect("the button");
    assert_eq!(quad.corner_radii, [theme().radii.md; 4], "one button is simply a button");
}

#[test]
fn favorite_toggle_flips_and_fills() {
    let on = State::new(false);
    let mut ui = ui(FavoriteToggle::new(on.binding()).label("Portrait").color(Color::WHITE));
    let tree = ui.access_tree();
    let node = named(&tree, Role::Button, "Portrait");
    assert_eq!(node.toggled, Some(ravenrs_core::Toggled::False));
    let at = node.bounds.center();
    assert!(paths(&ui).is_empty(), "an outline when off");
    click(&mut ui, at);
    assert!(on.get());
    assert_eq!(paths(&ui), [Color::WHITE], "filled when on: not only a change of color");
    assert_eq!(named(&ui.access_tree(), Role::Button, "Portrait").toggled, Some(ravenrs_core::Toggled::True));
    key(&mut ui, Key::Space);
    assert!(!on.get(), "from the keyboard, focused by the click");

    let mut named_by_labels =
        self::ui(Localized::new(Labels::pt_br(), FavoriteToggle::new(on.binding()).star()));
    named(&named_by_labels.access_tree(), Role::Button, "Favorito");
}

#[test]
fn swatch_grid_tells_the_handler_the_color_and_the_modifier() {
    let picked = Rc::new(Cell::new(None));
    let sink = picked.clone();
    let (rose, blue, green) = (Color::hex(0xe11d48), Color::hex(0x2563eb), Color::hex(0x16a34a));
    let mut ui = ui(SwatchGrid::new([(rose, "Rose"), (blue, "Blue"), (green, "Green")])
        .columns(2)
        .selected(Some(blue))
        .on_pick(move |color, modifiers| sink.set(Some((color, modifiers.meta)))));
    let tree = ui.access_tree();
    named(&tree, Role::Group, "Swatches");
    assert_eq!(named(&tree, Role::Button, "Blue").selected, Some(true));
    assert_eq!(named(&tree, Role::Button, "Rose").selected, Some(false));
    let (rose_at, blue_at, green_at) = (
        named(&tree, Role::Button, "Rose").bounds,
        named(&tree, Role::Button, "Blue").bounds,
        named(&tree, Role::Button, "Green").bounds,
    );
    assert_eq!(rose_at.y(), blue_at.y(), "two to a row");
    assert_eq!(green_at.x(), rose_at.x(), "and the third below");
    assert_eq!(green_at.y(), rose_at.max_y());

    click(&mut ui, blue_at.center());
    assert_eq!(picked.get(), Some((blue, false)));

    let meta = Modifiers { meta: true, ..Modifiers::default() };
    ui.handle_event(&Event::KeyUp { key: Key::Tab, modifiers: meta });
    click(&mut ui, rose_at.center());
    assert_eq!(picked.get(), Some((rose, true)), "the modifier reaches the handler");
    ui.handle_event(&Event::KeyUp { key: Key::Tab, modifiers: Modifiers::default() });

    // Rose has focus. Down goes to the row below, and Right from there has nowhere to go.
    key(&mut ui, Key::ArrowDown);
    key(&mut ui, Key::Enter);
    assert_eq!(picked.get(), Some((green, false)));
    key(&mut ui, Key::ArrowRight);
    key(&mut ui, Key::Enter);
    assert_eq!(picked.get(), Some((green, false)), "stays at the end");
    key(&mut ui, Key::ArrowUp);
    key(&mut ui, Key::ArrowRight);
    key(&mut ui, Key::Space);
    assert_eq!(picked.get(), Some((blue, false)));
}

#[test]
fn nine_point_anchor_takes_clicks_and_arrows() {
    let point = State::new(AnchorPoint::Center);
    let mut ui = ui(Localized::new(Labels::pt_br(), NinePointAnchor::new(point.binding())));
    let tree = ui.access_tree();
    let group = named(&tree, Role::RadioGroup, "Ponto de referência");
    assert_eq!(group.value.as_deref(), Some("Centro"));
    assert_eq!(group.bounds.width(), 32.0, "three squares of 10 and two gaps");
    assert_eq!(named(&tree, Role::RadioButton, "Centro").selected, Some(true));
    let at = named(&tree, Role::RadioButton, "Inferior direito").bounds.center();
    click(&mut ui, at);
    assert_eq!(point.get(), AnchorPoint::BottomRight);

    point.set(AnchorPoint::Center);
    ui.update();
    tab(&mut ui);
    key(&mut ui, Key::ArrowLeft);
    assert_eq!(point.get(), AnchorPoint::Left);
    key(&mut ui, Key::ArrowLeft);
    assert_eq!(point.get(), AnchorPoint::Left, "stops at the edge");
    key(&mut ui, Key::ArrowUp);
    assert_eq!(point.get(), AnchorPoint::TopLeft);
    key(&mut ui, Key::ArrowUp);
    assert_eq!(point.get(), AnchorPoint::TopLeft);
    key(&mut ui, Key::ArrowRight);
    key(&mut ui, Key::ArrowRight);
    assert_eq!(point.get(), AnchorPoint::TopRight);
    key(&mut ui, Key::ArrowRight);
    assert_eq!(point.get(), AnchorPoint::TopRight, "does not wrap to the next row");
    key(&mut ui, Key::ArrowDown);
    assert_eq!(point.get(), AnchorPoint::Right);
    assert_eq!(
        named(&ui.access_tree(), Role::RadioGroup, "Ponto de referência").value.as_deref(),
        Some("Direita")
    );
}

#[test]
fn anchor_point_converts_to_and_from_shares_of_a_box() {
    for point in AnchorPoint::ALL {
        let (x, y) = point.fraction();
        assert_eq!(AnchorPoint::from_fraction(x, y), point);
    }
    assert_eq!(AnchorPoint::TopLeft.fraction(), (0.0, 0.0));
    assert_eq!(AnchorPoint::Bottom.fraction(), (0.5, 1.0));
    assert_eq!(AnchorPoint::from_fraction(-3.0, 9.0), AnchorPoint::BottomLeft, "out of range is clamped");
}

#[test]
fn color_pair_swaps_resets_and_reports_clicks() {
    let (black, white, red) = (Color::hex(0x000000), Color::hex(0xffffff), Color::hex(0xff0000));
    let (front, back) = (State::new(red), State::new(white));
    let clicks = Rc::new(Cell::new((0, 0)));
    let (a, b) = (clicks.clone(), clicks.clone());
    let mut ui = ui(ColorPair::new(front.binding(), back.binding())
        .defaults(black, white)
        .on_foreground(move || a.set((a.get().0 + 1, a.get().1)))
        .on_background(move || b.set((b.get().0, b.get().1 + 1))));
    let tree = ui.access_tree();
    let upper = named(&tree, Role::Button, "Foreground");
    let lower = named(&tree, Role::Button, "Background");
    assert_eq!(upper.value.as_deref(), Some("#FF0000"), "in words, not only in color");
    assert_eq!(lower.value.as_deref(), Some("#FFFFFF"));
    assert_eq!(lower.bounds.x(), upper.bounds.x() + 12.0, "the background shows past the foreground");
    let (over_both, only_back) =
        (upper.bounds.center(), Point::new(lower.bounds.max_x() - 2.0, lower.bounds.max_y() - 2.0));
    let swap = named(&tree, Role::Button, "Swap foreground and background").bounds.center();
    let reset = named(&tree, Role::Button, "Default colors").bounds.center();

    click(&mut ui, over_both);
    assert_eq!(clicks.get(), (1, 0), "the foreground is on top");
    click(&mut ui, only_back);
    assert_eq!(clicks.get(), (1, 1));

    click(&mut ui, swap);
    assert_eq!((front.get(), back.get()), (white, red));
    assert_eq!(named(&ui.access_tree(), Role::Button, "Foreground").value.as_deref(), Some("#FFFFFF"));
    click(&mut ui, reset);
    assert_eq!((front.get(), back.get()), (black, white));

    let mut plain = self::ui(ColorPair::new(front.binding(), back.binding()));
    let tree = plain.access_tree();
    named(&tree, Role::Image, "Foreground");
    assert!(tree.find(Role::Button, "Default colors").is_none(), "no default colors, no button");
}
