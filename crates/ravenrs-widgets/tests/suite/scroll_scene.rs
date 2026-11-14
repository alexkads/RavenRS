//! Scroll-driven pages: sticky content, scenes held for a length of scrolling, and
//! effects linked to the scroll position.
#![allow(clippy::expect_used, reason = "test helpers fail loudly")]

use std::cell::Cell;
use std::rc::Rc;

use ravenrs_core::{
    AccessTree, Brush, Color, Effect, Event, Point, PointerButton, Primitive, Quad, Rect, Size,
};
use ravenrs_style::Theme;
use ravenrs_widgets::*;

const MARK: Color = Color::rgb(1.0, 0.0, 0.0);

fn ui_with(root: impl View + 'static, theme: Theme) -> Ui {
    let mut ui = Ui::new(root, theme);
    ui.set_viewport(Size::new(800.0, 600.0), 1.0);
    ui.update();
    ui
}

fn ui(root: impl View + 'static) -> Ui {
    ui_with(root, Theme::raven_dark())
}

fn wheel(ui: &mut Ui, dy: f32) {
    ui.handle_event(&Event::Scroll { position: Point::new(400.0, 300.0), delta: Point::new(0.0, dy) });
    ui.update();
}

fn bounds(ui: &mut Ui, name: &str) -> Rect {
    let tree: AccessTree = ui.access_tree();
    tree.nodes.iter().find(|n| n.name.as_deref() == Some(name)).expect(name).bounds
}

fn block(height: f32) -> Canvas {
    Canvas::new(|_, _| {}).size(800.0, height)
}

/// A 100 by 100 square painted in [`MARK`], to find in the scene.
fn marker() -> Canvas {
    Canvas::new(|frame, scene| scene.push_quad(Quad::new(frame, MARK))).size(100.0, 100.0)
}

fn marker_quad(ui: &Ui) -> Quad {
    ui.scene()
        .primitives()
        .iter()
        .find_map(|p| match p {
            Primitive::Quad(q) if matches!(q.fill, Brush::Solid(c) if c.r == 1.0 && c.g == 0.0 && c.b == 0.0) => Some(*q),
            _ => None,
        })
        .expect("the marker is painted")
}

fn opacity(q: &Quad) -> f32 {
    match q.fill {
        Brush::Solid(c) => c.a,
        _ => panic!("the marker is a solid fill"),
    }
}

#[test]
fn sticky_content_stays_below_the_top_and_leaves_with_its_section() {
    let clicks = Rc::new(Cell::new(0));
    let counted = clicks.clone();
    let section = VStack::new((
        Sticky::new(Button::new("Bar").on_click(move || counted.set(counted.get() + 1))).top(10.0),
        block(1500.0),
    ))
    .spacing(0.0);
    let mut ui = ui(ScrollView::new((block(300.0), section, block(1500.0))).spacing(0.0));
    assert_eq!(bounds(&mut ui, "Bar").y(), 300.0, "in place before its section reaches the top");

    wheel(&mut ui, -500.0);
    let held = bounds(&mut ui, "Bar");
    assert_eq!(held.y(), 10.0, "held 10 pixels below the top");

    let center = held.center();
    ui.handle_event(&Event::PointerMove { position: center });
    ui.handle_event(&Event::PointerDown { position: center, button: PointerButton::Primary });
    ui.handle_event(&Event::PointerUp { position: center, button: PointerButton::Primary });
    ui.update();
    assert_eq!(clicks.get(), 1, "a click lands where the pinned bar shows");

    wheel(&mut ui, -1300.0);
    let gone = bounds(&mut ui, "Bar");
    assert!(gone.y() < 10.0, "taken away by the bottom of its section, at {gone:?}");
}

#[test]
fn a_scene_holds_its_content_and_links_follow_its_length() {
    let link = ScrollLink::custom(|t| Effect::moved(0.0, 100.0 * t));
    let scene = ScrollScene::new(ScrollLinked::new(marker(), link)).length(1000.0);
    let mut ui = ui(ScrollView::new((scene, block(2000.0))).spacing(0.0));
    assert_eq!(marker_quad(&ui).rect.y(), 0.0, "progress 0 when the scene pins");

    wheel(&mut ui, -500.0);
    assert!((marker_quad(&ui).rect.y() - 50.0).abs() < 0.5, "pinned, then moved by half the effect");

    wheel(&mut ui, -500.0);
    assert!((marker_quad(&ui).rect.y() - 100.0).abs() < 0.5, "the end of the scene is the end of the effect");

    wheel(&mut ui, -50.0);
    assert!(
        (marker_quad(&ui).rect.y() - 50.0).abs() < 0.5,
        "let go: it scrolls away holding its last effect"
    );
}

