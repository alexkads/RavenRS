//! The instruments for pictures: each test fails if its feature is removed.
#![allow(clippy::expect_used, clippy::unwrap_used, reason = "test helpers fail loudly")]

use ravenrs_core::{
    Color, Event, ImageData, Key, Modifiers, Point, PointerButton, Primitive, Rect, Role, Size,
};
use ravenrs_reactive::State;
use ravenrs_style::Theme;
use ravenrs_widgets::*;

fn ui(view: impl View + 'static) -> Ui {
    let mut ui = Ui::new(view, Theme::raven_dark());
    ui.set_viewport(Size::new(800.0, 600.0), 1.0);
    ui.update();
    ui.update();
    ui
}

fn quads(ui: &Ui) -> Vec<(Rect, Color)> {
    ui.scene()
        .primitives()
        .iter()
        .filter_map(|p| match p {
            Primitive::Quad(q) => match q.fill {
                ravenrs_core::Brush::Solid(c) => Some((q.rect, c)),
                _ => None,
            },
            _ => None,
        })
        .collect()
}

fn paths(ui: &Ui) -> usize {
    ui.scene().primitives().iter().filter(|p| matches!(p, Primitive::Path(_))).count()
}

fn glyph_runs(ui: &Ui) -> usize {
    ui.scene().primitives().iter().filter(|p| matches!(p, Primitive::Glyphs(_))).count()
}

fn press(ui: &mut Ui, key: Key, modifiers: Modifiers) {
    ui.handle_event(&Event::KeyDown { key, modifiers });
    ui.update();
    ui.update();
}

fn down(ui: &mut Ui, at: Point) {
    ui.handle_event(&Event::PointerMove { position: at });
    ui.handle_event(&Event::PointerDown { position: at, button: PointerButton::Primary });
    ui.update();
}

fn drag_to(ui: &mut Ui, at: Point) {
    ui.handle_event(&Event::PointerMove { position: at });
    ui.update();
}

fn up(ui: &mut Ui, at: Point) {
    ui.handle_event(&Event::PointerUp { position: at, button: PointerButton::Primary });
    ui.update();
    ui.update();
}

fn click(ui: &mut Ui, at: Point) {
    down(ui, at);
    up(ui, at);
}

fn double_click(ui: &mut Ui, at: Point) {
    click(ui, at);
    ui.handle_event(&Event::DoubleClick { position: at });
    ui.update();
    ui.update();
}

fn images(ui: &Ui) -> Vec<(Rect, ImageData)> {
    ui.scene()
        .primitives()
        .iter()
        .filter_map(|p| match p {
            Primitive::Image(i) => Some((i.rect, i.image.clone())),
            _ => None,
        })
        .collect()
}

const PLAIN: Modifiers = Modifiers { shift: false, ctrl: false, alt: false, meta: false };
const SHIFT: Modifiers = Modifiers { shift: true, ctrl: false, alt: false, meta: false };

// ------------------------------------------------------------------------------- Histogram

const RED: Color = Color::rgb(1.0, 0.0, 0.0);
const GREEN: Color = Color::rgb(0.0, 1.0, 0.0);

fn tallest(ui: &Ui, color: Color) -> f32 {
    quads(ui).iter().filter(|(_, c)| *c == color).map(|(r, _)| r.height()).fold(0.0, f32::max)
}

#[test]
fn histogram_adds_overlapping_channels_as_light() {
    let view = Histogram::new([
        HistogramChannel::new("Red", RED, [0.0_f32, 10.0, 10.0, 0.0]),
        HistogramChannel::new("Green", GREEN, [0.0_f32, 5.0, 5.0, 0.0]),
    ])
    .size(100.0, 100.0);
    let ui = ui(view);
    // The shorter channel is where both overlap, and shows their sum, never green alone.
    assert!(tallest(&ui, RED) > 40.0, "the part above the overlap is pure red");
    assert_eq!(tallest(&ui, GREEN), 0.0);
    let yellow = quads(&ui).iter().any(|(_, c)| c.r > 0.5 && c.g > 0.5 && c.b == 0.0);
    assert!(yellow, "the overlap is yellow");
}

#[test]
fn histogram_log_scale_raises_small_hills() {
    let bins = [0.0_f32, 1000.0, 10.0, 0.0];
    let linear = ui(Histogram::new([HistogramChannel::new("R", RED, bins)]).size(100.0, 100.0));
    let log = ui(Histogram::new([HistogramChannel::new("R", RED, bins)])
        .size(100.0, 100.0)
        .scale(HistogramScale::Logarithmic));
    let small = |ui: &Ui| {
        quads(ui).iter().filter(|(_, c)| *c == RED).map(|(r, _)| r.height()).fold(f32::MAX, f32::min)
    };
    assert!(small(&linear) < 2.0, "{}", small(&linear));
    assert!(small(&log) > 20.0, "{}", small(&log));
}

