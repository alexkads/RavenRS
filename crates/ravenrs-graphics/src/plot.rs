//! The charts of a dashboard: the same numbers as [`crate::chart`], laid out for a
//! screen. Besides what to draw, it says where each thing landed, so that whoever
//! shows it can tell what is under the pointer without asking for another picture.
//!
//! [`crate::chart`] stays as it is, so documents already printed keep their looks.

use std::f32::consts::{FRAC_PI_2, TAU};
use std::rc::Rc;

use ravenrs_core::{Color, LinearGradient, Point, Rect};

use crate::chart::{
    ChartData, ChartDrawing, ChartKind, ChartLabel, ChartOptions, ChartStyle, LabelAlign, arc, nice_ticks,
};
use crate::vector::{self, Layer, PathSegment, Shape};

/// How a line goes from one point to the next.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum Curve {
    /// A smooth curve that may swing past the points.
    #[default]
    Natural,
    /// A smooth curve that never swings past the points, so it invents no dips.
    Monotone,
    /// Straight lines.
    Linear,
    /// Level until halfway, then a jump.
    Step,
}

/// How series share the same category.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum Stack {
    /// Side by side, or over each other.
    #[default]
    None,
    /// Each on top of the one before.
    Normal,
    /// On top of each other, and always adding up to the whole height.
    Expand,
}

/// Which gridlines a chart has, as they look on the screen.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum Grid {
    /// Level lines.
    #[default]
    Horizontal,
    /// Upright lines.
    Vertical,
    /// Both.
    Both,
    /// No lines.
    None,
}

/// The shape of the grid of a radar or radial chart.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum PolarGrid {
    /// Polygons through the spokes.
    #[default]
    Polygon,
    /// Circles.
    Circle,
    /// Nothing behind the data.
    None,
}

/// Where a label sits relative to what it names.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum LabelAt {
    /// Just outside it: above a point or a bar, around a slice.
    #[default]
    Outside,
    /// In its middle.
    Inside,
    /// Inside, where it starts.
    InsideStart,
}

/// The look of a chart. Start from [`PlotStyle::new`] and change what differs.
#[allow(missing_debug_implementations, reason = "holds closures")]
#[derive(Clone)]
#[non_exhaustive]
pub struct PlotStyle {
    /// The kind of chart.
    pub kind: ChartKind,
    /// Series colors, or slice and ring colors, repeated as needed.
    pub colors: Vec<Color>,
    /// Colors per category, over the colors of the series.
    pub item_colors: Vec<Color>,
    /// Text that matters, such as the number in the center.
    pub ink: Color,
    /// Text around the data.
    pub muted: Color,
    /// Gridlines.
    pub grid: Color,
    /// What is behind the chart. Slices are kept apart by a real gap, so it no longer
    /// paints anything.
    pub surface: Color,
    /// The track behind a ring.
    pub track: Color,
    /// The font size of the text around the data.
    pub size: f32,
    /// How lines go from point to point.
    pub curve: Curve,
    /// How series share a category.
    pub stack: Stack,
    /// Fills areas from strong at the top to faint at the bottom, and bars from strong
    /// at the tip to fainter at the base.
    pub gradient: bool,
    /// How solid an area or a radar polygon is.
    pub fill_opacity: f32,
    /// Bars lie down, with their categories on the left.
    pub horizontal: bool,
    /// The value axis runs the other way: bars hang from the top, or if they lie down
    /// they grow to the left from the right.
    pub reversed: bool,
    /// The shortest a bar with a value gets, however small the value is.
    pub min_bar: f32,
    /// The categories of bars sit on the line of zero, on its other side from the bar,
    /// instead of along the edge of the chart.
    pub labels_at_zero: bool,
    /// The corners of a bar.
    pub bar_radius: f32,
    /// The widest a bar gets.
    pub max_bar: f32,
    /// The color of a bar below zero.
    pub negative: Option<Color>,
    /// The color of a candle that closed at or above where it opened.
    pub rising: Color,
    /// The color of a candle that closed below where it opened.
    pub falling: Color,
    /// The category standing out: a slice comes forward, a bar gets an outline.
    pub active: Option<usize>,
    /// Draws a dot at each point of a line or a radar.
    pub dots: bool,
    /// Where the values are written, if at all.
    pub value_labels: Option<LabelAt>,
    /// Where the categories are written beside the data, if at all.
    pub category_labels: Option<LabelAt>,
    /// How numbers are written.
    pub format: Rc<dyn Fn(f64) -> String>,
    /// How categories are written on the axis.
    pub tick_format: Rc<dyn Fn(&str) -> String>,
    /// Gridlines of a chart with axes.
    pub grid_lines: Grid,
    /// Writes the categories along the axis.
    pub category_axis: bool,
    /// Writes numbers along the axis.
    pub value_axis: bool,
    /// The scale fits the values instead of starting from zero.
    pub tight_scale: bool,
    /// The gap between slices; zero for none.
    pub separator: f32,
    /// How far each slice reaches, in order, as a share of the room from the hole to
    /// the edge. One with no number here reaches the edge.
    pub slice_radii: Vec<f32>,
    /// Labels outside a round chart sit in a column on each side, none over another,
    /// each joined to its slice by a line with an elbow.
    pub label_guides: bool,
    /// The hole of a doughnut or of rings, as a share of the radius.
    pub hole: f32,
    /// One ring per series, instead of only the first series.
    pub concentric: bool,
    /// The grid of a radar or radial chart.
    pub polar_grid: PolarGrid,
    /// Fills the grid of a radar chart with the first color.
    pub polar_fill: bool,
    /// Lines from the center of a radar chart to each category.
    pub spokes: bool,
    /// How many rings the grid of a radar chart has.
    pub rings: usize,
    /// Radar polygons with no fill.
    pub lines_only: bool,
    /// Writes numbers along the radius of a radar chart.
    pub radius_axis: bool,
    /// Where rings start, in degrees clockwise from the top.
    pub start_angle: f32,
    /// How many degrees the largest ring covers.
    pub sweep: f32,
    /// Draws the track behind each ring.
    pub tracks: bool,
    /// A big number and a caption in the middle of a doughnut or of rings.
    pub center: Option<(String, String)>,
    /// Leaves gridlines out of [`Plot::layers`] and says where they go in
    /// [`Plot::rules`], for a screen that can line them up with its pixels and fade
    /// the data without them. Pages leave this off and get everything in the layers.
    pub rules_apart: bool,
    /// Leaves bars out of [`Plot::layers`] and says where they go in [`Plot::bars`],
    /// for a screen that draws them itself at no cost when the size changes. The
    /// gridlines they cross are left out from behind them, so that a bar that fades
    /// shows no line through it.
    pub bars_apart: bool,
    /// Gridlines in dashes.
    pub grid_dashed: bool,
    /// About how many steps the value axis has; round numbers decide how many exactly.
    pub value_ticks: usize,
    /// Upright gridlines evenly spread into this many columns, instead of at each
    /// category or number, and whatever [`Self::grid_lines`] says of the others.
    pub grid_columns: Option<usize>,
    /// The first category along the axis starts at its point and the last one ends
    /// at its point, so that they line up with the ends of a line or an area.
    pub ends_aligned: bool,
    /// Lines across the data at values that matter, such as a target.
    pub references: Vec<Reference>,
    /// The value axis goes exactly from the first number to the second, and what is
    /// past them is cut off, instead of the axis fitting the values.
    pub domain: Option<(f64, f64)>,
}

/// A dashed line across a chart at a value that matters.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct Reference {
    /// Where it goes.
    pub value: f64,
    /// What it says, at its end.
    pub label: Option<String>,
    /// Its color; [`PlotStyle::muted`] if it has none.
    pub color: Option<Color>,
}

impl Reference {
    /// A line at this value.
    pub fn new(value: f64, label: Option<String>, color: Option<Color>) -> Self {
        Self { value, label, color }
    }
}

impl PlotStyle {
    /// The look most charts of this kind want.
    pub fn new(kind: ChartKind) -> Self {
        Self {
            kind,
            colors: crate::chart::PALETTE.to_vec(),
            item_colors: Vec::new(),
            ink: Color::BLACK,
            muted: Color::rgba(0.35, 0.39, 0.46, 1.0),
            grid: Color::rgba(0.85, 0.87, 0.9, 1.0),
            surface: Color::WHITE,
            track: Color::rgba(0.92, 0.93, 0.95, 1.0),
            size: 12.0,
            curve: Curve::Natural,
            stack: Stack::None,
            gradient: false,
            fill_opacity: if kind == ChartKind::Radar { 0.6 } else { 0.4 },
            horizontal: false,
            reversed: false,
            min_bar: 0.5,
            labels_at_zero: false,
            bar_radius: 6.0,
            max_bar: 96.0,
            negative: None,
            rising: crate::chart::PALETTE[1],
            falling: crate::chart::PALETTE[3],
            active: None,
            dots: false,
            value_labels: None,
            category_labels: None,
            format: Rc::new(|value| format!("{value:.0}")),
            tick_format: Rc::new(str::to_owned),
            grid_lines: Grid::Horizontal,
            category_axis: true,
            value_axis: false,
            tight_scale: false,
            separator: 2.0,
            slice_radii: Vec::new(),
            label_guides: false,
            hole: if kind == ChartKind::Doughnut { 0.6 } else { 0.0 },
            concentric: false,
            polar_grid: if kind == ChartKind::RadialBar { PolarGrid::None } else { PolarGrid::Polygon },
            polar_fill: false,
            spokes: true,
            rings: 5,
            lines_only: false,
            radius_axis: false,
            start_angle: 0.0,
            sweep: 360.0,
            tracks: false,
            center: None,
            rules_apart: false,
            bars_apart: false,
            grid_dashed: false,
            value_ticks: TICKS,
            grid_columns: None,
            ends_aligned: false,
            references: Vec::new(),
            domain: None,
        }
    }

    fn color_of(&self, series: usize) -> Color {
        if self.colors.is_empty() { Color::BLACK } else { self.colors[series % self.colors.len()] }
    }

    /// The color of a category of a series: its own, if it has one.
    fn item_color(&self, series_color: Color, index: usize) -> Color {
        self.item_colors.get(index).copied().unwrap_or(series_color)
    }
}

/// A category of a chart with axes, and the strip of the chart that belongs to it.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct Spot {
    /// Which category.
    pub index: usize,
    /// The strip the pointer is over when it means this category.
    pub band: Rect,
    /// The middle of the strip's data: where a cursor goes.
    pub center: Point,
}

/// A value's place on the chart: where a dot goes, or an active marker.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct Mark {
    /// Which series.
    pub series: usize,
    /// Which category.
    pub index: usize,
    /// Where it landed.
    pub at: Point,
    /// Its color.
    pub color: Color,
}

/// A slice of a circle or of a ring, for telling what is under the pointer.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct Sector {
    /// Which series.
    pub series: usize,
    /// Which category.
    pub index: usize,
    /// The center of the circle.
    pub center: Point,
    /// Where the slice starts, from the center.
    pub inner: f32,
    /// Where the slice ends, from the center.
    pub outer: f32,
    /// Its first angle, in radians clockwise from the right.
    pub from: f32,
    /// Its last angle.
    pub to: f32,
    /// Its color.
    pub color: Color,
}

impl Sector {
    fn contains(&self, p: Point) -> bool {
        let (dx, dy) = (p.x - self.center.x, p.y - self.center.y);
        let distance = (dx * dx + dy * dy).sqrt();
        if distance < self.inner || distance > self.outer {
            return false;
        }
        let past_start = (dy.atan2(dx) - self.from).rem_euclid(TAU);
        past_start <= self.to - self.from
    }
}

/// A bar as it landed, for drawing it again over the picture.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct Bar {
    /// Which series.
    pub series: usize,
    /// Which category.
    pub index: usize,
    /// Where it is.
    pub rect: Rect,
    /// Its color.
    pub color: Color,
    /// Its corners: top left, top right, bottom right, bottom left.
    pub radii: [f32; 4],
    /// What fills it in place of its color, from its tip to its base.
    pub gradient: Option<LinearGradient>,
}

/// What a price did in a category of a [`ChartKind::Candlestick`].
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct Candle {
    /// Which category.
    pub index: usize,
    /// From where it opened to where it closed.
    pub body: Rect,
    /// From its body up to its high, and down to its low.
    pub wicks: [Rect; 2],
    /// Its color.
    pub color: Color,
    /// It closed at or above where it opened: its body is an outline, so that this
    /// does not rest on the color alone.
    pub rising: bool,
}

/// A thin straight line left out of the picture.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct Rule {
    /// Where it is.
    pub rect: Rect,
    /// Its color.
    pub color: Color,
}

/// What is under a point of the chart.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Hit {
    /// Which category.
    pub index: usize,
    /// Which series, when the point means only one.
    pub series: Option<usize>,
}

/// A chart laid out.
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct Plot {
    /// What to draw, back to front.
    pub layers: Vec<Layer>,
    /// The text, to draw over the layers.
    pub labels: Vec<ChartLabel>,
    /// The categories of a chart with axes.
    pub spots: Vec<Spot>,
    /// Where each value landed.
    pub marks: Vec<Mark>,
    /// The slices of a round chart.
    pub sectors: Vec<Sector>,
    /// Where the data is, inside the axes.
    pub area: Rect,
    /// The bars of a bar chart.
    pub bars: Vec<Bar>,
    /// The candles of a [`ChartKind::Candlestick`], when [`PlotStyle::bars_apart`] leaves
    /// them out of the layers.
    pub candles: Vec<Candle>,
    /// Gridlines, behind the layers, when [`PlotStyle::rules_apart`] asks for them.
    pub rules: Vec<Rule>,
    /// Lines of reference, over the layers, when [`PlotStyle::rules_apart`] asks for them.
    pub top_rules: Vec<Rule>,
}

impl From<ChartDrawing> for Plot {
    fn from(drawing: ChartDrawing) -> Self {
        Self {
            layers: drawing.shapes.into_iter().map(Layer::Shape).collect(),
            labels: drawing.labels,
            ..Self::default()
        }
    }
}

impl Plot {
    /// What is under `point`, in the coordinates the chart was laid out in.
    pub fn hit(&self, point: Point) -> Option<Hit> {
        if let Some(sector) = self.sectors.iter().find(|s| s.contains(point)) {
            return Some(Hit { index: sector.index, series: Some(sector.series) });
        }
        self.spots.iter().find(|s| s.band.contains(point)).map(|s| Hit { index: s.index, series: None })
    }

    /// Whether `point` is over the slice of this category, once it has grown by `by`.
    pub fn over_grown_slice(&self, point: Point, index: usize, by: f32) -> bool {
        self.sectors
            .iter()
            .filter(|s| s.index == index)
            .any(|s| Sector { outer: s.outer + by, ..*s }.contains(point))
    }

