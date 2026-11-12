//! Links, anchors, counters that start when seen, and strips that move with the clock.
#![allow(clippy::expect_used, reason = "test helpers fail loudly")]

use ravenrs_core::{AccessTree, Brush, Color, Event, Point, PointerButton, Primitive, Quad, Rect, Size};
use ravenrs_style::Theme;
use ravenrs_widgets::*;

fn ui_with(root: impl View + 'static, theme: Theme) -> Ui {
    let mut ui = Ui::new(root, theme);
    ui.set_viewport(Size::new(800.0, 600.0), 1.0);
    ui.set_time(0.0);
    ui.update();
    ui
}

fn ui(root: impl View + 'static) -> Ui {
    ui_with(root, Theme::raven_dark())
}

fn names(ui: &mut Ui) -> Vec<String> {
    let tree: AccessTree = ui.access_tree();
    tree.nodes.iter().filter_map(|n| n.name.clone()).collect()
}

fn bounds(ui: &mut Ui, name: &str) -> Rect {
    let tree: AccessTree = ui.access_tree();
    tree.nodes.iter().find(|n| n.name.as_deref() == Some(name)).expect(name).bounds
}

fn click(ui: &mut Ui, at: Point) {
    ui.handle_event(&Event::PointerMove { position: at });
    ui.handle_event(&Event::PointerDown { position: at, button: PointerButton::Primary });
    ui.handle_event(&Event::PointerUp { position: at, button: PointerButton::Primary });
    ui.update();
}

/// Runs the clock from `from` to `to` seconds, a frame at a time.
fn run(ui: &mut Ui, from: f64, to: f64) {
    let mut t = from;
    while t <= to {
        ui.set_time(t);
        ui.update();
        t += 1.0 / 60.0;
    }
}

fn block(height: f32) -> Canvas {
    Canvas::new(|_, _| {}).size(800.0, height)
}

#[test]
fn a_link_opens_its_address_through_the_links_service() {
    let mut ui = ui(VStack::new((Link::new("Docs", "https://example.com/docs"),)));
    let at = bounds(&mut ui, "Docs").center();
    click(&mut ui, at);
    assert_eq!(ui.links().opened(), ["https://example.com/docs"]);
}

#[test]
fn a_hash_link_glides_to_its_anchor_below_a_pinned_bar() {
    let page = ScrollView::new((
        Link::new("Go to reports", "#reports"),
        block(2000.0),
        Anchor::new("reports", Text::new("Reports title")).offset(64.0),
        block(2000.0),
    ))
    .spacing(0.0);
    let mut ui = ui(page);
    let at = bounds(&mut ui, "Go to reports").center();
    click(&mut ui, at);
    assert!(ui.links().opened().is_empty(), "anchors stay in the app");
    run(&mut ui, 0.0, 0.25);
    let midway = bounds(&mut ui, "Reports title").y();
    assert!(midway > 64.5 && midway < 2000.0, "still on its way at {midway}");
    run(&mut ui, 0.25, 1.0);
    assert!((bounds(&mut ui, "Reports title").y() - 64.0).abs() < 0.5, "lands 64 pixels below the top");
}

#[test]
fn with_reduced_motion_a_hash_link_jumps() {
    let mut theme = Theme::raven_dark();
    theme.interaction.reduced_motion = true;
    let page = ScrollView::new((
        Link::new("Go", "#end"),
        block(3000.0),
        Anchor::new("end", Text::new("The end")),
        block(1000.0),
    ))
    .spacing(0.0);
    let mut ui = ui_with(page, theme);
    let at = bounds(&mut ui, "Go").center();
    click(&mut ui, at);
    ui.update();
    assert!(bounds(&mut ui, "The end").y().abs() < 0.5);
}

#[test]
fn a_counter_waits_until_it_is_seen_and_then_counts_up() {
    let page =
        ScrollView::new((block(1200.0), CountUp::new(100_000.0).seconds(1.0), block(1200.0))).spacing(0.0);
    let mut ui = ui(page);
    run(&mut ui, 0.0, 2.0);
    assert!(names(&mut ui).iter().any(|n| n == "0"), "below the fold it has not started");
    ui.handle_event(&Event::Scroll { position: Point::new(400.0, 300.0), delta: Point::new(0.0, -1000.0) });
    run(&mut ui, 2.0, 2.3);
    let partway = names(&mut ui);
    assert!(!partway.iter().any(|n| n == "0" || n == "100,000"), "counting: {partway:?}");
    run(&mut ui, 2.3, 3.5);
    assert!(names(&mut ui).iter().any(|n| n == "100,000"));
}

/// A counter bound to a value the test changes, and what it shows.
fn counter(value: &ravenrs_reactive::State<f64>) -> impl View + 'static {
    let value = value.clone();
    view_fn(move || CountUp::new(value.get()).start_at_value().seconds(1.0).format(|v| format!("{v:.0}")))
}

fn shown(ui: &mut Ui) -> f64 {
    names(ui).iter().find_map(|n| n.parse().ok()).expect("a number")
}