#[test]
fn histogram_marks_only_the_end_that_clips_and_says_it() {
    let mut bins = vec![100.0_f32; 16];
    bins[15] = 500.0;
    bins[0] = 0.0;
    let mut ui = ui(Histogram::new([HistogramChannel::new("Red", RED, bins)]).size(160.0, 80.0));
    assert_eq!(paths(&ui), 1, "one triangle, at the high end");
    let tree = ui.access_tree();
    let node = tree.nodes.iter().find(|n| n.role == Role::Image).unwrap();
    assert_eq!(node.name.as_deref(), Some("Histogram"));
    let value = node.value.clone().unwrap();
    assert!(value.contains("Red") && value.contains("Highlights clipped"), "{value}");
    assert!(!value.contains("Shadows clipped"), "{value}");
}

#[test]
fn histogram_end_spike_does_not_flatten_the_rest() {
    let mut bins = vec![100.0_f32; 16];
    bins[15] = 100_000.0;
    let ui = ui(Histogram::new([HistogramChannel::new("Red", RED, bins)]).size(160.0, 82.0));
    let body = quads(&ui).iter().filter(|(_, c)| *c == RED).map(|(r, _)| r.height()).fold(f32::MAX, f32::min);
    assert!(body > 70.0, "the inner bins still reach the top: {body}");
}

#[test]
fn histogram_never_draws_more_columns_than_pixels() {
    let bins = vec![10.0_f32; 4096];
    let ui = ui(Histogram::new([HistogramChannel::new("Red", RED, bins)]).size(52.0, 40.0));
    assert_eq!(quads(&ui).iter().filter(|(_, c)| *c == RED).count(), 50);
}

#[test]
fn histogram_line_channel_is_a_path_and_empty_data_draws_no_bars() {
    let ui_line = ui(Histogram::new([HistogramChannel::new("Light", RED, [1.0_f32, 2.0, 3.0]).line()])
        .size(100.0, 40.0)
        .clip_share(1.0));
    assert_eq!(paths(&ui_line), 1);
    assert_eq!(tallest(&ui_line, RED), 0.0);
    let empty = ui(Histogram::new([HistogramChannel::new("R", RED, Vec::<f32>::new())]).size(100.0, 40.0));
    assert_eq!(tallest(&empty, RED), 0.0);
}

// ----------------------------------------------------------------------------------- Ruler

fn ruler_ticks(ui: &Ui) -> Vec<Rect> {
    quads(ui).into_iter().map(|(r, _)| r).filter(|r| r.width() <= 1.0 && r.height() < 18.0 + 0.5).collect()
}

#[test]
fn ruler_step_follows_the_zoom_without_crowding() {
    for scale in [0.013_f32, 0.4, 1.0, 3.3, 27.0, 400.0] {
        let ui = ui(Ruler::horizontal().scale(scale).origin(-13.0));
        let marks = ruler_ticks(&ui);
        assert!(!marks.is_empty(), "scale {scale}");
        assert!(marks.len() <= 200, "at most one mark every 4 pixels at scale {scale}: {}", marks.len());
        // At most one number per 64 pixels of the 800 wide ruler.
        assert!(glyph_runs(&ui) <= 14, "scale {scale}: {}", glyph_runs(&ui));
        assert!(glyph_runs(&ui) >= 2, "scale {scale}: {}", glyph_runs(&ui));
    }
}

#[test]
fn ruler_origin_moves_the_marks_and_the_marker_is_in_units() {
    let at_zero = ui(Ruler::horizontal().scale(2.0).marker(100.0));
    let moved = ui(Ruler::horizontal().scale(2.0).origin(30.0).marker(100.0));
    let accent = |ui: &Ui| {
        let tree_color = Theme::raven_dark().color(ravenrs_style::Semantic::Accent);
        quads(ui).into_iter().find(|(_, c)| *c == tree_color).map(|(r, _)| r.x()).unwrap()
    };
    assert!((accent(&at_zero) - 199.5).abs() < 0.01);
    assert!((accent(&moved) - 139.5).abs() < 0.01);
}

#[test]
fn ruler_whole_units_never_divides_a_unit() {
    let divided = ui(Ruler::horizontal().scale(100.0));
    let whole = ui(Ruler::horizontal().scale(100.0).whole_units());
    assert!(ruler_ticks(&divided).len() > ruler_ticks(&whole).len() * 4);
}

#[test]
fn vertical_ruler_stacks_digits_and_says_its_marker() {
    let mut ui = ui(HStack::new((Ruler::vertical().scale(1.0).origin(100.0).marker(150.0),)).height(400.0));
    // "100", "200"... come as one glyph run per digit.
    assert!(glyph_runs(&ui) >= 3 * 3, "{}", glyph_runs(&ui));
    let tree = ui.access_tree();
    let node = tree.nodes.iter().find(|n| n.role == Role::Image).unwrap();
    assert_eq!(node.name.as_deref(), Some("Ruler"));
    assert_eq!(node.value.as_deref(), Some("150"));
}

// ------------------------------------------------------------------------------ ColorWheel

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.6
}

/// A 214 wide wheel at the top left: its disc has a radius of 100, around (107, 107).
fn wheel(start: HueSaturation) -> (Ui, State<HueSaturation>) {
    let value = State::new(start);
    let ui = ui(ColorWheel::new(value.binding()).diameter(214.0));
    (ui, value)
}