    /// Whether this category has any place on the chart, and how many there are.
    pub fn categories(&self) -> usize {
        let spots = self.spots.iter().map(|s| s.index + 1).max().unwrap_or(0);
        spots.max(self.sectors.iter().map(|s| s.index + 1).max().unwrap_or(0))
    }

    /// Where a tooltip about a category should point at.
    pub fn anchor_of(&self, index: usize) -> Option<Point> {
        if let Some(spot) = self.spots.iter().find(|s| s.index == index) {
            return Some(spot.center);
        }
        let sector = self.sectors.iter().find(|s| s.index == index)?;
        let middle = (sector.from + sector.to) / 2.0;
        let radius = (sector.inner + sector.outer) / 2.0;
        Some(Point::new(sector.center.x + radius * middle.cos(), sector.center.y + radius * middle.sin()))
    }
}

/// Roughly how wide a character is, as a share of the font size.
const CHAR_WIDTH: f32 = 0.56;
const TICKS: usize = 4;
/// Room on the sides of a line, so that dots and labels at its ends stay in.
const LINE_MARGIN: f32 = 12.0;
const TICK_MARGIN: f32 = 8.0;
const GRID_DASH: f32 = 4.0;
const GRID_DASH_GAP: f32 = 3.0;
const BAR_GAP: f32 = 4.0;
/// How many series a candlestick chart needs: open, high, low and close.
const CANDLE_SERIES: usize = 4;
/// The share of its category a candle's body takes.
const CANDLE_WIDTH: f32 = 0.6;
/// How solid the base of a bar in a gradient is.
const BAR_GRADIENT_BASE: f32 = 0.55;
/// The share of a category left between its bars and the next category's.
const CATEGORY_GAP: f32 = 0.2;
const DOT_RADIUS: f32 = 4.0;
const ACTIVE_SLICE_GROWTH: f32 = 10.0;
const OUTSIDE_LINE: f32 = 10.0;
/// How far a guide goes out of its slice before it turns.
const GUIDE_OUT: f32 = 12.0;
/// How far it then goes along, to its label.
const GUIDE_RUN: f32 = 14.0;
const GUIDE_GAP: f32 = 4.0;
/// The least a slice reaches, or it would not be seen.
const SLICE_RADIUS_MIN: f32 = 0.1;
/// A slice smaller than this share of the circle has no label of its own.
const SLICE_LABEL_MIN: f64 = 0.04;
const RING_GAP: f32 = 4.0;
const CENTER_VALUE_SCALE: f32 = 2.4;

/// How wide text is at a font size, for whoever knows its font.
pub type Measure<'a> = &'a dyn Fn(&str, f32) -> f32;

/// How wide labels are: measured when there is a font, or else estimated, as a page
/// that has not chosen its font yet does.
#[derive(Clone, Copy)]
struct Ruler<'a>(Option<Measure<'a>>);

impl Ruler<'_> {
    fn width(&self, content: &str, size: f32) -> f32 {
        match self.0 {
            Some(measure) => measure(content, size),
            None => content.chars().count() as f32 * size * CHAR_WIDTH,
        }
    }

    fn longest<'t>(&self, texts: impl Iterator<Item = &'t str>, size: f32) -> f32 {
        texts.map(|t| self.width(t, size)).fold(0.0, f32::max)
    }
}

/// Lays a chart out inside `rect`.
pub fn draw(data: &ChartData, rect: Rect, style: &PlotStyle) -> Plot {
    layout(data, rect, style, Ruler(None))
}

/// Lays a chart out inside `rect`, knowing how wide its labels really are.
pub fn draw_measured(data: &ChartData, rect: Rect, style: &PlotStyle, measure: Measure<'_>) -> Plot {
    layout(data, rect, style, Ruler(Some(measure)))
}

/// Lays out a scatter chart or a sparkline just as [`crate::chart`] does, and says
/// where each value landed: each one owns the strip from halfway to its neighbors.
pub fn draw_small(data: &ChartData, rect: Rect, options: &ChartOptions<'_>, style: &ChartStyle<'_>) -> Plot {
    let mut places = Vec::new();
    let drawing = crate::chart::draw_placed(data, rect, options, style, &mut places);
    let mut plot = Plot::from(drawing);
    plot.marks =
        places.iter().map(|p| Mark { series: p.series, index: p.index, at: p.at, color: p.color }).collect();
    let (top, bottom) =
        places.iter().fold((f32::INFINITY, f32::NEG_INFINITY), |(a, b), p| (a.min(p.at.y), b.max(p.at.y)));
    plot.area = if top <= bottom { Rect::new(rect.x(), top, rect.width(), bottom - top) } else { rect };
    let mut columns: Vec<(usize, f32)> = Vec::new();
    for place in &places {
        if !columns.iter().any(|(index, _)| *index == place.index) {
            columns.push((place.index, place.at.x));
        }
    }
    columns.sort_by(|a, b| a.1.total_cmp(&b.1));
    for (i, (index, x)) in columns.iter().enumerate() {
        let from = if i == 0 { rect.x() } else { (columns[i - 1].1 + x) / 2.0 };
        let to = columns.get(i + 1).map_or(rect.max_x(), |next| (next.1 + x) / 2.0);
        plot.spots.push(Spot {
            index: *index,
            band: Rect::new(from, rect.y(), to - from, rect.height()),
            center: Point::new(*x, plot.area.center().y),
        });
    }
    plot
}

fn layout(data: &ChartData, rect: Rect, style: &PlotStyle, ruler: Ruler<'_>) -> Plot {
    let mut plot = Plot { area: rect, ..Plot::default() };
    if rect.width() <= 0.0 || rect.height() <= 0.0 {
        return plot;
    }
    match style.kind {
        ChartKind::Pie | ChartKind::Doughnut => pie(data, rect, style, ruler, &mut plot),
        ChartKind::Radar => radar(data, rect, style, &mut plot),
        ChartKind::RadialBar => radial(data, rect, style, &mut plot),
        _ => cartesian(data, rect, style, ruler, &mut plot),
    }
    plot
}

fn text(content: impl Into<String>, rect: Rect, align: LabelAlign, size: f32, color: Color) -> ChartLabel {
    ChartLabel { text: content.into(), rect, align, size, bold: false, color }
}

fn centered(content: impl Into<String>, at: Point, size: f32, color: Color) -> ChartLabel {
    let content = content.into();
    let width = (content.chars().count() as f32 * size * CHAR_WIDTH + 8.0) * 2.0;
    text(
        content,
        Rect::new(at.x - width / 2.0, at.y - size, width, size * 2.0),
        LabelAlign::Center,
        size,
        color,
    )
}

fn with_alpha(color: Color, alpha: f32) -> Color {
    Color::rgba(color.r, color.g, color.b, alpha)
}

/// Text that reads over this color.
fn ink_over(color: Color) -> Color {
    let luminance = 0.299 * color.r + 0.587 * color.g + 0.114 * color.b;
    if luminance > 0.6 { Color::rgba(0.07, 0.08, 0.11, 1.0) } else { Color::WHITE }
}

fn stroke(path: Vec<PathSegment>, width: f32, color: Color) -> Layer {
    Layer::Shape(Shape { path, fill: None, stroke: Some((width, color)) })
}

fn fill(path: Vec<PathSegment>, color: Color) -> Layer {
    Layer::Shape(Shape::filled(path, color))
}

fn dot(at: Point, radius: f32, color: Color) -> Layer {
    let side = radius * 2.0;
    fill(vector::ellipse(Rect::new(at.x - radius, at.y - radius, side, side)), color)
}

/// The two control points of each stretch of a natural spline, along one axis,
/// from the system that makes curves agree in slope and bend where they meet.
fn natural_controls(values: &[f32]) -> (Vec<f32>, Vec<f32>) {
    let n = values.len() - 1;
    let (mut lower, mut main, mut right) = (vec![1.0; n], vec![4.0; n], vec![0.0; n]);
    lower[0] = 0.0;
    main[0] = 2.0;
    right[0] = values[0] + 2.0 * values[1];
    for i in 1..n - 1 {
        right[i] = 4.0 * values[i] + 2.0 * values[i + 1];
    }
    lower[n - 1] = 2.0;
    main[n - 1] = 7.0;
    right[n - 1] = 8.0 * values[n - 1] + values[n];
    for i in 1..n {
        let m = lower[i] / main[i - 1];
        main[i] -= m;
        right[i] -= m * right[i - 1];
    }
    let mut first = vec![0.0; n];
    first[n - 1] = right[n - 1] / main[n - 1];
    for i in (0..n - 1).rev() {
        first[i] = (right[i] - first[i + 1]) / main[i];
    }
    let mut second = vec![0.0; n];
    second[n - 1] = (values[n] + first[n - 1]) / 2.0;
    for i in 0..n - 1 {
        second[i] = 2.0 * values[i + 1] - first[i + 1];
    }
    (first, second)
}

/// The slope at each point of a curve that never swings past them.
fn monotone_slopes(points: &[Point]) -> Vec<f32> {
    let secant = |a: Point, b: Point| {
        let run = b.x - a.x;
        if run.abs() < f32::EPSILON { 0.0 } else { (b.y - a.y) / run }
    };
    let n = points.len();
    let mut slopes = vec![0.0; n];
    slopes[0] = secant(points[0], points[1]);
    slopes[n - 1] = secant(points[n - 2], points[n - 1]);
    for i in 1..n - 1 {
        let (before, after) = (secant(points[i - 1], points[i]), secant(points[i], points[i + 1]));
        slopes[i] = if before * after <= 0.0 { 0.0 } else { 2.0 * before * after / (before + after) };
    }
    slopes
}

/// The path through `points`, without its starting move.
pub fn curve_through(points: &[Point], curve: Curve) -> Vec<PathSegment> {
    if points.len() < 2 {
        return Vec::new();
    }
    match curve {
        Curve::Natural if points.len() > 2 => {
            let xs: Vec<f32> = points.iter().map(|p| p.x).collect();
            let ys: Vec<f32> = points.iter().map(|p| p.y).collect();
            let ((x1, x2), (y1, y2)) = (natural_controls(&xs), natural_controls(&ys));
            (0..points.len() - 1)
                .map(|i| {
                    PathSegment::Cubic(Point::new(x1[i], y1[i]), Point::new(x2[i], y2[i]), points[i + 1])
                })
                .collect()
        }
        Curve::Monotone => {
            let slopes = monotone_slopes(points);
            points
                .windows(2)
                .enumerate()
                .map(|(i, pair)| {
                    let third = (pair[1].x - pair[0].x) / 3.0;
                    PathSegment::Cubic(
                        Point::new(pair[0].x + third, pair[0].y + slopes[i] * third),
                        Point::new(pair[1].x - third, pair[1].y - slopes[i + 1] * third),
                        pair[1],
                    )
                })
                .collect()
        }
        Curve::Step => {
            let mut path = Vec::with_capacity(points.len() * 3);
            for pair in points.windows(2) {
                let middle = (pair[0].x + pair[1].x) / 2.0;
                path.push(PathSegment::Line(Point::new(middle, pair[0].y)));
                path.push(PathSegment::Line(Point::new(middle, pair[1].y)));
                path.push(PathSegment::Line(pair[1]));
            }
            path
        }
        _ => points.iter().skip(1).map(|p| PathSegment::Line(*p)).collect(),
    }
}

/// The corners of a bar, clockwise from the top left, kept to what fits in it.
fn bar_corners(rect: Rect, start: f32, end: f32, horizontal: bool) -> [f32; 4] {
    let limit = rect.width().min(rect.height()) / 2.0;
    let (start, end) = (start.clamp(0.0, limit), end.clamp(0.0, limit));
    if horizontal { [start, end, end, start] } else { [end, end, start, start] }
}

/// A rectangle with its own radius at the start and at the end of its length.
fn bar_path(rect: Rect, start: f32, end: f32, horizontal: bool) -> Vec<PathSegment> {
    if (start - end).abs() < f32::EPSILON {
        return vector::rectangle(rect, start);
    }
    let (x0, x1, y0, y1) = (rect.x(), rect.max_x(), rect.y(), rect.max_y());
    let [top_left, top_right, bottom_right, bottom_left] = bar_corners(rect, start, end, horizontal);
    let at = Point::new;
    let k = 1.0 - 0.552_284_8;
    vec![
        PathSegment::Move(at(x0 + top_left, y0)),
        PathSegment::Line(at(x1 - top_right, y0)),
        PathSegment::Cubic(at(x1 - top_right * k, y0), at(x1, y0 + top_right * k), at(x1, y0 + top_right)),
        PathSegment::Line(at(x1, y1 - bottom_right)),
        PathSegment::Cubic(
            at(x1, y1 - bottom_right * k),
            at(x1 - bottom_right * k, y1),
            at(x1 - bottom_right, y1),
        ),
        PathSegment::Line(at(x0 + bottom_left, y1)),
        PathSegment::Cubic(
            at(x0 + bottom_left * k, y1),
            at(x0, y1 - bottom_left * k),
            at(x0, y1 - bottom_left),
        ),
        PathSegment::Line(at(x0, y0 + top_left)),
        PathSegment::Cubic(at(x0, y0 + top_left * k), at(x0 + top_left * k, y0), at(x0 + top_left, y0)),
        PathSegment::Close,
    ]
}

/// The values of each series, stacked: where each one starts and ends per category.
fn stacked(data: &ChartData, stack: Stack) -> Vec<Vec<(f64, f64)>> {
    let count = data.series.iter().map(|s| s.values.len()).max().unwrap_or(0);
    let value =
        |s: usize, i: usize| data.series[s].values.get(i).copied().filter(|v| v.is_finite()).unwrap_or(0.0);
    let totals: Vec<f64> =
        (0..count).map(|i| (0..data.series.len()).map(|s| value(s, i).abs()).sum()).collect();
    let mut above = vec![0.0; count];
    let mut below = vec![0.0; count];
    (0..data.series.len())
        .map(|s| {
            (0..count)
                .map(|i| match stack {
                    Stack::None => (0.0, value(s, i)),
                    Stack::Normal | Stack::Expand => {
                        let mut v = value(s, i);
                        if stack == Stack::Expand && totals[i] > 0.0 {
                            v /= totals[i];
                        }
                        let tower = if v < 0.0 { &mut below } else { &mut above };
                        let start = tower[i];
                        tower[i] += v;
                        (start, tower[i])
                    }
                })
                .collect()
        })
        .collect()
}