#[test]
fn outside_a_scene_a_link_follows_the_pass_through_the_view() {
    let fade =
        ScrollLink::keyframes([(0.0, Effect::hidden()), (1.0, Effect::IDENTITY)]).range(ScrollRange::Through);
    let mut ui =
        ui(ScrollView::new((block(700.0), ScrollLinked::new(marker(), fade), block(2000.0))).spacing(0.0));
    wheel(&mut ui, -400.0);
    // The square's top is at 300: (600 - 300) / (600 + 100) of the way through.
    let q = marker_quad(&ui);
    assert!((opacity(&q) - 300.0 / 700.0).abs() < 1e-3, "opacity {}", opacity(&q));
}

#[test]
fn with_reduced_motion_links_fade_but_never_move() {
    let mut theme = Theme::raven_dark();
    theme.interaction.reduced_motion = true;
    let link = ScrollLink::custom(|t| Effect::moved(0.0, 100.0 * t).with_opacity(1.0 - t * 0.5));
    let scene = ScrollScene::new(ScrollLinked::new(marker(), link)).length(1000.0);
    let mut ui = ui_with(ScrollView::new((scene, block(2000.0))).spacing(0.0), theme);
    wheel(&mut ui, -500.0);
    let q = marker_quad(&ui);
    assert_eq!(q.rect.y(), 0.0, "held in place");
    assert!((opacity(&q) - 0.75).abs() < 1e-3, "still fades, opacity {}", opacity(&q));
}

#[test]
fn scrolling_a_linked_scene_rebuilds_nothing() {
    let builds = Rc::new(Cell::new(0));
    let counted = builds.clone();
    let root = view_fn(move || {
        counted.set(counted.get() + 1);
        ScrollView::new((scene_like(), block(2000.0))).spacing(0.0)
    });
    let mut ui = ui(root);
    let before = builds.get();
    for _ in 0..10 {
        wheel(&mut ui, -80.0);
    }
    assert_eq!(builds.get(), before, "scrolling only places and paints");
}

fn scene_like() -> impl View {
    ScrollScene::new(ScrollLinked::new(marker(), ScrollLink::fade_out())).length(1000.0)
}

/// A scroll sideways, as a trackpad gives it.
fn swipe(ui: &mut Ui, dx: f32) {
    ui.handle_event(&Event::Scroll { position: Point::new(400.0, 300.0), delta: Point::new(dx, 0.0) });
    ui.update();
}

fn column(width: f32) -> Canvas {
    Canvas::new(|_, _| {}).size(width, 100.0)
}

#[test]
fn sticky_content_stays_at_the_left_of_a_sideways_scroll_and_leaves_with_its_row() {
    // #124: a row of cards that scrolls sideways keeps its title on screen.
    let row = HStack::new((Sticky::new(Button::new("Title")).left(10.0), column(1500.0))).spacing(0.0);
    let content = HStack::new((column(300.0), row, column(1500.0))).spacing(0.0);
    let mut ui = ui(ScrollView::new((content,)).horizontal().spacing(0.0));
    let at_first = bounds(&mut ui, "Title");
    assert_eq!(at_first.x(), 300.0, "in place before its row reaches the left");

    swipe(&mut ui, -500.0);
    let held = bounds(&mut ui, "Title");
    assert_eq!(held.x(), 10.0, "held 10 pixels from the left");
    assert_eq!(held.y(), at_first.y(), "and it does not move down");

    swipe(&mut ui, -1500.0);
    let gone = bounds(&mut ui, "Title");
    assert!(gone.x() < 10.0, "taken away by the right of its row, at {gone:?}");
}

#[test]
fn a_sideways_link_follows_the_pass_across_the_view() {
    let fade = ScrollLink::keyframes([(0.0, Effect::hidden()), (1.0, Effect::IDENTITY)])
        .range(ScrollRange::Through)
        .sideways();
    let content =
        HStack::new((column(700.0), ScrollLinked::new(marker(), fade), column(2000.0))).spacing(0.0);
    let mut ui = ui(ScrollView::new((content,)).horizontal().spacing(0.0));
    let coming = opacity(&marker_quad(&ui));
    assert!(coming > 0.0 && coming < 0.2, "just in from the right: {coming}");
    swipe(&mut ui, -300.0);
    let further = opacity(&marker_quad(&ui));
    assert!(
        (further - (coming + 300.0 / 900.0)).abs() < 0.02,
        "a third of the way further: {coming} {further}"
    );
}