#[test]
fn wheel_puts_red_right_and_hue_counterclockwise() {
    let (mut ui, value) = wheel(HueSaturation::default());
    click(&mut ui, Point::new(157.0, 107.0));
    assert!(near(value.get().hue, 0.0) && (value.get().saturation - 0.5).abs() < 0.01, "{:?}", value.get());
    click(&mut ui, Point::new(107.0, 57.0));
    assert!(near(value.get().hue, 90.0), "up is 90 degrees: {:?}", value.get());
}

#[test]
fn wheel_drag_past_the_edge_stays_at_the_edge_and_shift_keeps_the_hue() {
    let (mut ui, value) = wheel(HueSaturation::default());
    down(&mut ui, Point::new(157.0, 107.0));
    drag_to(&mut ui, Point::new(600.0, 107.0));
    assert_eq!(value.get().saturation, 1.0);
    up(&mut ui, Point::new(600.0, 107.0));

    let (mut ui, value) = wheel(HueSaturation::new(40.0, 0.2));
    ui.handle_event(&Event::KeyDown { key: Key::Other("Shift".into()), modifiers: SHIFT });
    down(&mut ui, Point::new(107.0, 157.0));
    assert!(near(value.get().hue, 40.0), "{:?}", value.get());
    assert!((value.get().saturation - 0.5).abs() < 0.01, "{:?}", value.get());
}

#[test]
fn wheel_keys_turn_hue_and_change_saturation() {
    let (mut ui, value) = wheel(HueSaturation::new(359.5, 0.5));
    ui.handle_event(&Event::KeyDown { key: Key::Tab, modifiers: PLAIN });
    ui.update();
    press(&mut ui, Key::ArrowRight, PLAIN);
    assert!(near(value.get().hue, 0.5), "hue wraps around: {:?}", value.get());
    press(&mut ui, Key::ArrowLeft, SHIFT);
    assert!(near(value.get().hue, 350.5), "{:?}", value.get());
    press(&mut ui, Key::ArrowUp, PLAIN);
    assert!((value.get().saturation - 0.51).abs() < 0.001);
    press(&mut ui, Key::ArrowDown, SHIFT);
    assert!((value.get().saturation - 0.41).abs() < 0.001);
    press(&mut ui, Key::Home, PLAIN);
    assert_eq!(value.get().saturation, 0.0);
    assert!(near(value.get().hue, 350.5), "taking the color out keeps the hue");
}

#[test]
fn wheel_double_click_takes_the_color_out() {
    let (mut ui, value) = wheel(HueSaturation::new(10.0, 0.5));
    double_click(&mut ui, Point::new(107.0, 57.0));
    assert_eq!(value.get().saturation, 0.0);
}

#[test]
fn wheel_shares_one_disc_across_wheels_and_rebuilds() {
    let value = State::new(HueSaturation::new(0.0, 0.5));
    let view = HStack::new((
        ColorWheel::new(value.binding()).diameter(120.0),
        ColorWheel::new(value.binding()).diameter(120.0),
    ));
    let mut ui = ui(view);
    let before = images(&ui);
    assert_eq!(before.len(), 2);
    assert_eq!(before[0].1, before[1].1, "same texture for the same size");
    value.set(HueSaturation::new(200.0, 0.9));
    ui.update();
    assert_eq!(images(&ui)[0].1, before[0].1, "a new color computes no new disc");
    // The disc shows the colors: red to the right, cyan to the left, clear in the corner.
    let disc = &before[0].1;
    let (side, pixels) = (disc.width() as usize, disc.pixels());
    let at = |x: usize, y: usize| &pixels[(y * side + x) * 4..(y * side + x) * 4 + 4];
    let right = at(side - 3, side / 2);
    assert!(right[0] > 240 && right[1] < 30, "{right:?}");
    let left = at(2, side / 2);
    assert!(left[0] < 30 && left[2] > 200, "{left:?}");
    assert_eq!(at(0, 0)[3], 0);
}

#[test]
fn wheel_says_its_value_and_can_be_turned_off() {
    let (mut live, _) = wheel(HueSaturation::new(120.0, 0.25));
    let tree = live.access_tree();
    let node = tree.find(Role::Slider, "Color wheel").unwrap();
    assert_eq!(node.value.as_deref(), Some("Hue 120°, Saturation 25%"));

    let value = State::new(HueSaturation::new(10.0, 0.5));
    let mut off = ui(ColorWheel::new(value.binding()).diameter(214.0).disabled(true));
    click(&mut off, Point::new(157.0, 107.0));
    assert_eq!(value.get(), HueSaturation::new(10.0, 0.5));
}

// ----------------------------------------------------------------------------- CurveEditor

/// A plot of 100 x 100 at (7, 7), so a value `v` sits at `7 + 100 v`, and `y` goes up.
fn at(x: f32, y: f32) -> Point {
    Point::new(7.0 + x * 100.0, 107.0 - y * 100.0)
}

fn curve(start: Vec<Point>) -> (Ui, State<Vec<Point>>) {
    let points = State::new(start);
    let ui = ui(CurveEditor::new(points.binding()).size(114.0, 114.0));
    (ui, points)
}

fn line() -> Vec<Point> {
    vec![Point::ZERO, Point::new(1.0, 1.0)]
}

fn close(a: Point, x: f32, y: f32) -> bool {
    (a.x - x).abs() < 0.011 && (a.y - y).abs() < 0.011
}