#[test]
fn a_counter_counts_from_where_it_is_to_a_new_value() {
    let value = ravenrs_reactive::State::new(100.0);
    let mut ui = ui(counter(&value));
    assert_eq!(shown(&mut ui), 100.0, "it starts at its value");
    let width = bounds(&mut ui, "100").width();
    value.set(200.0);
    run(&mut ui, 0.0, 0.2);
    let early = shown(&mut ui);
    assert!(early > 140.0 && early < 200.0, "fast at first: {early}");
    // Changed again halfway: from where it is, with no jump.
    value.set(0.0);
    run(&mut ui, 0.2, 0.22);
    let turned = shown(&mut ui);
    assert!((turned - early).abs() < 15.0, "from {early}, not from 200: {turned}");
    run(&mut ui, 0.22, 1.5);
    assert_eq!(shown(&mut ui), 0.0, "and it lands on the value");
    value.set(888.0);
    run(&mut ui, 1.5, 3.0);
    assert!((bounds(&mut ui, "888").width() - width).abs() < 0.5, "digits keep one width");
}

#[test]
fn under_reduced_motion_a_counter_changes_at_once() {
    let value = ravenrs_reactive::State::new(100.0);
    let mut ui = ui(counter(&value));
    ui.set_reduced_motion(true);
    value.set(200.0);
    run(&mut ui, 0.0, 0.02);
    assert_eq!(shown(&mut ui), 200.0);
}

const MARK: Color = Color::rgb(1.0, 0.0, 0.0);

fn marker_x(ui: &Ui) -> f32 {
    ui.scene()
        .primitives()
        .iter()
        .find_map(|p| match p {
            Primitive::Quad(q) if matches!(q.fill, Brush::Solid(c) if c.r == 1.0 && c.g == 0.0) => {
                Some(q.rect.x())
            }
            _ => None,
        })
        .expect("the marker is painted")
}

fn strip() -> Ticker<Canvas> {
    Ticker::new(
        Canvas::new(|frame, scene| {
            scene.push_quad(Quad::new(Rect::new(frame.x(), frame.y(), 10.0, 10.0), MARK))
        })
        .size(300.0, 40.0),
    )
    .speed(100.0)
    .gap(20.0)
}

#[test]
fn a_ticker_moves_with_the_clock_and_wraps_after_one_copy() {
    let mut ui = ui(VStack::new((strip(),)));
    let start = marker_x(&ui);
    ui.set_time(1.0);
    ui.update();
    assert!((marker_x(&ui) - (start - 100.0)).abs() < 0.5, "100 pixels a second");
    ui.set_time(3.2 + 1.0);
    ui.update();
    // 420 pixels is one copy and its gap (300 + 20) plus 100: back where 1 second was.
    assert!((marker_x(&ui) - (start - 100.0)).abs() < 0.5, "wrapped");
}

#[test]
fn with_reduced_motion_a_ticker_stands_still() {
    let mut theme = Theme::raven_dark();
    theme.interaction.reduced_motion = true;
    let mut ui = ui_with(VStack::new((strip(),)), theme);
    let start = marker_x(&ui);
    ui.set_time(2.0);
    ui.update();
    assert_eq!(marker_x(&ui), start);
}

fn red(q: &Quad) -> bool {
    matches!(q.fill, Brush::Solid(c) if c.r > 0.9 && c.g < 0.2 && c.b < 0.2 && c.a > 0.9)
}

fn field_quads(ui: &Ui) -> Vec<Quad> {
    ui.scene()
        .primitives()
        .iter()
        .filter_map(|p| match p {
            Primitive::Quad(q) => Some(*q),
            _ => None,
        })
        .collect()
}

#[test]
fn a_quad_field_waves_as_a_floor_and_stands_up_into_its_picture() {
    let field = QuadField::new(40, 24, |_, _, _| MARK).height(600.0);
    let page = ScrollView::new((ScrollScene::new(field).length(1000.0), block(2000.0))).spacing(0.0);
    let mut ui = ui(page);
    let floor: Vec<Quad> = field_quads(&ui);
    assert!(floor.len() > 200, "the floor is drawn, {} squares", floor.len());
    assert!(
        floor.iter().filter(|q| red(q)).count() < floor.len() / 10,
        "at rest it is the floor, not the picture"
    );
    ui.set_time(1.0);
    ui.update();
    let waved = field_quads(&ui);
    assert!(
        floor.iter().zip(&waved).any(|(a, b)| (a.rect.y() - b.rect.y()).abs() > 0.5),
        "the floor waves with the clock"
    );
    ui.handle_event(&Event::Scroll { position: Point::new(400.0, 300.0), delta: Point::new(0.0, -1000.0) });
    ui.update();
    let picture = field_quads(&ui);
    let shown = picture.iter().filter(|q| red(q)).count();
    assert!(
        shown > 40 * 24 * 8 / 10,
        "scrolled through the scene, it is the picture: {shown} of {}",
        40 * 24
    );
}

#[test]
fn with_reduced_motion_the_floor_stands_still() {
    let mut theme = Theme::raven_dark();
    theme.interaction.reduced_motion = true;
    let mut ui = ui_with(VStack::new((QuadField::new(30, 18, |_, _, _| MARK).height(400.0),)), theme);
    let before = field_quads(&ui);
    ui.set_time(2.0);
    ui.update();
    let after = field_quads(&ui);
    assert_eq!(before.len(), after.len());
    assert!(before.iter().zip(&after).all(|(a, b)| a.rect == b.rect));
}
