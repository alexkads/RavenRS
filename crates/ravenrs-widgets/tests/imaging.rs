//! The instruments for pictures: each test fails if its feature is removed.
#![allow(clippy::expect_used, clippy::unwrap_used, reason = "test helpers fail loudly")]

use ravenrs_core::{Color, Primitive, Rect, Role, Size};
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