/// Where the box of a label goes, `half` to each side of `at`: centered, unless a
/// measured label would stick out of the chart, and then against the chart's edge.
fn inside(at: f32, width: f32, half: f32, rect: Rect, ruler: Ruler<'_>) -> (f32, LabelAlign) {
    if ruler.0.is_some() && at - width / 2.0 < rect.x() {
        (rect.x(), LabelAlign::Left)
    } else if ruler.0.is_some() && at + width / 2.0 > rect.max_x() {
        (rect.max_x() - half * 2.0, LabelAlign::Right)
    } else {
        (at - half, LabelAlign::Center)
    }
}

/// A gridline, wherever this chart wants them.
fn push_rule(plot: &mut Plot, style: &PlotStyle, rect: Rect, color: Color) {
    let mut push = |rect: Rect| {
        if style.rules_apart {
            plot.rules.push(Rule { rect, color });
        } else {
            plot.layers.push(fill(vector::rectangle(rect, 0.0), color));
        }
    };
    if !style.grid_dashed {
        push(rect);
        return;
    }
    each_dash(rect, &mut push);
}

/// The dashes of a thin line, the last one cut short by its end.
fn each_dash(rect: Rect, push: &mut dyn FnMut(Rect)) {
    let upright = rect.height() > rect.width();
    let length = if upright { rect.height() } else { rect.width() };
    let mut from = 0.0;
    while from < length {
        let dash = GRID_DASH.min(length - from);
        push(if upright {
            Rect::new(rect.x(), rect.y() + from, rect.width(), dash)
        } else {
            Rect::new(rect.x() + from, rect.y(), dash, rect.height())
        });
        from += GRID_DASH + GRID_DASH_GAP;
    }
}

struct Axes {
    area: Rect,
    low: f64,
    high: f64,
    horizontal: bool,
    /// The axis runs the other way.
    reversed: bool,
    /// What is past the ends of the axis is cut off.
    fixed: bool,
}

impl Axes {
    /// How far along the value axis a number is, in the page's coordinates.
    fn at(&self, value: f64) -> f32 {
        let share = ((value - self.low) / (self.high - self.low).max(f64::EPSILON)) as f32;
        let share = if self.reversed { 1.0 - share } else { share };
        if self.horizontal {
            self.area.x() + share * self.area.width()
        } else {
            self.area.max_y() - share * self.area.height()
        }
    }

    /// The point of a value at a place along the category axis.
    fn point(&self, along: f32, value: f64) -> Point {
        if self.horizontal { Point::new(self.at(value), along) } else { Point::new(along, self.at(value)) }
    }
}

fn cartesian(data: &ChartData, rect: Rect, style: &PlotStyle, ruler: Ruler<'_>, plot: &mut Plot) {
    let count = data.categories.len();
    let is_candle = style.kind == ChartKind::Candlestick;
    if count == 0 || data.series.is_empty() || (is_candle && data.series.len() < CANDLE_SERIES) {
        return;
    }
    let unstacked;
    // Prices do not add up.
    let style = if is_candle && style.stack != Stack::None {
        unstacked = PlotStyle { stack: Stack::None, ..style.clone() };
        &unstacked
    } else {
        style
    };
    let towers = stacked(data, style.stack);
    let is_bar = style.kind == ChartKind::Bar;
    let banded = is_bar || is_candle;
    let values = towers.iter().flatten().flat_map(|(a, b)| [*a, *b]);
    let (mut min, mut max) =
        values.fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), v| (a.min(v), b.max(v)));
    if style.stack == Stack::None {
        let real = data.series.iter().flat_map(|s| s.values.iter().copied()).filter(|v| v.is_finite());
        (min, max) = real.fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), v| (a.min(v), b.max(v)));
    }
    if !min.is_finite() {
        return;
    }
    // A candle is about where the price went, not how far it is from zero.
    if !is_candle && (!style.tight_scale || is_bar) {
        min = min.min(0.0);
        max = max.max(0.0);
    }
    let domain =
        style.domain.filter(|(a, b)| a.is_finite() && b.is_finite() && a < b && style.stack != Stack::Expand);
    if domain.is_none() && style.stack != Stack::Expand {
        // A target above every value is still worth seeing.
        for reference in style.references.iter().filter(|r| r.value.is_finite()) {
            min = min.min(reference.value);
            max = max.max(reference.value);
        }
    }
    if let Some((from, to)) = domain {
        (min, max) = (from, to);
    }
    let (mut low, step, ticks) = nice_ticks(min, max, style.value_ticks.max(1));
    let mut high = low + step * ticks.saturating_sub(1) as f64;
    if style.stack == Stack::Expand {
        high = 1.0;
    }
    let mut tick_values: Vec<f64> = if style.stack == Stack::Expand {
        (0..=4).map(|i| f64::from(i) / 4.0).collect()
    } else {
        (0..ticks).map(|i| low + step * i as f64).collect()
    };
    if let Some((from, to)) = domain {
        // Round numbers stay, as far as they are inside what was asked for.
        let slack = step * 1e-6;
        tick_values.retain(|v| *v >= from - slack && *v <= to + slack);
        (low, high) = (from, to);
    }
    let tick_text = |value: f64| {
        if style.stack == Stack::Expand { format!("{:.0}%", value * 100.0) } else { (style.format)(value) }
    };
    let size = style.size;
    let line_height = size * 1.3;
    let horizontal = is_bar && style.horizontal;
    let tick_texts: Vec<String> = tick_values.iter().map(|v| tick_text(*v)).collect();
    let longest_value = ruler.longest(tick_texts.iter().map(String::as_str), size).ceil();
    let labels: Vec<String> = data.categories.iter().map(|c| (style.tick_format)(c)).collect();
    let longest_label = ruler.longest(labels.iter().map(String::as_str), size).ceil();

    let outside =
        style.value_labels == Some(LabelAt::Outside) || style.category_labels == Some(LabelAt::Outside);
    let mut left = rect.x();
    let mut right = rect.max_x();
    let mut top = rect.y() + 4.0;
    let mut bottom = rect.max_y();
    let reversed = is_bar && style.reversed;
    if horizontal {
        let (categories, tips) = (longest_label + TICK_MARGIN, longest_value.max(size * 3.0) + TICK_MARGIN);
        if style.category_axis {
            if reversed {
                right -= categories;
            } else {
                left += categories;
            }
        }
        if style.value_axis {
            bottom -= line_height + TICK_MARGIN;
        }
        if outside {
            if reversed {
                left += tips;
            } else {
                right -= tips;
            }
        }
    } else {
        if style.category_axis {
            if reversed {
                top += line_height + TICK_MARGIN;
            } else {
                bottom -= line_height + TICK_MARGIN;
            }
        }
        if style.value_axis {
            left += longest_value + TICK_MARGIN;
        }
        if outside {
            let (tips, under_zero) = (line_height + 2.0, if min < 0.0 { line_height } else { 0.0 });
            // A label past a bar under zero needs the same room there.
            if reversed {
                bottom -= tips;
                top += under_zero;
            } else {
                top += tips;
                bottom -= under_zero;
            }
        }
        if !banded {
            left += if style.value_axis { 4.0 } else { LINE_MARGIN };
            right -= LINE_MARGIN;
        }
    }
    let area = Rect::new(left, top, (right - left).max(1.0), (bottom - top).max(1.0));
    plot.area = area;
    let axes = Axes { area, low, high, horizontal, reversed, fixed: domain.is_some() };

    // The grid.
    let grid = with_alpha(style.grid, style.grid.a * 0.8);
    // Evenly spread columns take the place of whatever lines stand upright.
    let columns = style.grid_columns.filter(|n| *n > 0);
    let value_lines = if horizontal {
        columns.is_none() && matches!(style.grid_lines, Grid::Vertical | Grid::Both)
    } else {
        matches!(style.grid_lines, Grid::Horizontal | Grid::Both)
    };
    let category_lines = if horizontal {
        matches!(style.grid_lines, Grid::Horizontal | Grid::Both)
    } else {
        columns.is_none() && matches!(style.grid_lines, Grid::Vertical | Grid::Both)
    };
    if let Some(columns) = columns {
        for i in 0..=columns {
            let at = area.x() + area.width() * i as f32 / columns as f32;
            push_rule(plot, style, Rect::new(at - 0.5, area.y(), 1.0, area.height()), grid);
        }
    }
    let along_size = if horizontal { area.height() } else { area.width() };
    let along_start = if horizontal { area.y() } else { area.x() };
    let band = along_size / count as f32;
    let center_of = |i: usize| {
        if banded {
            along_start + band * (i as f32 + 0.5)
        } else if count > 1 {
            along_start + along_size * i as f32 / (count - 1) as f32
        } else {
            along_start + along_size / 2.0
        }
    };
    for value in &tick_values {
        let at = axes.at(*value);
        if value_lines {
            let line = if horizontal {
                Rect::new(at - 0.5, area.y(), 1.0, area.height())
            } else {
                Rect::new(area.x(), at - 0.5, area.width(), 1.0)
            };
            push_rule(plot, style, line, grid);
        }
        if style.value_axis {
            let label = tick_text(*value);
            plot.labels.push(if horizontal {
                let (x, align) = inside(at, ruler.width(&label, size), 40.0, rect, ruler);
                text(
                    label,
                    Rect::new(x, area.max_y() + TICK_MARGIN, 80.0, line_height),
                    align,
                    size,
                    style.muted,
                )
            } else {
                text(
                    label,
                    Rect::new(rect.x(), at - line_height / 2.0, longest_value, line_height),
                    LabelAlign::Right,
                    size,
                    style.muted,
                )
            });
        }
    }
    if category_lines {
        for i in 0..count {
            let at = center_of(i);
            let line = if horizontal {
                Rect::new(area.x(), at - 0.5, area.width(), 1.0)
            } else {
                Rect::new(at - 0.5, area.y(), 1.0, area.height())
            };
            push_rule(plot, style, line, grid);
        }
    }

    // The categories along the axis, as many as fit, and the strips they own.
    let room = if horizontal { line_height + 2.0 } else { longest_label + TICK_MARGIN * 2.0 };
    let spacing = if count > 1 { (center_of(1) - center_of(0)).abs() } else { along_size };
    // Far from the size of a pixel: below one, categories are that much closer together.
    let every = (room / spacing.max(0.001)).ceil().max(1.0) as usize;
    let aligned = style.ends_aligned && !banded && !horizontal && count > 1;
    let last = count.saturating_sub(1);
    for (i, label) in labels.iter().enumerate() {
        let at = center_of(i);
        // With its ends lined up the last category always shows, in place of the one
        // before it if there is no room for both.
        let crowded = aligned && (last - i) as f32 * spacing < room && i != last;
        let shows = if aligned && i == last { true } else { i.is_multiple_of(every) && !crowded };
        // On the line of zero, it goes on the side the bars of this category leave free.
        let at_zero = (is_bar && style.labels_at_zero).then(|| {
            let values = data.series.iter().filter_map(|s| s.values.get(i)).filter(|v| v.is_finite());
            let mut seen = values.clone().peekable();
            let under = seen.peek().is_some()
                && values.clone().all(|v| *v <= 0.0)
                && values.clone().any(|v| *v < 0.0);
            (axes.at(0.0_f64.clamp(low, high)), under != reversed)
        });
        if style.category_axis && shows {
            plot.labels.push(if let (true, Some((zero, past))) = (horizontal, at_zero) {
                let x = if past { zero + TICK_MARGIN } else { zero - TICK_MARGIN - longest_label };
                let align = if past { LabelAlign::Left } else { LabelAlign::Right };
                let place = Rect::new(x, at - line_height / 2.0, longest_label, line_height);
                text(label.clone(), place, align, size, style.muted)
            } else if horizontal {
                let (x, align) = if reversed {
                    (area.max_x() + TICK_MARGIN, LabelAlign::Left)
                } else {
                    (rect.x(), LabelAlign::Right)
                };
                text(
                    label.clone(),
                    Rect::new(x, at - line_height / 2.0, longest_label, line_height),
                    align,
                    size,
                    style.muted,
                )
            } else {
                let (x, align) = if aligned && i == 0 {
                    (at, LabelAlign::Left)
                } else if aligned && i == last {
                    (at - spacing * 2.0, LabelAlign::Right)
                } else {
                    inside(at, ruler.width(label, size), spacing, rect, ruler)
                };
                let y = match at_zero {
                    Some((zero, true)) => zero - TICK_MARGIN - line_height,
                    Some((zero, false)) => zero + TICK_MARGIN,
                    None if reversed => area.y() - TICK_MARGIN - line_height,
                    None => area.max_y() + TICK_MARGIN,
                };
                text(label.clone(), Rect::new(x, y, spacing * 2.0, line_height), align, size, style.muted)
            });
        }
        let half = spacing / 2.0;
        let band_rect = if horizontal {
            Rect::new(rect.x(), at - half, rect.width(), spacing)
        } else {
            Rect::new(at - half, rect.y(), spacing, rect.height())
        };
        let center =
            if horizontal { Point::new(area.center().x, at) } else { Point::new(at, area.center().y) };
        plot.spots.push(Spot { index: i, band: band_rect, center });
    }

    if is_bar {
        bars(data, &towers, style, &axes, band, plot);
        if style.bars_apart {
            plot.rules = rules_around_bars(&plot.rules, &plot.bars);
        }
    } else if is_candle {
        candles(data, style, &axes, band, plot);
    } else {
        lines(data, &towers, style, &axes, &center_of, plot);
    }
    references(style, &axes, rect, ruler, plot);
}

/// How far from its side a rounded corner leaves a line `from_end` past the bar's end.
fn corner_inset(radius: f32, from_end: f32) -> f32 {
    if radius <= 0.0 || from_end >= radius {
        return 0.0;
    }
    let rise = radius - from_end;
    radius - (radius * radius - rise * rise).max(0.0).sqrt()
}

/// What is left of a thin line once it stops at the sides of a bar, following its
/// corners. A line along the bar's very edge stays: it is not behind it.
fn line_around_bar(line: Rect, bar: &Bar, out: &mut Vec<Rect>) {
    let [top_left, top_right, bottom_right, bottom_left] = bar.radii;
    let center = line.center();
    let upright = line.height() > line.width();
    let (from, to) = if upright {
        if center.x <= bar.rect.x() || center.x >= bar.rect.max_x() {
            out.push(line);
            return;
        }
        let (left, right) = (center.x - bar.rect.x(), bar.rect.max_x() - center.x);
        let start = corner_inset(top_left, left).max(corner_inset(top_right, right));
        let end = corner_inset(bottom_left, left).max(corner_inset(bottom_right, right));
        (bar.rect.y() + start, bar.rect.max_y() - end)
    } else {
        if center.y <= bar.rect.y() || center.y >= bar.rect.max_y() {
            out.push(line);
            return;
        }
        let (above, below) = (center.y - bar.rect.y(), bar.rect.max_y() - center.y);
        let start = corner_inset(top_left, above).max(corner_inset(bottom_left, below));
        let end = corner_inset(top_right, above).max(corner_inset(bottom_right, below));
        (bar.rect.x() + start, bar.rect.max_x() - end)
    };
    let (begin, finish) = if upright { (line.y(), line.max_y()) } else { (line.x(), line.max_x()) };
    let piece = |a: f32, b: f32| {
        if upright {
            Rect::new(line.x(), a, line.width(), b - a)
        } else {
            Rect::new(a, line.y(), b - a, line.height())
        }
    };
    if from > begin {
        out.push(piece(begin, from.min(finish)));
    }
    if to < finish {
        out.push(piece(to.max(begin), finish));
    }
}