#[test]
fn curve_press_on_empty_space_adds_a_point_in_order_and_drags_it() {
    let (mut ui, points) = curve(line());
    down(&mut ui, at(0.5, 0.2));
    assert_eq!(points.get().len(), 3);
    assert!(close(points.get()[1], 0.5, 0.2), "{:?}", points.get());
    drag_to(&mut ui, at(0.6, 0.9));
    up(&mut ui, at(0.6, 0.9));
    assert!(close(points.get()[1], 0.6, 0.9), "{:?}", points.get());
}

#[test]
fn curve_drag_stays_between_neighbors_and_inside_the_plot() {
    let (mut ui, points) =
        curve(vec![Point::ZERO, Point::new(0.3, 0.3), Point::new(0.6, 0.6), Point::new(1.0, 1.0)]);
    down(&mut ui, at(0.3, 0.3));
    drag_to(&mut ui, Point::new(600.0, 500.0));
    up(&mut ui, Point::new(600.0, 500.0));
    let moved = points.get()[1];
    assert!(moved.x < 0.6 && moved.x >= 0.58, "{moved:?}");
    assert_eq!(moved.y, 0.0);
}

#[test]
fn curve_double_click_removes_a_point_but_not_the_ends() {
    let (mut ui, points) = curve(vec![Point::ZERO, Point::new(0.5, 0.2), Point::new(1.0, 1.0)]);
    double_click(&mut ui, at(0.5, 0.2));
    assert_eq!(points.get(), line());
    double_click(&mut ui, at(1.0, 1.0));
    assert_eq!(points.get(), line(), "the ends stay");
}

#[test]
fn curve_keyboard_chooses_moves_adds_and_removes() {
    let (mut ui, points) = curve(vec![Point::ZERO, Point::new(0.5, 0.5), Point::new(1.0, 1.0)]);
    press(&mut ui, Key::Tab, PLAIN);
    press(&mut ui, Key::PageDown, PLAIN);
    press(&mut ui, Key::PageDown, PLAIN);
    press(&mut ui, Key::ArrowUp, PLAIN);
    assert!(close(points.get()[1], 0.5, 0.51), "{:?}", points.get());
    press(&mut ui, Key::ArrowRight, SHIFT);
    assert!(close(points.get()[1], 0.55, 0.51), "{:?}", points.get());
    press(&mut ui, Key::Enter, PLAIN);
    assert_eq!(points.get().len(), 4, "added between the chosen one and the next");
    assert!(points.get()[2].x > 0.55 && points.get()[2].x < 1.0);
    press(&mut ui, Key::Delete, PLAIN);
    assert_eq!(points.get().len(), 3);
    press(&mut ui, Key::PageUp, PLAIN);
    press(&mut ui, Key::PageUp, PLAIN);
    press(&mut ui, Key::PageUp, PLAIN);
    press(&mut ui, Key::Delete, PLAIN);
    assert_eq!(points.get().len(), 3, "the first point cannot go");
}

#[test]
fn curve_value_goes_through_the_points_and_never_swings_past_them() {
    let points = [Point::ZERO, Point::new(0.25, 0.8), Point::new(0.5, 0.8), Point::new(1.0, 1.0)];
    for p in points {
        assert!((CurveEditor::value_at(&points, p.x) - p.y).abs() < 1e-4);
    }
    let mut before = 0.0;
    for i in 0..=100 {
        let y = CurveEditor::value_at(&points, i as f32 / 100.0);
        assert!(y >= before - 1e-4, "monotone over monotone points at {i}");
        before = y;
    }
    assert!(
        (0..=100).all(|i| {
            let x = 0.25 + 0.25 * i as f32 / 100.0;
            (CurveEditor::value_at(&points, x) - 0.8).abs() < 1e-4
        }),
        "flat stays flat"
    );
    assert_eq!(CurveEditor::value_at(&[], 0.5), 0.5, "no points is a straight line");
}

#[test]
fn curve_says_the_chosen_point_and_draws_the_backdrop_and_baseline() {
    let (mut live, _) = curve(vec![Point::ZERO, Point::new(0.5, 0.25), Point::new(1.0, 1.0)]);
    let tree = live.access_tree();
    assert_eq!(tree.find(Role::Slider, "Curve").unwrap().value.as_deref(), Some("3 points"));
    down(&mut live, at(0.5, 0.25));
    up(&mut live, at(0.5, 0.25));
    let tree = live.access_tree();
    let value = tree.find(Role::Slider, "Curve").unwrap().value.clone().unwrap();
    assert_eq!(value, "Point 2/3: input 50%, output 25%");

    let points = State::new(line());
    let plain = ui(CurveEditor::new(points.binding()).size(114.0, 114.0).baseline(false));
    let with_line = ui(CurveEditor::new(points.binding()).size(114.0, 114.0));
    assert_eq!(paths(&with_line), paths(&plain) + 1);
    let backed = ui(CurveEditor::new(points.binding())
        .size(114.0, 114.0)
        .histogram([HistogramChannel::new("R", RED, [1.0_f32, 2.0, 1.0])], HistogramScale::Linear));
    let backdrop = quads(&backed).iter().filter(|(_, c)| c.r == 1.0 && c.g == 0.0 && c.a < 1.0).count();
    assert!(backdrop >= 3, "{backdrop}");
}

#[test]
fn curve_disabled_ignores_input() {
    let points = State::new(line());
    let mut off = ui(CurveEditor::new(points.binding()).size(114.0, 114.0).disabled(true));
    click(&mut off, at(0.5, 0.2));
    assert_eq!(points.get(), line());
}

// ----------------------------------------------------------------------------- ZoomControl

fn center_of(ui: &mut Ui, role: Role, name: &str) -> Point {
    let tree = ui.access_tree();
    tree.find(role, name).unwrap_or_else(|| panic!("no {name}")).bounds.center()
}

fn zoom_control(start: ZoomView) -> (Ui, State<ZoomView>) {
    let view = State::new(start);
    let ui = ui(ZoomControl::new(view.binding()));
    (ui, view)
}

#[test]
fn zoom_control_steps_through_round_stops_and_stops_at_its_range() {
    let (mut ui, view) = zoom_control(ZoomView::at(1.0));
    let more = center_of(&mut ui, Role::Button, "Zoom in");
    click(&mut ui, more);
    assert_eq!(view.get().scale, 1.5);
    click(&mut ui, more);
    assert_eq!(view.get().scale, 2.0);
    let less = center_of(&mut ui, Role::Button, "Zoom out");
    click(&mut ui, less);
    click(&mut ui, less);
    click(&mut ui, less);
    assert_eq!(view.get().scale, 0.67);

    let view = State::new(ZoomView::at(1.9));
    let mut near_top = self::ui(ZoomControl::new(view.binding()).range(0.5, 2.0));
    let more = center_of(&mut near_top, Role::Button, "Zoom in");
    click(&mut near_top, more);
    assert_eq!(view.get().scale, 2.0);
    let tree = near_top.access_tree();
    assert!(tree.find(Role::Button, "Zoom in").unwrap().disabled, "nothing above the range");
}

#[test]
fn zoom_control_fits_and_shows_actual_size_and_says_the_percentage() {
    let (mut ui, view) = zoom_control(ZoomView { scale: 0.125, ..ZoomView::at(0.125) });
    let tree = ui.access_tree();
    assert!(tree.nodes.iter().any(|n| n.name.as_deref() == Some("12.5%")), "the percentage is shown");
    assert!(tree.find(Role::Group, "Zoom").is_some());
    let fit = center_of(&mut ui, Role::Button, "Fit");
    click(&mut ui, fit);
    assert!(view.get().fit);
    let actual = center_of(&mut ui, Role::Button, "Actual size");
    click(&mut ui, actual);
    assert_eq!((view.get().fit, view.get().scale), (false, 1.0));
    let tree = ui.access_tree();
    assert!(tree.nodes.iter().any(|n| n.name.as_deref() == Some("100%")));
    assert_eq!(tree.find(Role::Button, "Fit").unwrap().toggled, Some(ravenrs_core::Toggled::False));
}

#[test]
fn zoom_control_words_follow_the_language() {
    let view = State::new(ZoomView::at(0.125));
    let mut ui = self::ui(Localized::new(Labels::pt_br(), ZoomControl::new(view.binding())));
    let tree = ui.access_tree();
    assert!(tree.find(Role::Button, "Encaixar").is_some());
    assert!(tree.find(Role::Button, "Tamanho real").is_some());
    assert!(tree.nodes.iter().any(|n| n.name.as_deref() == Some("12,5%")), "decimal comma");
}

// ----------------------------------------------------------------------------- ZoomPanView

/// 1000 x 500 of content in a 400 x 200 view at the top left: fit is 40%.
const CONTENT: Size = Size::new(1000.0, 500.0);

fn pan_view(start: ZoomView) -> (Ui, State<ZoomView>) {
    let view = State::new(start);
    let ui = ui(ZoomPanView::custom(view.binding(), CONTENT, |_, _| {}).size(400.0, 200.0));
    (ui, view)
}

fn pixel_image(side: u32) -> ImageData {
    ImageData::from_fn(side, side, |_, _| Color::rgb(0.5, 0.5, 0.5)).unwrap()
}

fn same(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.002
}

#[test]
fn zoom_pan_fit_follows_the_view_and_writes_scale_and_size() {
    let (mut ui, view) = pan_view(ZoomView::fit());
    ui.update();
    assert!(same(view.get().scale, 0.4), "{:?}", view.get());
    assert_eq!(view.get().viewport, Size::new(400.0, 200.0));
    assert_eq!(view.get().center, Point::new(0.5, 0.5));
}

#[test]
fn zoom_pan_drag_moves_content_and_stops_at_the_limits() {
    let (mut ui, view) = pan_view(ZoomView::at(1.0));
    ui.update();
    down(&mut ui, Point::new(200.0, 100.0));
    drag_to(&mut ui, Point::new(100.0, 100.0));
    assert!(same(view.get().center.x, 0.6), "dragging left shows what is to the right: {:?}", view.get());
    drag_to(&mut ui, Point::new(-3000.0, -3000.0));
    up(&mut ui, Point::new(-3000.0, -3000.0));
    let v = view.get();
    assert!(same(v.center.x, 0.8) && same(v.center.y, 0.8), "the far corner is the limit: {v:?}");
    assert!(same(v.visible(CONTENT).max_x(), 1.0));
}