/// Gridlines stop where a bar starts and go on where it ends.
fn rules_around_bars(rules: &[Rule], bars: &[Bar]) -> Vec<Rule> {
    let mut kept = Vec::with_capacity(rules.len());
    let (mut pieces, mut next) = (Vec::new(), Vec::new());
    for rule in rules {
        pieces.clear();
        pieces.push(rule.rect);
        for bar in bars {
            next.clear();
            for piece in &pieces {
                line_around_bar(*piece, bar, &mut next);
            }
            std::mem::swap(&mut pieces, &mut next);
        }
        kept.extend(pieces.iter().map(|rect| Rule { rect: *rect, color: rule.color }));
    }
    kept
}

/// The lines of reference that fall on the axis, over the data.
fn references(style: &PlotStyle, axes: &Axes, rect: Rect, ruler: Ruler<'_>, plot: &mut Plot) {
    let (area, size) = (axes.area, style.size);
    let line_height = size * 1.3;
    for reference in &style.references {
        if !reference.value.is_finite() || reference.value < axes.low || reference.value > axes.high {
            continue;
        }
        let at = axes.at(reference.value);
        let color = reference.color.unwrap_or(style.muted);
        let line = if axes.horizontal {
            Rect::new(at - 0.5, area.y(), 1.0, area.height())
        } else {
            Rect::new(area.x(), at - 0.5, area.width(), 1.0)
        };
        each_dash(line, &mut |rect| {
            if style.rules_apart {
                plot.top_rules.push(Rule { rect, color });
            } else {
                plot.layers.push(fill(vector::rectangle(rect, 0.0), color));
            }
        });
        let Some(label) = &reference.label else { continue };
        let wide = ruler.width(label, size).ceil() + 2.0;
        let place = if axes.horizontal {
            // At the top, on the side of the line with room.
            let x = if at + wide + 4.0 > rect.max_x() { at - wide - 4.0 } else { at + 4.0 };
            Rect::new(x, area.y(), wide, line_height)
        } else {
            // At the end, over the line unless that would leave the chart.
            let above = at - line_height - 1.0;
            let y = if above < rect.y() { at + 1.0 } else { above };
            Rect::new(area.max_x() - wide, y, wide, line_height)
        };
        plot.labels.push(text(label.clone(), place, LabelAlign::Right, size, color));
    }
}

fn bars(
    data: &ChartData,
    towers: &[Vec<(f64, f64)>],
    style: &PlotStyle,
    axes: &Axes,
    band: f32,
    plot: &mut Plot,
) {
    let horizontal = axes.horizontal;
    let side_by_side = style.stack == Stack::None;
    let slots = if side_by_side { data.series.len().max(1) } else { 1 };
    let usable = band * (1.0 - CATEGORY_GAP) - BAR_GAP * (slots - 1) as f32;
    let thickness = (usable / slots as f32).clamp(1.0, style.max_bar);
    let group = thickness * slots as f32 + BAR_GAP * (slots - 1) as f32;
    let start = if horizontal { axes.area.y() } else { axes.area.x() };
    let size = style.size;
    let reversed = axes.reversed;
    for (s, series) in data.series.iter().enumerate() {
        let series_color = style.color_of(s);
        let series_color = if series.color == Color::TRANSPARENT { series_color } else { series.color };
        for (i, (from, to)) in towers[s].iter().enumerate() {
            // A bar wholly past the ends of a fixed axis has nothing left to show.
            if axes.fixed && (from.max(*to) < axes.low || from.min(*to) > axes.high) {
                continue;
            }
            // Cut off, an end is square: it does not stop there.
            let cut_low = axes.fixed && from.min(*to) < axes.low;
            let cut_high = axes.fixed && from.max(*to) > axes.high;
            let reach = |value: f64| if axes.fixed { value.clamp(axes.low, axes.high) } else { value };
            let (from, to) = (&reach(*from), &reach(*to));
            let slot = if side_by_side { s } else { 0 };
            let along = start + band * i as f32 + (band - group) / 2.0 + (thickness + BAR_GAP) * slot as f32;
            let (a, b) = (axes.at(*from), axes.at(*to));
            // A bar too short grows away from its base, and nothing at all stays a sliver.
            let least = if from == to { style.min_bar.min(0.5) } else { style.min_bar };
            let b = if (b - a).abs() < least { a + least * if b < a { -1.0 } else { 1.0 } } else { b };
            let (low, high) = (a.min(b), a.max(b));
            let length = high - low;
            let rect = if horizontal {
                Rect::new(low, along, length, thickness)
            } else {
                Rect::new(along, low, thickness, length)
            };
            let original = series.values.get(i).copied().unwrap_or(0.0);
            let mut color = style.item_color(series_color, i);
            if original < 0.0
                && let Some(negative) = style.negative
            {
                color = negative;
            }
            let below = to < from;
            // In a tower, only its outer corners are round: those of whatever has a
            // value, each way from zero, wherever that is in the order of the series.
            let (base_round, tip_round) = match style.stack {
                Stack::None => (style.bar_radius, style.bar_radius),
                _ => {
                    let same_way = |tower: &Vec<(f64, f64)>| {
                        tower.get(i).is_some_and(|(a, b)| a != b && (b < a) == below)
                    };
                    let first = towers.iter().position(same_way);
                    let last = towers.iter().rposition(same_way);
                    (
                        if first == Some(s) { style.bar_radius } else { 0.0 },
                        if last == Some(s) { style.bar_radius } else { 0.0 },
                    )
                }
            };
            // `start` is the bottom or the left of the bar, and its tip may be there.
            let tip_at_start = below != reversed;
            let (first, end) = if tip_at_start { (tip_round, base_round) } else { (base_round, tip_round) };
            let (cut_start, cut_end) = if reversed { (cut_high, cut_low) } else { (cut_low, cut_high) };
            let (first, end) = (if cut_start { 0.0 } else { first }, if cut_end { 0.0 } else { end });
            let path = bar_path(rect, first, end, horizontal);
            let radii = bar_corners(rect, first, end, horizontal);
            let shown = if style.active == Some(i) { with_alpha(color, 0.8) } else { color };
            let gradient = style.gradient.then(|| {
                let (at_start, at_end) = if horizontal {
                    (Point::new(0.0, 0.0), Point::new(1.0, 0.0))
                } else {
                    (Point::new(0.0, 1.0), Point::new(0.0, 0.0))
                };
                let (tip, base) = if tip_at_start { (at_start, at_end) } else { (at_end, at_start) };
                let faint = with_alpha(shown, shown.a * BAR_GRADIENT_BASE);
                LinearGradient { start: tip, end: base, from: shown, to: faint }
            });
            // Nothing in a tower is not a line across it.
            let empty_in_tower = style.stack != Stack::None && from == to;
            if !empty_in_tower {
                plot.bars.push(Bar { series: s, index: i, rect, color: shown, radii, gradient });
            }
            if empty_in_tower {
            } else if style.active == Some(i) {
                if !style.bars_apart {
                    plot.layers.push(fill(path.clone(), shown));
                }
                plot.layers.push(Layer::Dashed { path, width: 2.0, dash: 4.0, color });
            } else if !style.bars_apart {
                plot.layers.push(fill(path, color));
            }
            let tip_value = *to;
            let center = rect.center();
            let tip = axes.point(along + thickness / 2.0, tip_value);
            plot.marks.push(Mark { series: s, index: i, at: tip, color });
            let category = data.categories.get(i).map(|c| (style.tick_format)(c)).unwrap_or_default();
            let number = (style.format)(original);
            let mut write = |content: String, at: LabelAt| {
                let wide = content.chars().count() as f32 * size * CHAR_WIDTH + 8.0;
                let height = size * 1.3;
                plot.labels.push(match (at, horizontal) {
                    (LabelAt::Outside, true) if reversed && original >= 0.0 => text(
                        content,
                        Rect::new(rect.x() - 6.0 - wide, center.y - height / 2.0, wide, height),
                        LabelAlign::Right,
                        size,
                        style.ink,
                    ),
                    (LabelAt::Outside, true) => text(
                        content,
                        Rect::new(rect.max_x() + 6.0, center.y - height / 2.0, wide, height),
                        LabelAlign::Left,
                        size,
                        style.ink,
                    ),
                    (LabelAt::Outside, false) => {
                        let under = (original < 0.0) != reversed;
                        let y = if under { rect.max_y() + 2.0 } else { rect.y() - height - 2.0 };
                        centered(content, Point::new(center.x, y + height / 2.0), size, style.ink)
                    }
                    (LabelAt::InsideStart, true) if reversed => text(
                        content,
                        Rect::new(rect.max_x() - 8.0 - wide, center.y - height / 2.0, wide, height),
                        LabelAlign::Right,
                        size,
                        ink_over(color),
                    ),
                    (LabelAt::InsideStart, true) => text(
                        content,
                        Rect::new(rect.x() + 8.0, center.y - height / 2.0, wide, height),
                        LabelAlign::Left,
                        size,
                        ink_over(color),
                    ),
                    (LabelAt::InsideStart, false) if reversed => {
                        centered(content, Point::new(center.x, rect.y() + height), size, ink_over(color))
                    }
                    (LabelAt::InsideStart, false) => {
                        centered(content, Point::new(center.x, rect.max_y() - height), size, ink_over(color))
                    }
                    _ => centered(content, center, size, ink_over(color)),
                });
            };
            if let Some(at) = style.value_labels {
                write(number, at);
            }
            if let Some(at) = style.category_labels {
                write(category, at);
            }
        }
    }
}

fn candles(data: &ChartData, style: &PlotStyle, axes: &Axes, band: f32, plot: &mut Plot) {
    let [open, high, low, close] = [0, 1, 2, 3].map(|s: usize| data.series[s].values.as_slice());
    let width = (band * CANDLE_WIDTH).clamp(1.0, style.max_bar);
    let reach = |value: f64| if axes.fixed { value.clamp(axes.low, axes.high) } else { value };
    for i in 0..data.categories.len() {
        let value = |of: &[f64]| of.get(i).copied().filter(|v| v.is_finite());
        let (Some(open), Some(high), Some(low), Some(close)) =
            (value(open), value(high), value(low), value(close))
        else {
            continue;
        };
        // Whatever it says, the wick covers where the price was.
        let (top, bottom) = (high.max(open).max(close), low.min(open).min(close));
        if axes.fixed && (top < axes.low || bottom > axes.high) {
            continue;
        }
        let x = axes.area.x() + band * (i as f32 + 0.5);
        let (upper, lower) = (axes.at(reach(open.max(close))), axes.at(reach(open.min(close))));
        let body = Rect::new(x - width / 2.0, upper, width, (lower - upper).max(1.0));
        let (high_at, low_at) = (axes.at(reach(top)), axes.at(reach(bottom)));
        let wicks = [
            Rect::new(x - 0.5, high_at, 1.0, (body.y() - high_at).max(0.0)),
            Rect::new(x - 0.5, body.max_y(), 1.0, (low_at - body.max_y()).max(0.0)),
        ];
        let rising = close >= open;
        let color = if rising { style.rising } else { style.falling };
        plot.marks.push(Mark { series: 3, index: i, at: Point::new(x, axes.at(reach(close))), color });
        plot.candles.push(Candle { index: i, body, wicks, color, rising });
        if style.bars_apart {
            continue;
        }
        for wick in wicks {
            plot.layers.push(fill(vector::rectangle(wick, 0.0), color));
        }
        let outline = vector::rectangle(body, 1.0);
        plot.layers.push(if rising { stroke(outline, 1.5, color) } else { fill(outline, color) });
    }
}

/// The lowest and the highest point of each column, in the order they come, and the
/// two ends: all a line with more points than columns can show.
fn thinned(points: &[Point], area: Rect) -> Vec<Point> {
    let mut kept = Vec::with_capacity(area.width() as usize * 2 + 2);
    let mut column = Vec::new();
    let mut flush = |column: &mut Vec<Point>| {
        let by_height = |a: &&Point, b: &&Point| a.y.total_cmp(&b.y);
        let (Some(low), Some(high)) = (column.iter().min_by(by_height), column.iter().max_by(by_height))
        else {
            return;
        };
        if low.x <= high.x {
            kept.extend([*low, *high]);
        } else {
            kept.extend([*high, *low]);
        }
        column.clear();
    };
    let mut current = f32::NEG_INFINITY;
    for point in points {
        let at = (point.x - area.x()).floor();
        if at != current {
            flush(&mut column);
            current = at;
        }
        column.push(*point);
    }
    flush(&mut column);
    if let (Some(first), Some(last)) = (points.first(), points.last()) {
        kept.insert(0, *first);
        kept.push(*last);
    }
    kept
}

/// The path through the points of a line. With more of them than columns on the
/// screen a curve shows nothing a straight line does not, and past two per column
/// only the extremes of each can be told apart: they are all that is drawn, or a
/// long series would cost a picture far more than what can be seen of it.
fn curve_within(points: &[Point], curve: Curve, area: Rect) -> Vec<PathSegment> {
    let columns = area.width().max(1.0) as usize;
    if points.len() > columns * 2 {
        let kept = thinned(points, area);
        return curve_through(&kept, Curve::Linear);
    }
    curve_through(points, if points.len() > columns { Curve::Linear } else { curve })
}

fn lines(
    data: &ChartData,
    towers: &[Vec<(f64, f64)>],
    style: &PlotStyle,
    axes: &Axes,
    center_of: &dyn Fn(usize) -> f32,
    plot: &mut Plot,
) {
    let filled = style.kind == ChartKind::Area;
    let is_stacked = style.stack != Stack::None;
    let size = style.size;
    for (s, series) in data.series.iter().enumerate() {
        let color = if series.color == Color::TRANSPARENT { style.color_of(s) } else { series.color };
        // A value that is missing breaks nothing: the line goes from the one before to the one after.
        let finite = |i: usize| series.values.get(i).is_some_and(|v| v.is_finite());
        let places: Vec<usize> = (0..towers[s].len()).filter(|i| finite(*i)).collect();
        let tops: Vec<Point> =
            places.iter().map(|i| Point::new(center_of(*i), axes.at(towers[s][*i].1))).collect();
        let (Some(first), Some(last)) = (tops.first().copied(), tops.last().copied()) else { continue };
        let curve = curve_within(&tops, style.curve, axes.area);
        let clipped = |layer: Layer| {
            if axes.fixed {
                // A little past the area, so a line along its edge keeps its width.
                Layer::Clipped { rect: axes.area.inflate(1.0), layer: Box::new(layer) }
            } else {
                layer
            }
        };
        if filled {
            let mut path = vec![PathSegment::Move(first)];
            path.extend(curve.iter().copied());
            if is_stacked {
                let mut under: Vec<Point> =
                    places.iter().map(|i| Point::new(center_of(*i), axes.at(towers[s][*i].0))).collect();
                under.reverse();
                path.push(PathSegment::Line(under[0]));
                path.extend(curve_within(&under, style.curve, axes.area));
            } else {
                let zero = axes.at(0.0_f64.clamp(axes.low, axes.high));
                path.push(PathSegment::Line(Point::new(last.x, zero)));
                path.push(PathSegment::Line(Point::new(first.x, zero)));
            }
            path.push(PathSegment::Close);
            if style.gradient {
                plot.layers.push(clipped(Layer::Gradient {
                    path,
                    top: (axes.area.y(), with_alpha(color, 0.8)),
                    bottom: (axes.area.max_y(), with_alpha(color, 0.1)),
                }));
            } else {
                plot.layers.push(clipped(fill(path, with_alpha(color, style.fill_opacity))));
            }
        }
        let mut line = vec![PathSegment::Move(first)];
        line.extend(curve);
        plot.layers.push(clipped(stroke(line, if filled { 1.5 } else { 2.0 }, color)));
        for (place, top) in places.iter().zip(&tops) {
            // A value past the ends of a fixed axis has no place on the chart.
            if axes.fixed && (top.y < axes.area.y() - 0.01 || top.y > axes.area.max_y() + 0.01) {
                continue;
            }
            let item = style.item_color(color, *place);
            plot.marks.push(Mark { series: s, index: *place, at: *top, color: item });
            if style.dots {
                plot.layers.push(dot(*top, DOT_RADIUS, item));
            }
            let above = Point::new(top.x, top.y - size * 1.1);
            if style.value_labels.is_some() {
                let value = series.values.get(*place).copied().unwrap_or(0.0);
                plot.labels.push(centered((style.format)(value), above, size, style.ink));
            }
            if style.category_labels.is_some() {
                let name = data.categories.get(*place).map(|c| (style.tick_format)(c)).unwrap_or_default();
                plot.labels.push(centered(name, above, size, style.ink));
            }
        }
    }
}

/// A slice of a ring around `center`; a slice of a circle when `inner` is zero.
pub fn slice_path(center: Point, inner: f32, outer: f32, from: f32, to: f32) -> Vec<PathSegment> {
    let at = |r: f32, a: f32| Point::new(center.x + r * a.cos(), center.y + r * a.sin());
    let mut path = vec![PathSegment::Move(at(outer, from))];
    arc(&mut path, center, outer, from, to);
    if inner > 0.0 {
        path.push(PathSegment::Line(at(inner, to)));
        arc(&mut path, center, inner, to, from);
    } else {
        path.push(PathSegment::Line(center));
    }
    path.push(PathSegment::Close);
    path
}

/// A slice as [`slice_path`] draws it, with `gap` left clear all around it: half on
/// each of its sides, of the same width all the way, and half at its ends. What is
/// behind the chart shows through.
pub fn gapped_slice(center: Point, inner: f32, outer: f32, from: f32, to: f32, gap: f32) -> Vec<PathSegment> {
    if gap <= 0.0 {
        return slice_path(center, inner, outer, from, to);
    }
    let half = gap / 2.0;
    let inner = if inner > 0.0 { inner + half } else { 0.0 };
    let outer = (outer - half).max(inner + 0.5);
    let span = to - from;
    // A whole circle has no sides to keep apart.
    if span >= TAU - 0.001 {
        return slice_path(center, inner, outer, from, to);
    }
    let at = |r: f32, a: f32| Point::new(center.x + r * a.cos(), center.y + r * a.sin());
    // A slice thinner than the gap keeps half of itself, or it would be gone.
    let along_outer = (half / outer).min(span / 4.0);
    let mut path = vec![PathSegment::Move(at(outer, from + along_outer))];
    arc(&mut path, center, outer, from + along_outer, to - along_outer);
    if inner > 0.0 {
        let along_inner = (half / inner).min(span / 4.0);
        path.push(PathSegment::Line(at(inner, to - along_inner)));
        arc(&mut path, center, inner, to - along_inner, from + along_inner);
    } else {
        // Where the two sides meet, once each has moved away from the other.
        let reach = half / (span / 2.0).sin().abs().max(0.05);
        path.push(PathSegment::Line(at(reach.min(outer / 2.0), (from + to) / 2.0)));
    }
    path.push(PathSegment::Close);
    path
}

/// A label outside a round chart, before it knows where it goes.
struct Guide {
    text: String,
    angle: f32,
    reach: f32,
    color: Color,
    share: f64,
}

/// Puts each side's labels in a column in the order of their slices, as close as they
/// fit to where their slices are, and draws the lines that join them.
fn place_guides(
    guides: Vec<Guide>,
    center: Point,
    rect: Rect,
    style: &PlotStyle,
    ruler: Ruler<'_>,
    plot: &mut Plot,
) {
    let line_height = style.size * 1.3;
    let edge = guides.iter().map(|g| g.reach).fold(0.0, f32::max);
    let (mut right, mut left): (Vec<Guide>, Vec<Guide>) =
        guides.into_iter().partition(|g| g.angle.cos() >= 0.0);
    for (side, sign) in [(&mut right, 1.0_f32), (&mut left, -1.0)] {
        let wanted = |g: &Guide| center.y + (g.reach + GUIDE_OUT) * g.angle.sin();
        let (top, bottom) = (rect.y() + line_height / 2.0, rect.max_y() - line_height / 2.0);
        // More than a column holds: the smallest slices go without.
        let holds = ((bottom - top) / line_height).floor().max(0.0) as usize + 1;
        side.sort_by(|a, b| b.share.total_cmp(&a.share));
        side.truncate(holds);
        side.sort_by(|a, b| wanted(a).total_cmp(&wanted(b)));
        let mut places: Vec<f32> = Vec::with_capacity(side.len());
        for guide in side.iter() {
            let least = places.last().map_or(top, |last| last + line_height);
            places.push(wanted(guide).max(least));
        }
        // What ran past the bottom pushes the others back up.
        let mut most = bottom;
        for place in places.iter_mut().rev() {
            *place = place.min(most);
            most = *place - line_height;
        }
        let column = center.x + sign * (edge + GUIDE_OUT + GUIDE_RUN);
        let clear = edge + GUIDE_OUT / 2.0;
        for (guide, y) in side.iter().zip(places) {
            let (cos, sin) = (guide.angle.cos(), guide.angle.sin());
            let from = Point::new(center.x + guide.reach * cos, center.y + guide.reach * sin);
            // It turns outside the circle, wherever the label had to go.
            let along = (clear * clear - (y - center.y).powi(2)).max(0.0).sqrt();
            let turn = (guide.reach + GUIDE_OUT) * cos.abs();
            let elbow = Point::new(center.x + sign * turn.max(along), y);
            let path = vec![
                PathSegment::Move(from),
                PathSegment::Line(elbow),
                PathSegment::Line(Point::new(column, y)),
            ];
            plot.layers.push(stroke(path, 1.0, with_alpha(guide.color, 0.8)));
            let wide = ruler.width(&guide.text, style.size).ceil() + 2.0;
            let (x, align) = if sign > 0.0 {
                (column + GUIDE_GAP, LabelAlign::Left)
            } else {
                (column - GUIDE_GAP - wide, LabelAlign::Right)
            };
            let place = Rect::new(x, y - line_height / 2.0, wide, line_height);
            plot.labels.push(text(guide.text.clone(), place, align, style.size, style.ink));
        }
    }
}

/// What a guide says about a value of a round chart.
fn guide_text(data: &ChartData, style: &PlotStyle, index: usize, value: f64) -> String {
    let category = style.category_labels == Some(LabelAt::Outside);
    let number = style.value_labels == Some(LabelAt::Outside);
    let name = data.categories.get(index).map(|c| (style.tick_format)(c)).unwrap_or_default();
    match (category, number) {
        (true, true) => format!("{name}  {}", (style.format)(value)),
        (true, false) => name,
        _ => (style.format)(value),
    }
}

fn center_text(style: &PlotStyle, center: Point, plot: &mut Plot) {
    let Some((value, caption)) = &style.center else { return };
    let big = style.size * CENTER_VALUE_SCALE;
    let mut number =
        centered(value.clone(), Point::new(center.x, center.y - style.size * 0.7), big, style.ink);
    number.bold = true;
    plot.labels.push(number);
    plot.labels.push(centered(
        caption.clone(),
        Point::new(center.x, center.y + big * 0.7),
        style.size,
        style.muted,
    ));
}

fn pie(data: &ChartData, rect: Rect, style: &PlotStyle, ruler: Ruler<'_>, plot: &mut Plot) {
    let rings: Vec<usize> = if style.concentric { (0..data.series.len()).collect() } else { vec![0] };
    if data.series.is_empty() {
        return;
    }
    let outside =
        style.value_labels == Some(LabelAt::Outside) || style.category_labels == Some(LabelAt::Outside);
    let guided = outside && style.label_guides;
    let room = if outside { style.size * 1.5 + OUTSIDE_LINE } else { 2.0 };
    let growth = if style.active.is_some() { ACTIVE_SLICE_GROWTH } else { 0.0 };
    let center = rect.center();
    let radius = if guided {
        // The width of the labels comes out of the sides, and the top and the bottom
        // only need a line of text.
        let first = data.series.get(rings[0]).map_or(&[][..], |s| s.values.as_slice());
        let texts: Vec<String> =
            first.iter().enumerate().map(|(i, v)| guide_text(data, style, i, *v)).collect();
        let wide = ruler.longest(texts.iter().map(String::as_str), style.size).ceil();
        let across = rect.width() / 2.0 - wide - GUIDE_OUT - GUIDE_RUN - GUIDE_GAP - 2.0;
        let tall = rect.height() / 2.0 - style.size * 0.7 - GUIDE_OUT;
        (across.min(tall) - growth).max(rect.width().min(rect.height()) / 6.0).max(1.0)
    } else {
        (rect.width().min(rect.height()) / 2.0 - room - growth).max(1.0)
    };
    let hole = radius * style.hole;
    let mut guides = Vec::new();
    let ring_width = (radius - hole) / rings.len() as f32;
    for (depth, s) in rings.iter().enumerate() {
        let series = &data.series[*s];
        let total: f64 = series.values.iter().filter(|v| **v > 0.0).sum();
        if total <= 0.0 {
            continue;
        }
        let outer = radius - ring_width * depth as f32;
        let inner = if rings.len() == 1 { hole } else { outer - ring_width + 1.0 };
        let mut angle = -FRAC_PI_2;
        for (i, value) in series.values.iter().enumerate() {
            if *value <= 0.0 {
                continue;
            }
            let share = *value / total;
            let end = angle + share as f32 * TAU;
            let color = style.item_color(style.color_of(i), i);
            let share_of_room = style.slice_radii.get(i).map_or(1.0, |r| r.clamp(SLICE_RADIUS_MIN, 1.0));
            let own = inner + (outer - inner) * share_of_room;
            let reach = if style.active == Some(i) { own + growth } else { own };
            let path = gapped_slice(center, inner, reach, angle, end, style.separator);
            plot.layers.push(fill(path, color));
            plot.sectors.push(Sector {
                series: *s,
                index: i,
                center,
                inner,
                outer: reach,
                from: angle,
                to: end,
                color,
            });
            let middle = (angle + end) / 2.0;
            let inside = if inner > 0.0 { (reach + inner) / 2.0 } else { reach * 0.62 };
            plot.marks.push(Mark {
                series: *s,
                index: i,
                at: Point::new(center.x + inside * middle.cos(), center.y + inside * middle.sin()),
                color,
            });
            if guided && depth == 0 {
                let text = guide_text(data, style, i, *value);
                guides.push(Guide { text, angle: middle, reach, color, share });
            }
            if share >= SLICE_LABEL_MIN {
                let category = data.categories.get(i).map(|c| (style.tick_format)(c)).unwrap_or_default();
                let number = (style.format)(*value);
                let mut write = |content: String, at: LabelAt| match at {
                    LabelAt::Outside if guided => {}
                    LabelAt::Outside => {
                        let (cos, sin) = (middle.cos(), middle.sin());
                        let from = Point::new(center.x + reach * cos, center.y + reach * sin);
                        let to = Point::new(
                            center.x + (reach + OUTSIDE_LINE) * cos,
                            center.y + (reach + OUTSIDE_LINE) * sin,
                        );
                        plot.layers.push(stroke(
                            vec![PathSegment::Move(from), PathSegment::Line(to)],
                            1.0,
                            with_alpha(color, 0.8),
                        ));
                        let wide = ruler.width(&content, style.size) + 6.0;
                        let tip = Point::new(to.x + cos * wide / 2.0, to.y + sin * style.size * 0.7);
                        plot.labels.push(centered(content, tip, style.size, style.ink));
                    }
                    _ => {
                        let at =
                            Point::new(center.x + inside * middle.cos(), center.y + inside * middle.sin());
                        plot.labels.push(centered(content, at, style.size, ink_over(color)));
                    }
                };
                if let Some(at) = style.value_labels {
                    write(number, at);
                }
                if let Some(at) = style.category_labels {
                    write(category, at);
                }
            }
            angle = end;
        }
    }
    place_guides(guides, center, rect, style, ruler, plot);
    center_text(style, center, plot);
}

fn circle_path(center: Point, radius: f32) -> Vec<PathSegment> {
    vector::ellipse(Rect::new(center.x - radius, center.y - radius, radius * 2.0, radius * 2.0))
}

fn polygon_path(points: &[Point]) -> Vec<PathSegment> {
    let mut path: Vec<PathSegment> = points
        .iter()
        .enumerate()
        .map(|(i, p)| if i == 0 { PathSegment::Move(*p) } else { PathSegment::Line(*p) })
        .collect();
    path.push(PathSegment::Close);
    path
}