#[test]
fn zoom_pan_cannot_move_content_that_fits() {
    let (mut ui, view) = pan_view(ZoomView::fit());
    ui.update();
    down(&mut ui, Point::new(200.0, 100.0));
    drag_to(&mut ui, Point::new(50.0, 20.0));
    up(&mut ui, Point::new(50.0, 20.0));
    assert_eq!(view.get().center, Point::new(0.5, 0.5));
    assert!(view.get().fit);
}

#[test]
fn zoom_pan_pinch_keeps_the_point_under_the_pointer() {
    let (mut ui, view) = pan_view(ZoomView::at(1.0));
    ui.update();
    let pointer = Point::new(300.0, 50.0);
    ui.handle_event(&Event::PointerMove { position: pointer });
    let before = view.get().placed(CONTENT);
    let fraction = ((pointer.x - before.x()) / before.width(), (pointer.y - before.y()) / before.height());
    ui.handle_event(&Event::Pinch { position: pointer, delta: 0.5 });
    ui.update();
    let v = view.get();
    assert!(same(v.scale, 1.5), "{v:?}");
    let after = v.placed(CONTENT);
    assert!(((pointer.x - after.x()) / after.width() - fraction.0).abs() < 0.002);
    assert!(((pointer.y - after.y()) / after.height() - fraction.1).abs() < 0.002);
    // Down to the scale that shows everything and no further, which is fit again.
    ui.handle_event(&Event::Pinch { position: pointer, delta: -0.95 });
    ui.handle_event(&Event::Pinch { position: pointer, delta: -0.95 });
    ui.update();
    assert!(same(view.get().scale, 0.4) && view.get().fit, "{:?}", view.get());
}

#[test]
fn zoom_pan_wheel_pans_and_knows_where_the_limits_are() {
    let (mut ui, view) = pan_view(ZoomView::at(1.0));
    ui.update();
    ui.update();
    let inside = Point::new(200.0, 100.0);
    let moved = ui.handle_event(&Event::Scroll { position: inside, delta: Point::new(-100.0, 50.0) });
    assert!(moved);
    ui.update();
    ui.update();
    let v = view.get();
    assert!(same(v.center.x, 0.6), "{v:?}");
    assert!(same(v.center.y, 0.4), "{v:?}");
    // A change from elsewhere moves what the wheel starts from.
    view.set(ZoomView { center: Point::new(0.2, 0.5), ..view.get() });
    ui.update();
    ui.update();
    assert!(same(view.get().center.x, 0.2), "{:?}", view.get());
    ui.handle_event(&Event::Scroll { position: inside, delta: Point::new(-100.0, 0.0) });
    ui.update();
    ui.update();
    assert!(same(view.get().center.x, 0.3), "{:?}", view.get());
    // Fitted, there is nothing to scroll and the wheel goes on to whatever is behind.
    let (mut fitted, _) = pan_view(ZoomView::fit());
    fitted.update();
    fitted.update();
    assert!(!fitted.handle_event(&Event::Scroll { position: inside, delta: Point::new(0.0, -40.0) }));
}

#[test]
fn zoom_pan_double_click_goes_between_fit_and_actual_size() {
    let (mut ui, view) = pan_view(ZoomView::fit());
    ui.update();
    double_click(&mut ui, Point::new(100.0, 50.0));
    assert!(!view.get().fit && same(view.get().scale, 1.0), "{:?}", view.get());
    assert!(view.get().center.x < 0.5, "toward where it was clicked: {:?}", view.get());
    double_click(&mut ui, Point::new(100.0, 50.0));
    assert!(view.get().fit && same(view.get().scale, 0.4), "{:?}", view.get());
}

#[test]
fn zoom_pan_keys_zoom_reset_and_move() {
    let (mut ui, view) = pan_view(ZoomView::at(1.0));
    ui.update();
    press(&mut ui, Key::Tab, PLAIN);
    press(&mut ui, Key::Character("+".into()), PLAIN);
    assert!(same(view.get().scale, 1.5));
    press(&mut ui, Key::Character("-".into()), PLAIN);
    press(&mut ui, Key::Character("-".into()), PLAIN);
    assert!(same(view.get().scale, 0.67));
    press(&mut ui, Key::Character("1".into()), PLAIN);
    assert!(same(view.get().scale, 1.0));
    press(&mut ui, Key::ArrowRight, PLAIN);
    assert!(same(view.get().center.x, 0.54), "{:?}", view.get());
    press(&mut ui, Key::ArrowDown, SHIFT);
    assert!(same(view.get().center.y, 0.7), "{:?}", view.get());
    press(&mut ui, Key::Character("0".into()), PLAIN);
    assert!(view.get().fit && same(view.get().center.x, 0.5));
}

#[test]
fn zoom_pan_range_limits_the_scale_but_fit_stays_reachable() {
    let view = State::new(ZoomView::at(50.0));
    let mut ui =
        ui(ZoomPanView::custom(view.binding(), CONTENT, |_, _| {}).size(400.0, 200.0).range(0.8, 3.0));
    ui.update();
    assert_eq!(view.get().scale, 3.0);
    view.set(ZoomView::at(0.1));
    ui.update();
    ui.update();
    assert!(same(view.get().scale, 0.4), "the range gives way to what shows everything: {:?}", view.get());
}