fn radar(data: &ChartData, rect: Rect, style: &PlotStyle, plot: &mut Plot) {
    let count = data.categories.len();
    if count < 3 || data.series.is_empty() {
        return;
    }
    let size = style.size;
    let center = rect.center();
    let room = size * 1.6 + if style.category_axis { 6.0 } else { 0.0 };
    let radius = (rect.width().min(rect.height()) / 2.0 - room).max(1.0);
    let max = data
        .series
        .iter()
        .flat_map(|s| s.values.iter().copied())
        .filter(|v| v.is_finite())
        .fold(0.0_f64, f64::max);
    let (_, step, ticks) = nice_ticks(0.0, max.max(f64::EPSILON), TICKS);
    let high = (step * ticks.saturating_sub(1) as f64).max(f64::EPSILON);
    let angle_of = |i: usize| -FRAC_PI_2 + TAU * i as f32 / count as f32;
    let at = |r: f32, a: f32| Point::new(center.x + r * a.cos(), center.y + r * a.sin());
    let outline = |r: f32| -> Vec<PathSegment> {
        match style.polar_grid {
            PolarGrid::Circle => circle_path(center, r),
            _ => polygon_path(&(0..count).map(|i| at(r, angle_of(i))).collect::<Vec<_>>()),
        }
    };
    if style.polar_grid != PolarGrid::None {
        if style.polar_fill {
            plot.layers.push(fill(outline(radius), with_alpha(style.color_of(0), 0.2)));
        }
        let rings = style.rings.max(1);
        for ring in 1..=rings {
            let share = ring as f32 / rings as f32;
            plot.layers.push(stroke(outline(radius * share), 1.0, style.grid));
        }
        if style.spokes {
            for i in 0..count {
                let path = vec![PathSegment::Move(center), PathSegment::Line(at(radius, angle_of(i)))];
                plot.layers.push(stroke(path, 1.0, style.grid));
            }
        }
    }
    for (s, series) in data.series.iter().enumerate() {
        let color = if series.color == Color::TRANSPARENT { style.color_of(s) } else { series.color };
        let points: Vec<Point> = (0..count)
            .map(|i| {
                let value = series.values.get(i).copied().filter(|v| v.is_finite()).unwrap_or(0.0);
                at(radius * (value / high) as f32, angle_of(i))
            })
            .collect();
        let path = polygon_path(&points);
        if style.lines_only {
            plot.layers.push(stroke(path, 2.0, color));
        } else {
            plot.layers.push(Layer::Shape(Shape {
                path,
                fill: Some(with_alpha(color, style.fill_opacity)),
                stroke: Some((1.5, color)),
            }));
        }
        for (i, point) in points.iter().enumerate() {
            plot.marks.push(Mark { series: s, index: i, at: *point, color });
            if style.dots {
                plot.layers.push(dot(*point, DOT_RADIUS, color));
            }
        }
    }
    if style.radius_axis {
        let angle = -FRAC_PI_2 + TAU / 12.0;
        for tick in 1..ticks {
            let value = step * tick as f64;
            let spot = at(radius * (value / high) as f32, angle);
            plot.labels.push(centered((style.format)(value), spot, size * 0.9, style.muted));
        }
    }
    let half = TAU / count as f32 / 2.0;
    for i in 0..count {
        let angle = angle_of(i);
        if style.category_axis {
            let name = (style.tick_format)(&data.categories[i]);
            let lines = name.lines().count().max(1) as f32;
            let wide = name.lines().map(|l| l.chars().count()).max().unwrap_or(0) as f32 * size * CHAR_WIDTH;
            let spot = Point::new(
                center.x + (radius + 6.0) * angle.cos() + angle.cos() * wide / 2.0,
                center.y + (radius + 6.0 + size * lines * 0.5) * angle.sin(),
            );
            let mut label = centered(name, spot, size, style.muted);
            label.rect = Rect::new(
                spot.x - label.rect.width() / 2.0,
                spot.y - size * lines,
                label.rect.width(),
                size * lines * 2.0,
            );
            plot.labels.push(label);
        }
        // The pointer anywhere around a spoke means that category.
        plot.sectors.push(Sector {
            series: 0,
            index: i,
            center,
            inner: 0.0,
            outer: radius + room,
            from: angle - half,
            to: angle + half,
            color: Color::TRANSPARENT,
        });
    }
}