/// A throw: three quick steps of a drag, a frame apart, and the release.
fn throw(ui: &mut Ui) {
    let mut time = 10.0;
    ui.set_time(time);
    down(ui, Point::new(300.0, 100.0));
    ui.update();
    for x in [280.0, 260.0, 240.0, 220.0] {
        time += 1.0 / 60.0;
        ui.set_time(time);
        drag_to(ui, Point::new(x, 100.0));
    }
    up(ui, Point::new(220.0, 100.0));
}

#[test]
fn zoom_pan_throw_coasts_then_stops_asking_for_frames() {
    let (mut ui, view) = pan_view(ZoomView::at(1.0));
    ui.update();
    throw(&mut ui);
    let released = view.get().center.x;
    let mut time = 10.1;
    ui.set_time(time);
    ui.update();
    let early = view.get().center.x;
    assert!(early > released, "it goes on after the release: {released} {early}");
    let mut last = early;
    let mut frames = 0;
    while ui.next_wake().is_some() && frames < 2000 {
        time += 1.0 / 60.0;
        ui.set_time(time);
        ui.update();
        assert!(view.get().center.x >= last - 1e-5, "it never comes back");
        last = view.get().center.x;
        frames += 1;
    }
    assert!(frames > 5 && frames < 400, "it stops in a sensible time: {frames}");
    assert!(last > early, "{early} {last}");
    assert!(last <= 0.8 + 1e-4, "within the limit: {last}");
    // One more build brings the scroll offset up to date.
    ui.update();
    assert_eq!(ui.next_wake(), None, "and asks for no more frames");
    assert!(!ui.is_animating());
    assert!(!ui.needs_redraw());
}

#[test]
fn zoom_pan_slow_release_does_not_coast() {
    let (mut ui2, view2) = pan_view(ZoomView::at(1.0));
    ui2.update();
    ui2.set_time(10.0);
    down(&mut ui2, Point::new(300.0, 100.0));
    ui2.update();
    ui2.set_time(10.02);
    drag_to(&mut ui2, Point::new(299.0, 100.0));
    for i in 1..10 {
        ui2.set_time(10.02 + f64::from(i) * 0.1);
        ui2.update();
    }
    up(&mut ui2, Point::new(299.0, 100.0));
    let stopped = view2.get().center.x;
    ui2.set_time(12.0);
    ui2.update();
    assert_eq!(view2.get().center.x, stopped);
    assert_eq!(ui2.next_wake(), None);
}

#[test]
fn zoom_pan_reduced_motion_and_inertia_off_do_not_coast() {
    let (mut reduced, view) = pan_view(ZoomView::at(1.0));
    reduced.set_reduced_motion(true);
    reduced.update();
    throw(&mut reduced);
    let stopped = view.get().center.x;
    assert!(stopped > 0.5, "the drag itself still works");
    reduced.set_time(10.2);
    reduced.update();
    assert_eq!(view.get().center.x, stopped);
    assert_eq!(reduced.next_wake(), None);

    let view = State::new(ZoomView::at(1.0));
    let mut off =
        ui(ZoomPanView::custom(view.binding(), CONTENT, |_, _| {}).size(400.0, 200.0).inertia(false));
    off.update();
    throw(&mut off);
    let stopped = view.get().center.x;
    off.set_time(10.2);
    off.update();
    assert_eq!(view.get().center.x, stopped);
    assert_eq!(off.next_wake(), None);
}

#[test]
fn zoom_pan_gesture_needs_no_build_unless_someone_reads_the_view() {
    let (mut ui, view) = pan_view(ZoomView::at(1.0));
    ui.update();
    down(&mut ui, Point::new(300.0, 100.0));
    ui.update();
    let builds = ui.stats().builds;
    for x in [280.0, 260.0, 240.0] {
        ui.set_time(1.0);
        drag_to(&mut ui, Point::new(x, 100.0));
    }
    assert_eq!(ui.stats().builds, builds, "the drag only paints");
    assert!(view.get().center.x > 0.5);

    // Someone else reading the view keeps following it.
    let view = State::new(ZoomView::at(1.0));
    let shown = view.clone();
    let mut watched = self::ui(VStack::new((
        ZoomPanView::custom(view.binding(), CONTENT, |_, _| {}).size(400.0, 200.0),
        Text::bound(&shown, |v| format!("{:.2}", v.center.x)),
    )));
    watched.update();
    down(&mut watched, Point::new(300.0, 100.0));
    watched.update();
    drag_to(&mut watched, Point::new(200.0, 100.0));
    watched.update();
    let tree = watched.access_tree();
    assert!(tree.nodes.iter().any(|n| n.name.as_deref() == Some("0.60")), "the text followed");
}

#[test]
fn zoom_pan_image_draws_only_the_part_in_view() {
    let view = State::new(ZoomView::at(2.0).centered_on(Point::new(0.25, 0.25)));
    let mut ui = ui(ZoomPanView::image(view.binding(), pixel_image(400)).size(200.0, 100.0));
    ui.update();
    let image = ui
        .scene()
        .primitives()
        .iter()
        .find_map(|p| if let Primitive::Image(i) = p { Some(i.clone()) } else { None })
        .unwrap();
    assert_eq!(image.rect, Rect::new(0.0, 0.0, 200.0, 100.0));
    assert!((image.uv[0] - 0.125).abs() < 0.001, "{:?}", image.uv);
    assert!((image.uv[2] - 0.375).abs() < 0.001, "{:?}", image.uv);
    assert!((image.uv[1] - 0.1875).abs() < 0.001, "{:?}", image.uv);
    assert!((image.uv[3] - 0.3125).abs() < 0.001, "{:?}", image.uv);
}

#[test]
fn zoom_pan_custom_content_gets_the_place_and_is_clipped() {
    use std::cell::Cell;
    use std::rc::Rc;
    let seen = Rc::new(Cell::new(None));
    let view = State::new(ZoomView::at(2.0).centered_on(Point::new(0.0, 0.0)));
    let told = seen.clone();
    let mut ui = ui(ZoomPanView::custom(view.binding(), CONTENT, move |paint, _| told.set(Some(paint)))
        .size(400.0, 200.0));
    ui.update();
    let paint: ZoomPanPaint = seen.get().unwrap();
    assert_eq!(paint.frame, Rect::new(0.0, 0.0, 400.0, 200.0));
    assert_eq!(paint.scale, 2.0);
    assert_eq!(paint.placed.size, Size::new(2000.0, 1000.0));
    assert_eq!(paint.placed.origin, Point::new(0.0, 0.0), "the limit brought it back to the corner");
    assert!(ui.scene().primitives().iter().any(|p| matches!(p, Primitive::PushClip(_))));
}

// --------------------------------------------------------------------------------- Minimap

/// A 160 x 80 map of the same content next to a view that shares its [`ZoomView`].
fn map_and_stage(start: ZoomView) -> (Ui, State<ZoomView>) {
    let view = State::new(start);
    let thumbnail = pixel_image(16);
    let ui = ui(VStack::new((
        Minimap::image(view.binding(), thumbnail).content_size(CONTENT),
        ZoomPanView::custom(view.binding(), CONTENT, |_, _| {}).size(400.0, 200.0),
    )));
    (ui, view)
}

#[test]
fn minimap_shows_the_visible_part_and_shares_the_view() {
    let (mut ui, _view) = map_and_stage(ZoomView::at(1.0));
    ui.update();
    let accent = Theme::raven_dark().color(ravenrs_style::Semantic::Accent);
    let outline = || -> Vec<Rect> {
        ui.scene()
            .primitives()
            .iter()
            .filter_map(|p| match p {
                Primitive::Quad(q) if q.border.is_some_and(|b| b.color == accent) => Some(q.rect),
                _ => None,
            })
            .collect()
    };
    let shown = outline();
    assert_eq!(shown.len(), 1);
    // 400 of 1000 wide and 200 of 500 tall, in a map of 160 x 80.
    assert!((shown[0].width() - 64.0).abs() < 0.5 && (shown[0].height() - 32.0).abs() < 0.5, "{shown:?}");
    assert!((shown[0].x() - 48.0).abs() < 0.5, "{shown:?}");
}

#[test]
fn minimap_press_and_drag_move_the_view_within_the_limits() {
    let (mut ui, view) = map_and_stage(ZoomView::at(1.0));
    ui.update();
    down(&mut ui, Point::new(120.0, 40.0));
    assert!(same(view.get().center.x, 0.75), "{:?}", view.get());
    drag_to(&mut ui, Point::new(2000.0, 2000.0));
    up(&mut ui, Point::new(2000.0, 2000.0));
    assert!(same(view.get().center.x, 0.8) && same(view.get().center.y, 0.8), "{:?}", view.get());
}

#[test]
fn minimap_keys_move_and_it_says_where_it_is() {
    let (mut ui, view) = map_and_stage(ZoomView::at(1.0));
    ui.update();
    press(&mut ui, Key::Tab, PLAIN);
    press(&mut ui, Key::ArrowRight, PLAIN);
    assert!(same(view.get().center.x, 0.55), "{:?}", view.get());
    press(&mut ui, Key::ArrowDown, SHIFT);
    assert!(same(view.get().center.y, 0.75), "{:?}", view.get());
    let tree = ui.access_tree();
    let value = tree.find(Role::Slider, "Navigator").unwrap().value.clone().unwrap();
    assert_eq!(value, "100% · 55%, 75%");
}

#[test]
fn minimap_press_while_fitted_does_nothing_unless_it_can_zoom() {
    let (mut ui, view) = map_and_stage(ZoomView::fit());
    ui.update();
    click(&mut ui, Point::new(120.0, 40.0));
    assert!(view.get().fit);

    let view = State::new(ZoomView::fit());
    let mut zooming = self::ui(VStack::new((
        Minimap::image(view.binding(), pixel_image(16)).content_size(CONTENT).press_zoom(2.0),
        ZoomPanView::custom(view.binding(), CONTENT, |_, _| {}).size(400.0, 200.0),
    )));
    zooming.update();
    click(&mut zooming, Point::new(120.0, 40.0));
    let v = view.get();
    assert!(!v.fit && same(v.scale, 2.0), "{v:?}");
    assert!(v.center.x > 0.5, "{v:?}");
}