fn radial(data: &ChartData, rect: Rect, style: &PlotStyle, plot: &mut Plot) {
    // An arc over its center, like a gauge, has room to sit lower and grow.
    let gauge = style.sweep <= 180.0 && (style.start_angle + 90.0).abs() < f32::EPSILON;
    let center_y_offset = if gauge { rect.height() * 0.25 } else { 0.0 };
    let center = Point::new(rect.center().x, rect.center().y + center_y_offset);
    let reach = if gauge {
        (rect.width() / 2.0).min(rect.height() * 0.75)
    } else {
        rect.width().min(rect.height()) / 2.0
    };
    let radius = (reach - 4.0).max(1.0);
    let hole = radius * if style.hole > 0.0 { style.hole } else { 0.45 };
    let start = style.start_angle.to_radians() - FRAC_PI_2;
    let sweep = style.sweep.to_radians();
    let stacking = style.stack != Stack::None;
    let Some(first) = data.series.first() else { return };
    let at = |r: f32, a: f32| Point::new(center.x + r * a.cos(), center.y + r * a.sin());

    if stacking {
        // Every series is a piece of the same ring.
        let width = radius - hole;
        let middle = hole + width / 2.0;
        let total: f64 = data
            .series
            .iter()
            .map(|s| s.values.first().copied().filter(|v| v.is_finite() && *v > 0.0).unwrap_or(0.0))
            .sum();
        if total <= 0.0 {
            return;
        }
        let mut angle = start;
        // Each round end reaches into the next piece, and the first one is on top.
        let pieces: Vec<(usize, f32, f32, Color)> = data
            .series
            .iter()
            .enumerate()
            .map(|(s, series)| {
                let value = series.values.first().copied().filter(|v| v.is_finite()).unwrap_or(0.0);
                let end = angle + (value / total) as f32 * sweep;
                let piece = (
                    s,
                    angle,
                    end,
                    if series.color == Color::TRANSPARENT { style.color_of(s) } else { series.color },
                );
                angle = end;
                piece
            })
            .collect();
        for (s, from, to, color) in &pieces {
            let mut path = vec![PathSegment::Move(at(middle, *from))];
            arc(&mut path, center, middle, *from, *to);
            plot.layers.push(stroke(path, width - 2.0, *color));
            plot.sectors.push(Sector {
                series: *s,
                index: 0,
                center,
                inner: hole,
                outer: radius,
                from: *from,
                to: *to,
                color: *color,
            });
            let mid = (*from + *to) / 2.0;
            plot.marks.push(Mark { series: *s, index: 0, at: at(middle, mid), color: *color });
        }
        center_text(style, center, plot);
        return;
    }

    let count = first.values.len().max(1);
    let max = first.values.iter().copied().filter(|v| v.is_finite()).fold(0.0_f64, f64::max);
    if max <= 0.0 && !style.tracks {
        return;
    }
    let max = max.max(f64::EPSILON);
    let width = ((radius - hole) - RING_GAP * (count - 1) as f32) / count as f32;
    for (i, value) in first.values.iter().enumerate() {
        let middle = radius - (width + RING_GAP) * i as f32 - width / 2.0;
        let (inner, outer) = (middle - width / 2.0, middle + width / 2.0);
        if style.polar_grid != PolarGrid::None {
            plot.layers.push(stroke(circle_path(center, outer + RING_GAP / 2.0), 1.0, style.grid));
        }
        if style.tracks {
            // The track goes all the way around, unless the chart is a gauge.
            let around = if gauge { sweep } else { TAU };
            let mut path = vec![PathSegment::Move(at(middle, start))];
            arc(&mut path, center, middle, start, start + around);
            plot.layers.push(stroke(path, width, style.track));
        }
        let value = if value.is_finite() { value.max(0.0) } else { 0.0 };
        let share = (value / max) as f32;
        let end = start + share * sweep;
        let color = style.item_color(style.color_of(i), i);
        // Nothing yet is no ring at all: a round cap alone would look like a value.
        if share > 0.0 {
            let mut path = vec![PathSegment::Move(at(middle, start))];
            arc(&mut path, center, middle, start, end);
            plot.layers.push(stroke(path, width, color));
        }
        plot.sectors.push(Sector {
            series: 0,
            index: i,
            center,
            inner,
            outer,
            from: start,
            to: start + sweep,
            color,
        });
        plot.marks.push(Mark { series: 0, index: i, at: at(middle, end), color });
        if style.category_labels.is_some() || style.value_labels.is_some() {
            let name = data.categories.get(i).map(|c| (style.tick_format)(c)).unwrap_or_default();
            let content = if style.category_labels.is_some() { name } else { (style.format)(value) };
            // Before its start, where a ring shorter than a whole turn leaves room.
            let spot = at(middle, start);
            let wide = content.chars().count() as f32 * style.size * CHAR_WIDTH + 8.0;
            plot.labels.push(text(
                content,
                Rect::new(spot.x - wide - width / 2.0 - 4.0, spot.y - width / 2.0, wide, width),
                LabelAlign::Right,
                style.size,
                style.muted,
            ));
        }
    }
    center_text(style, center, plot);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chart::SeriesData;

    const AREA: Rect = Rect::new(0.0, 0.0, 400.0, 300.0);
    const CLEAR: Color = Color::TRANSPARENT;

    fn data(series: &[(&str, &[f64])]) -> ChartData {
        ChartData {
            categories: (0..series[0].1.len()).map(|i| format!("C{i}")).collect(),
            positions: Vec::new(),
            series: series
                .iter()
                .map(|(name, values)| SeriesData {
                    name: (*name).to_owned(),
                    color: CLEAR,
                    values: values.to_vec(),
                })
                .collect(),
        }
    }

    fn rects_of(plot: &Plot) -> usize {
        plot.layers.iter().filter(|l| matches!(l, Layer::Shape(s) if s.fill.is_some())).count()
    }

    #[test]
    fn measured_labels_set_the_room_they_take() {
        let data = data(&[("A", &[1000.0, 2000.0])]);
        let style = PlotStyle { value_axis: true, ..PlotStyle::new(ChartKind::Bar) };
        let area = |width: f32| draw_measured(&data, AREA, &style, &|_, _| width).area.x();
        assert!(area(80.0) > area(10.0) + 60.0);
    }

    #[test]
    fn a_measured_label_that_would_stick_out_stays_in() {
        let mut data = data(&[("A", &[1.0, 2.0])]);
        data.categories = vec!["A long name for a point".into(), "Another long one".into()];
        let style = PlotStyle::new(ChartKind::Line);
        let plot = draw_measured(&data, AREA, &style, &|t, s| t.len() as f32 * s * 0.5);
        assert_eq!(plot.labels[0].align, LabelAlign::Left);
        assert_eq!(plot.labels[0].rect.x(), AREA.x());
        assert_eq!(plot.labels[1].align, LabelAlign::Right);
        assert_eq!(plot.labels[1].rect.max_x(), AREA.max_x());
    }

    #[test]
    fn the_ends_can_line_up_with_the_ends_of_a_line() {
        let data = data(&[("A", &[1.0, 2.0, 3.0])]);
        let style = PlotStyle { ends_aligned: true, ..PlotStyle::new(ChartKind::Line) };
        let plot = draw_measured(&data, AREA, &style, &|t, s| t.len() as f32 * s * 0.5);
        let (first, last) = (&plot.labels[0], &plot.labels[2]);
        assert_eq!(first.align, LabelAlign::Left);
        assert_eq!(first.rect.x(), plot.area.x());
        assert_eq!(last.align, LabelAlign::Right);
        assert!((last.rect.max_x() - plot.area.max_x()).abs() < 0.01);
        assert_eq!(plot.labels[1].align, LabelAlign::Center);
    }

    #[test]
    fn with_its_ends_lined_up_the_last_category_shows_even_if_it_was_to_be_skipped() {
        let values = [1.0; 40];
        let data = data(&[("A", &values)]);
        let style = PlotStyle { ends_aligned: true, ..PlotStyle::new(ChartKind::Line) };
        let plot = draw_measured(&data, AREA, &style, &|t, s| t.len() as f32 * s * 0.5);
        assert_eq!(plot.labels.last().map(|l| l.text.as_str()), Some("C39"));
        assert!(plot.labels.len() < 40);
        for pair in plot.labels.windows(2) {
            let (a, b) = (&pair[0], &pair[1]);
            assert!(a.text != b.text);
        }
    }

    #[test]
    fn a_fixed_domain_is_the_axis_and_cuts_off_what_is_past_it() {
        let lines = data(&[("A", &[10.0, 500.0, 30.0])]);
        let style =
            PlotStyle { domain: Some((13.0, 87.0)), value_axis: true, ..PlotStyle::new(ChartKind::Line) };
        let plot = draw(&lines, AREA, &style);
        assert!(plot.layers.iter().any(|l| matches!(l, Layer::Clipped { .. })));
        assert_eq!(plot.marks.len(), 1, "only the value inside has a place");
        let ticks: Vec<&str> = plot
            .labels
            .iter()
            .map(|l| l.text.as_str())
            .filter(|t| t.starts_with(['2', '4', '6', '8']))
            .collect();
        assert_eq!(ticks, ["20", "40", "60", "80"], "round numbers, as far as they fit");

        let bars = data(&[("A", &[5.0, 50.0, 500.0])]);
        let style = PlotStyle { domain: Some((20.0, 100.0)), ..PlotStyle::new(ChartKind::Bar) };
        let plot = draw(&bars, AREA, &style);
        assert_eq!(plot.bars.len(), 2, "the bar short of the axis is gone");
        assert_eq!(plot.bars[1].radii, [0.0; 4], "cut off at both ends, it has no round corners");
        let tall = plot.bars[1].rect;
        assert!((tall.y() - plot.area.y()).abs() < 0.01 && (tall.max_y() - plot.area.max_y()).abs() < 0.01);
    }

    #[test]
    fn with_no_domain_nothing_is_clipped() {
        let lines = data(&[("A", &[10.0, 500.0, 30.0])]);
        let plot = draw(&lines, AREA, &PlotStyle::new(ChartKind::Line));
        assert!(!plot.layers.iter().any(|l| matches!(l, Layer::Clipped { .. })));
        assert_eq!(plot.marks.len(), 3);
    }

    #[test]
    fn a_line_of_reference_stretches_the_scale_unless_the_domain_is_fixed() {
        let values = data(&[("A", &[10.0, 20.0])]);
        let reference = Reference::new(100.0, Some("Target".into()), None);
        let style = PlotStyle {
            rules_apart: true,
            references: vec![reference.clone()],
            ..PlotStyle::new(ChartKind::Line)
        };
        let plot = draw(&values, AREA, &style);
        assert!(plot.top_rules.len() > 10, "in dashes");
        assert!(plot.labels.iter().any(|l| l.text == "Target"));

        let fixed = PlotStyle { domain: Some((0.0, 50.0)), references: vec![reference], ..style };
        let plot = draw(&values, AREA, &fixed);
        assert!(plot.top_rules.is_empty(), "past the end of a fixed axis");
        assert!(!plot.labels.iter().any(|l| l.text == "Target"));
    }

    #[test]
    fn a_dashed_grid_has_gaps_and_keeps_to_the_area() {
        let data = data(&[("A", &[1.0, 2.0])]);
        let solid = PlotStyle { rules_apart: true, ..PlotStyle::new(ChartKind::Bar) };
        let solid_count = draw(&data, AREA, &solid).rules.len();
        let dashed = PlotStyle { rules_apart: true, grid_dashed: true, ..PlotStyle::new(ChartKind::Bar) };
        let plot = draw(&data, AREA, &dashed);
        assert!(plot.rules.len() > solid_count * 10);
        assert!(plot.rules.iter().all(|r| r.rect.max_x() <= plot.area.max_x() + 0.01));
    }

    #[test]
    fn the_axis_can_have_more_steps_and_the_grid_its_own_columns() {
        let data = data(&[("A", &[10.0, 100.0])]);
        let rules = |style: PlotStyle| {
            let style = PlotStyle { rules_apart: true, ..style };
            draw(&data, AREA, &style).rules
        };
        let few = rules(PlotStyle::new(ChartKind::Line));
        let many = rules(PlotStyle { value_ticks: 10, ..PlotStyle::new(ChartKind::Line) });
        assert!(many.len() > few.len(), "{} {}", many.len(), few.len());
        let columns = rules(PlotStyle { grid_columns: Some(5), ..PlotStyle::new(ChartKind::Line) });
        let upright = columns.iter().filter(|r| r.rect.height() > r.rect.width()).count();
        assert_eq!(upright, 6, "the ends of five columns");
    }

    #[test]
    fn gridlines_stay_in_the_layers_unless_asked_otherwise() {
        let data = data(&[("A", &[1.0, 2.0, 3.0])]);
        let page = draw(&data, AREA, &PlotStyle::new(ChartKind::Bar));
        assert!(page.rules.is_empty());
        let screen = PlotStyle { rules_apart: true, ..PlotStyle::new(ChartKind::Bar) };
        let screen = draw(&data, AREA, &screen);
        assert!(!screen.rules.is_empty());
        assert_eq!(rects_of(&page), rects_of(&screen) + screen.rules.len(), "the same lines, elsewhere");
        assert_eq!(screen.bars.len(), 3);
    }

    fn line_length_on_rule(rules: &[Rule], y: f32, from: f32, to: f32) -> f32 {
        rules
            .iter()
            .filter(|r| r.rect.center().y == y)
            .map(|r| (r.rect.max_x().min(to) - r.rect.x().max(from)).max(0.0))
            .sum()
    }

    #[test]
    fn bars_apart_leave_the_layers_and_the_lines_behind_them() {
        let data = data(&[("A", &[10.0, 20.0])]);
        let page = PlotStyle { rules_apart: true, ..PlotStyle::new(ChartKind::Bar) };
        let whole = draw(&data, AREA, &page);
        assert_eq!(whole.layers.len(), 2, "on a page bars are shapes");
        let screen = PlotStyle { bars_apart: true, bar_radius: 0.0, ..page };
        let plot = draw(&data, AREA, &screen);
        assert!(plot.layers.is_empty());
        assert_eq!(plot.bars.len(), 2);
        let bar = plot.bars[1].rect;
        for rule in plot.rules.iter().filter(|r| r.rect.center().y > bar.y()) {
            let (y, a, b) = (rule.rect.center().y, bar.x() + 0.01, bar.max_x() - 0.01);
            if y < bar.max_y() {
                assert_eq!(line_length_on_rule(&plot.rules, y, a, b), 0.0, "nothing behind the bar");
            }
        }
        assert!(plot.rules.len() > whole.rules.len(), "they go on past the bar");
    }

    #[test]
    fn a_line_near_a_round_corner_follows_it() {
        assert_eq!(corner_inset(0.0, 1.0), 0.0);
        assert_eq!(corner_inset(4.0, 4.0), 0.0);
        assert!((corner_inset(4.0, 0.0) - 4.0).abs() < 0.001);
        let bar = Bar {
            series: 0,
            index: 0,
            rect: Rect::new(10.0, 10.0, 20.0, 40.0),
            color: CLEAR,
            radii: [4.0, 4.0, 0.0, 0.0],
            gradient: None,
        };
        let mut out = Vec::new();
        line_around_bar(Rect::new(0.0, 10.5, 40.0, 1.0), &bar, &mut out);
        assert!(out[0].max_x() > 11.0, "it reaches the curve: {out:?}");
        let mut out = Vec::new();
        line_around_bar(Rect::new(0.0, 47.5, 40.0, 1.0), &bar, &mut out);
        assert_eq!(out[0].max_x(), 10.0, "a square corner has nothing to follow");
    }

    #[test]
    fn a_line_with_more_points_than_columns_keeps_only_the_extremes() {
        let values: Vec<f64> =
            (0..20_000).map(|i| if i == 7_777 { 1000.0 } else { f64::from(i % 50) }).collect();
        let many = data(&[("A", &values)]);
        let plot = draw(&many, AREA, &PlotStyle { rules_apart: true, ..PlotStyle::new(ChartKind::Line) });
        let Some(Layer::Shape(shape)) = plot.layers.first() else { panic!("a line") };
        assert!(shape.path.len() <= AREA.width() as usize * 2 + 4, "{}", shape.path.len());
        assert!(shape.path.iter().all(|s| !matches!(s, PathSegment::Cubic(..))), "straight lines");
        let top = shape
            .path
            .iter()
            .filter_map(|s| match s {
                PathSegment::Line(p) | PathSegment::Move(p) => Some(p.y),
                _ => None,
            })
            .fold(f32::INFINITY, f32::min);
        let peak = plot.marks[7_777].at.y;
        assert!((top - peak).abs() < 0.01, "the one peak survives: {top} {peak}");
        assert_eq!(plot.marks.len(), 20_000, "and every value still has its place");
    }

    #[test]
    fn categories_closer_than_a_pixel_do_not_pile_up_on_the_axis() {
        let values = vec![1.0; 5000];
        let many = data(&[("A", &values)]);
        let plot =
            draw_measured(&many, AREA, &PlotStyle::new(ChartKind::Line), &|t, s| t.len() as f32 * s * 0.5);
        assert!(plot.labels.len() < 20, "{}", plot.labels.len());
    }

    #[test]
    fn a_line_that_fits_keeps_its_curve() {
        let plot = draw(
            &data(&[("A", &[1.0, 3.0, 2.0])]),
            AREA,
            &PlotStyle { rules_apart: true, ..PlotStyle::new(ChartKind::Line) },
        );
        let Some(Layer::Shape(shape)) = plot.layers.first() else { panic!("a line") };
        assert!(shape.path.iter().any(|s| matches!(s, PathSegment::Cubic(..))));
    }

    fn bars_of(style: PlotStyle, series: &[(&str, &[f64])]) -> Plot {
        draw(&data(series), AREA, &style)
    }

    #[test]
    fn a_tower_rounds_whatever_is_on_its_ends() {
        let style = PlotStyle {
            stack: Stack::Normal,
            bars_apart: true,
            bar_radius: 4.0,
            ..PlotStyle::new(ChartKind::Bar)
        };
        let plot = bars_of(style, &[("a", &[5.0, 5.0]), ("b", &[5.0, 5.0]), ("c", &[5.0, 0.0])]);
        let of =
            |s: usize, i: usize| plot.bars.iter().find(|b| b.series == s && b.index == i).map(|b| b.radii);
        assert_eq!(of(2, 0), Some([4.0, 4.0, 0.0, 0.0]), "the top of the first tower");
        assert_eq!(of(1, 0), Some([0.0; 4]), "the middle");
        assert_eq!(of(1, 1), Some([4.0, 4.0, 0.0, 0.0]), "with nothing above it, the second is the top");
        assert_eq!(of(2, 1), None, "and the empty one is not drawn");
        assert_eq!(of(0, 1), Some([0.0, 0.0, 4.0, 4.0]), "the bottom");
    }

    #[test]
    fn a_tower_under_zero_rounds_its_lower_end() {
        let style = PlotStyle {
            stack: Stack::Normal,
            bars_apart: true,
            bar_radius: 4.0,
            ..PlotStyle::new(ChartKind::Bar)
        };
        let plot = bars_of(style, &[("a", &[-5.0]), ("b", &[-5.0])]);
        assert_eq!(plot.bars[1].radii, [0.0, 0.0, 4.0, 4.0], "the tip is the lowest");
        assert_eq!(plot.bars[0].radii, [4.0, 4.0, 0.0, 0.0]);
    }

    #[test]
    fn a_short_bar_reaches_the_length_asked_for_from_its_base() {
        let style = PlotStyle { min_bar: 6.0, bars_apart: true, ..PlotStyle::new(ChartKind::Bar) };
        let plot = bars_of(style, &[("a", &[0.001, 100.0, 0.0])]);
        let (tiny, tall, none) = (plot.bars[0].rect, plot.bars[1].rect, plot.bars[2].rect);
        assert!((tiny.height() - 6.0).abs() < 0.01);
        assert!((tiny.max_y() - tall.max_y()).abs() < 0.01, "both stand on the same line");
        assert!(none.height() <= 0.5, "nothing is not made to look like something");
    }

    #[test]
    fn bars_can_hang_from_the_top_or_come_from_the_right() {
        let hanging = PlotStyle { reversed: true, bars_apart: true, ..PlotStyle::new(ChartKind::Bar) };
        let plot = bars_of(hanging, &[("a", &[10.0, 20.0])]);
        let (short, long) = (plot.bars[0].rect, plot.bars[1].rect);
        assert!((short.y() - long.y()).abs() < 0.01, "from the same line");
        assert!(long.max_y() > short.max_y());
        assert!(plot.labels.iter().all(|l| l.rect.y() < plot.area.y()), "with the categories over them");

        let from_right = PlotStyle {
            reversed: true,
            horizontal: true,
            bars_apart: true,
            ..PlotStyle::new(ChartKind::Bar)
        };
        let plot = bars_of(from_right, &[("a", &[10.0, 20.0])]);
        let (short, long) = (plot.bars[0].rect, plot.bars[1].rect);
        assert!((short.max_x() - long.max_x()).abs() < 0.01, "from the same line");
        assert!(long.x() < short.x());
        assert!(
            plot.labels.iter().all(|l| l.rect.x() > plot.area.max_x()),
            "with the categories beside them"
        );
    }

    #[test]
    fn a_label_on_the_line_of_zero_keeps_off_the_bar() {
        let style = PlotStyle { labels_at_zero: true, ..PlotStyle::new(ChartKind::Bar) };
        let plot = bars_of(PlotStyle { bars_apart: true, ..style }, &[("a", &[10.0, -10.0])]);
        let name = |n: &str| plot.labels.iter().find(|l| l.text == n).map(|l| l.rect);
        let (up, down) = (name("C0").expect("a label"), name("C1").expect("a label"));
        let zero = plot.bars[0].rect.max_y();
        assert!(up.y() >= zero, "under the line, as its bar goes up");
        assert!(down.max_y() <= zero, "over the line, as its bar goes down");
    }

    #[test]
    fn a_gradient_goes_from_the_tip_to_the_base_either_way() {
        let style = PlotStyle { gradient: true, bars_apart: true, ..PlotStyle::new(ChartKind::Bar) };
        let plot = bars_of(style, &[("a", &[10.0, -10.0])]);
        let up = plot.bars[0].gradient.expect("a gradient");
        let down = plot.bars[1].gradient.expect("a gradient");
        assert!(up.start.y < up.end.y, "strong at the top");
        assert!(down.start.y > down.end.y, "strong at the bottom");
        assert!(up.to.a < up.from.a);
        let plain = PlotStyle { bars_apart: true, ..PlotStyle::new(ChartKind::Bar) };
        assert_eq!(bars_of(plain, &[("a", &[10.0])]).bars[0].gradient, None);
    }

    fn small(kind: ChartKind, data: &ChartData) -> Plot {
        let format = |value: f64| value.to_string();
        let options = ChartOptions {
            kind,
            title: "",
            legend: false,
            size: 12.0,
            ink: Color::BLACK,
            muted: Color::BLACK,
            grid: Color::BLACK,
            format: &format,
        };
        draw_small(data, AREA, &options, &ChartStyle::default())
    }

    #[test]
    fn a_sparkline_says_where_its_points_are_and_draws_the_same() {
        let values = data(&[("a", &[1.0, f64::NAN, 3.0, 2.0])]);
        let plot = small(ChartKind::Sparkline, &values);
        let indices: Vec<usize> = plot.marks.iter().map(|m| m.index).collect();
        assert_eq!(indices, [0, 2, 3], "a value that is missing has no place");
        assert_eq!(plot.hit(Point::new(1.0, 150.0)).map(|h| h.index), Some(0));
        assert_eq!(plot.hit(Point::new(399.0, 150.0)).map(|h| h.index), Some(3));
        assert_eq!(plot.categories(), 4);
        let format = |value: f64| value.to_string();
        let options = ChartOptions {
            kind: ChartKind::Sparkline,
            title: "",
            legend: false,
            size: 12.0,
            ink: Color::BLACK,
            muted: Color::BLACK,
            grid: Color::BLACK,
            format: &format,
        };
        let same = crate::chart::draw_styled(&values, AREA, &options, &ChartStyle::default());
        assert_eq!(plot.layers.len(), same.shapes.len());
    }

    #[test]
    fn the_dots_of_a_scatter_chart_own_the_strips_around_them_whatever_their_order() {
        let mut values = data(&[("a", &[1.0, 2.0, 3.0])]);
        values.positions = vec![30.0, 10.0, 20.0];
        let plot = small(ChartKind::Scatter, &values);
        assert_eq!(plot.marks.len(), 3);
        let left = plot.marks[1].at;
        assert!(left.x < plot.marks[2].at.x && plot.marks[2].at.x < plot.marks[0].at.x);
        assert_eq!(plot.hit(Point::new(left.x, 10.0)).map(|h| h.index), Some(1));
    }

    fn prices() -> ChartData {
        data(&[
            ("open", &[100.0, 110.0, 105.0, f64::NAN]),
            ("high", &[112.0, 111.0, 108.0, 105.0]),
            ("low", &[98.0, 101.0, 104.0, 105.0]),
            ("close", &[110.0, 102.0, 105.0, 105.0]),
        ])
    }

    #[test]
    fn a_candle_goes_from_open_to_close_with_wicks_to_its_extremes() {
        let style = PlotStyle {
            bars_apart: true,
            rules_apart: true,
            value_axis: true,
            ..PlotStyle::new(ChartKind::Candlestick)
        };
        let plot = draw(&prices(), AREA, &style);
        assert!(plot.layers.is_empty());
        assert_eq!(plot.candles.len(), 3, "the one with a missing value is left out");
        let (up, down) = (plot.candles[0], plot.candles[1]);
        assert!(up.rising && !down.rising);
        assert_ne!(up.color, down.color);
        assert!(up.wicks[0].y() < up.body.y(), "the high is over the body");
        assert!(up.wicks[1].max_y() > up.body.max_y(), "the low is under it");
        assert!((up.wicks[0].max_y() - up.body.y()).abs() < 0.01);
        assert!(plot.candles[2].body.height() >= 1.0, "a price that did not move still shows");
        assert!(plot.labels.iter().any(|l| l.text == "100"), "round numbers on the axis");
        assert!(!plot.labels.iter().any(|l| l.text == "0"), "and it does not go down to zero");
    }

    #[test]
    fn candles_need_their_four_series_and_do_not_stack() {
        let style = PlotStyle::new(ChartKind::Candlestick);
        assert!(draw(&data(&[("open", &[1.0])]), AREA, &style).layers.is_empty());
        let stacked =
            PlotStyle { stack: Stack::Normal, bars_apart: true, ..PlotStyle::new(ChartKind::Candlestick) };
        let plain = PlotStyle { bars_apart: true, ..PlotStyle::new(ChartKind::Candlestick) };
        assert_eq!(draw(&prices(), AREA, &stacked).candles, draw(&prices(), AREA, &plain).candles);
    }

    #[test]
    fn a_candle_past_a_fixed_axis_is_cut_off_or_gone() {
        let style = PlotStyle {
            bars_apart: true,
            domain: Some((103.0, 109.0)),
            ..PlotStyle::new(ChartKind::Candlestick)
        };
        let plot = draw(&prices(), AREA, &style);
        assert_eq!(plot.candles.len(), 3);
        assert!(plot.candles[0].wicks[0].y() >= plot.area.y() - 0.01);
        let gone = PlotStyle {
            bars_apart: true,
            domain: Some((200.0, 300.0)),
            ..PlotStyle::new(ChartKind::Candlestick)
        };
        assert!(draw(&prices(), AREA, &gone).candles.is_empty());
    }

    #[test]
    fn a_page_gets_candles_as_shapes_through_the_chart_module() {
        let format = |value: f64| value.to_string();
        let options = ChartOptions {
            kind: ChartKind::Candlestick,
            title: "",
            legend: false,
            size: 12.0,
            ink: Color::BLACK,
            muted: Color::BLACK,
            grid: Color::BLACK,
            format: &format,
        };
        let drawing = crate::chart::draw(&prices(), AREA, &options);
        assert!(drawing.shapes.len() > 9, "{}", drawing.shapes.len());
        assert!(drawing.labels.iter().any(|l| l.text == "100"));
    }

    #[test]
    fn a_natural_curve_goes_through_every_point() {
        let points =
            [Point::new(0.0, 0.0), Point::new(10.0, 8.0), Point::new(20.0, 3.0), Point::new(30.0, 9.0)];
        let path = curve_through(&points, Curve::Natural);
        assert_eq!(path.len(), 3);
        for (segment, point) in path.iter().zip(&points[1..]) {
            let PathSegment::Cubic(_, _, end) = segment else { panic!("a curve") };
            assert_eq!(end, point);
        }
    }

    #[test]
    fn a_monotone_curve_never_passes_the_points() {
        let points =
            [Point::new(0.0, 0.0), Point::new(10.0, 0.0), Point::new(20.0, 10.0), Point::new(30.0, 10.0)];
        for segment in curve_through(&points, Curve::Monotone) {
            let PathSegment::Cubic(a, b, _) = segment else { panic!("a curve") };
            assert!((0.0..=10.0).contains(&a.y) && (0.0..=10.0).contains(&b.y), "{a:?} {b:?}");
        }
    }

    #[test]
    fn a_step_changes_halfway() {
        let points = [Point::new(0.0, 0.0), Point::new(10.0, 8.0)];
        let path = curve_through(&points, Curve::Step);
        assert_eq!(path[0], PathSegment::Line(Point::new(5.0, 0.0)));
        assert_eq!(path[1], PathSegment::Line(Point::new(5.0, 8.0)));
    }

    #[test]
    fn stacked_series_start_where_the_last_one_ended() {
        let towers = stacked(&data(&[("a", &[1.0, 2.0]), ("b", &[3.0, 4.0])]), Stack::Normal);
        assert_eq!(towers[0], [(0.0, 1.0), (0.0, 2.0)]);
        assert_eq!(towers[1], [(1.0, 4.0), (2.0, 6.0)]);
    }

    #[test]
    fn expanded_series_always_add_up_to_the_whole() {
        let towers = stacked(&data(&[("a", &[1.0, 30.0]), ("b", &[3.0, 10.0])]), Stack::Expand);
        assert!((towers[1][0].1 - 1.0).abs() < 1e-9);
        assert!((towers[1][1].1 - 1.0).abs() < 1e-9);
        assert!((towers[0][1].1 - 0.75).abs() < 1e-9);
    }

    #[test]
    fn a_bar_below_zero_goes_the_other_way_in_its_own_color() {
        let negative = Color::rgba(1.0, 0.0, 0.0, 1.0);
        let mut style = PlotStyle::new(ChartKind::Bar);
        style.negative = Some(negative);
        let plot = draw(&data(&[("a", &[5.0, -5.0])]), AREA, &style);
        let up = plot.marks.iter().find(|m| m.index == 0).expect("a mark");
        let down = plot.marks.iter().find(|m| m.index == 1).expect("a mark");
        assert!(up.at.y < down.at.y);
        assert_eq!(down.color, negative);
        assert_ne!(up.color, negative);
    }

    #[test]
    fn horizontal_bars_grow_to_the_right() {
        let mut style = PlotStyle::new(ChartKind::Bar);
        style.horizontal = true;
        let plot = draw(&data(&[("a", &[2.0, 8.0])]), AREA, &style);
        assert!(plot.marks[1].at.x > plot.marks[0].at.x);
        assert!(plot.marks[1].at.y > plot.marks[0].at.y, "the first category is at the top");
    }

    #[test]
    fn side_by_side_bars_do_not_overlap() {
        let plot = draw(&data(&[("a", &[5.0]), ("b", &[5.0])]), AREA, &PlotStyle::new(ChartKind::Bar));
        assert!(plot.marks[1].at.x - plot.marks[0].at.x > 10.0);
    }

    #[test]
    fn the_pointer_finds_the_category_of_its_strip() {
        let plot = draw(&data(&[("a", &[1.0, 2.0, 3.0, 4.0])]), AREA, &PlotStyle::new(ChartKind::Bar));
        assert_eq!(plot.hit(Point::new(20.0, 100.0)).map(|h| h.index), Some(0));
        assert_eq!(plot.hit(Point::new(380.0, 100.0)).map(|h| h.index), Some(3));
        assert_eq!(plot.categories(), 4);
    }

    #[test]
    fn the_pointer_finds_the_slice_it_is_on() {
        let plot = draw(&data(&[("a", &[1.0, 1.0])]), AREA, &PlotStyle::new(ChartKind::Doughnut));
        let center = AREA.center();
        // Clockwise from the top: the first half is on the right.
        assert_eq!(plot.hit(Point::new(center.x + 100.0, center.y)).map(|h| h.index), Some(0));
        assert_eq!(plot.hit(Point::new(center.x - 100.0, center.y)).map(|h| h.index), Some(1));
        assert_eq!(plot.hit(center), None, "the hole is nothing");
    }

    #[test]
    fn the_active_slice_comes_forward() {
        let mut style = PlotStyle::new(ChartKind::Pie);
        style.active = Some(1);
        let plot = draw(&data(&[("a", &[1.0, 1.0])]), AREA, &style);
        assert!(plot.sectors[1].outer > plot.sectors[0].outer);
    }

    fn image_alpha(image: &ravenrs_core::ImageData, x: u32, y: u32) -> u8 {
        image.pixels()[((y * image.width() + x) * 4 + 3) as usize]
    }

    #[test]
    fn the_gap_between_slices_is_really_empty() {
        let plot = draw(&data(&[("a", &[1.0, 1.0])]), AREA, &PlotStyle::new(ChartKind::Pie));
        let image = vector::rasterize_layers(&plot.layers, AREA, 1.0).expect("a picture");
        let center = AREA.center();
        // The two halves meet along the upright line through the center.
        assert_eq!(image_alpha(&image, center.x as u32, (center.y - 60.0) as u32), 0);
        assert_eq!(image_alpha(&image, center.x as u32 + 40, (center.y - 60.0) as u32), 255);
    }

    #[test]
    fn a_whole_circle_has_no_gap() {
        let plot = draw(&data(&[("a", &[1.0])]), AREA, &PlotStyle::new(ChartKind::Pie));
        let image = vector::rasterize_layers(&plot.layers, AREA, 1.0).expect("a picture");
        let center = AREA.center();
        assert_eq!(image_alpha(&image, center.x as u32, (center.y - 60.0) as u32), 255);
    }

    #[test]
    fn each_slice_can_have_its_own_radius() {
        let style = PlotStyle { slice_radii: vec![1.0, 0.5], ..PlotStyle::new(ChartKind::Doughnut) };
        let plot = draw(&data(&[("a", &[1.0, 1.0, 1.0])]), AREA, &style);
        let (a, b, c) = (plot.sectors[0], plot.sectors[1], plot.sectors[2]);
        assert!(b.outer < a.outer);
        assert!(b.outer > b.inner, "it keeps going from the hole");
        assert_eq!(c.outer, a.outer, "with no number it reaches the edge");
    }

    #[test]
    fn guided_labels_never_cover_each_other_and_stay_in_the_chart() {
        // Small slices side by side, which straight lines would pile up.
        let values = [40.0, 1.5, 1.5, 1.5, 1.5, 1.5, 40.0, 1.5, 1.5, 1.5, 1.5, 1.5];
        let mut many = data(&[("a", &values)]);
        many.categories = (0..values.len()).map(|i| format!("Category {i}")).collect();
        let style = PlotStyle {
            category_labels: Some(LabelAt::Outside),
            value_labels: Some(LabelAt::Outside),
            label_guides: true,
            ..PlotStyle::new(ChartKind::Pie)
        };
        let measure = |t: &str, s: f32| t.len() as f32 * s * 0.5;
        let plot = draw_measured(&many, AREA, &style, &measure);
        assert!(plot.labels.len() > 8, "{}", plot.labels.len());
        for (i, a) in plot.labels.iter().enumerate() {
            assert!(a.rect.y() >= AREA.y() && a.rect.max_y() <= AREA.max_y() + 0.01, "{:?}", a.rect);
            assert!(a.rect.x() >= AREA.x() - 0.5 && a.rect.max_x() <= AREA.max_x() + 0.5, "{:?}", a.rect);
            for b in &plot.labels[i + 1..] {
                assert!(a.rect.inflate(-0.01).intersect(&b.rect).is_empty(), "{} over {}", a.text, b.text);
            }
        }
        assert!(plot.labels.iter().any(|l| l.text.contains("Category 0") && l.text.contains("40")));
        let elbows = plot
            .layers
            .iter()
            .filter(|l| matches!(l, Layer::Shape(s) if s.stroke.is_some() && s.path.len() == 3))
            .count();
        assert_eq!(elbows, plot.labels.len(), "a line to each, with a turn");
    }

    #[test]
    fn rings_get_one_per_series() {
        let mut style = PlotStyle::new(ChartKind::Pie);
        style.concentric = true;
        let plot = draw(&data(&[("a", &[1.0, 1.0]), ("b", &[1.0, 2.0])]), AREA, &style);
        assert_eq!(plot.sectors.len(), 4);
        assert!(plot.sectors[2].outer < plot.sectors[0].outer);
    }

    #[test]
    fn a_radar_has_a_vertex_per_category() {
        let plot = draw(&data(&[("a", &[1.0, 2.0, 3.0, 4.0, 5.0])]), AREA, &PlotStyle::new(ChartKind::Radar));
        assert_eq!(plot.marks.len(), 5);
        let top = plot.marks[0].at;
        assert!((top.x - AREA.center().x).abs() < 0.01, "the first spoke points up");
        assert!(top.y < AREA.center().y);
    }

    #[test]
    fn a_radar_needs_three_spokes() {
        let plot = draw(&data(&[("a", &[1.0, 2.0])]), AREA, &PlotStyle::new(ChartKind::Radar));
        assert!(plot.layers.is_empty());
    }

    #[test]
    fn the_longest_ring_covers_the_whole_sweep() {
        let mut style = PlotStyle::new(ChartKind::RadialBar);
        style.sweep = 180.0;
        let plot = draw(&data(&[("a", &[10.0, 5.0])]), AREA, &style);
        assert_eq!(plot.marks.len(), 2);
        let (long, short) = (plot.marks[0], plot.marks[1]);
        // A half circle clockwise from the top ends at the bottom.
        assert!(long.at.y > short.at.y);
    }

    #[test]
    fn labels_go_where_they_were_asked() {
        let mut style = PlotStyle::new(ChartKind::Bar);
        style.value_labels = Some(LabelAt::Outside);
        let plot = draw(&data(&[("a", &[5.0])]), AREA, &style);
        let value = plot.labels.iter().find(|l| l.text == "5").expect("the value");
        assert!(value.rect.center().y < plot.marks[0].at.y);
    }

    #[test]
    fn no_data_draws_nothing() {
        for kind in [ChartKind::Bar, ChartKind::Line, ChartKind::Pie, ChartKind::Radar, ChartKind::RadialBar]
        {
            let empty = ChartData::default();
            assert_eq!(rects_of(&draw(&empty, AREA, &PlotStyle::new(kind))), 0, "{kind:?}");
        }
    }

    #[test]
    fn a_kind_only_this_module_knows_still_comes_out_as_shapes() {
        let values = data(&[("a", &[1.0, 2.0, 3.0])]);
        let options = crate::chart::ChartOptions {
            kind: ChartKind::Radar,
            title: "",
            legend: false,
            size: 12.0,
            ink: Color::BLACK,
            muted: Color::BLACK,
            grid: Color::BLACK,
            format: &|value| value.to_string(),
        };
        assert!(!crate::chart::draw(&values, AREA, &options).shapes.is_empty());
    }
}
