//! Chart layout: turns aggregated numbers into shapes and labels. Pure logic,
//! kept apart from the engine so it can be tested without rendering a report.

use ravenrs_core::{Color, Point, Rect};
use serde::{Deserialize, Serialize};

use crate::vector::{self, PathSegment, Shape};

/// The kind of a chart.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub enum ChartKind {
    /// Vertical bars, side by side per series.
    #[default]
    Bar,
    /// Lines with a dot at each point.
    Line,
    /// Lines with the area under them filled.
    Area,
    /// Slices of a circle; uses the first series.
    Pie,
    /// Slices of a ring; uses the first series.
    Doughnut,
    /// Dots at numeric positions.
    Scatter,
    /// A small line without axes or text, to sit in a row of a table.
    Sparkline,
    /// A smoothed line over a filled area, without axes or text.
    SparkArea,
}

/// How a label sits in its box.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum LabelAlign {
    /// Left.
    #[default]
    Left,
    /// Centered.
    Center,
    /// Right.
    Right,
}

/// The default series and slice colors.
pub const PALETTE: [Color; 12] = [
    Color::rgba(0.247, 0.341, 0.859, 1.0),
    Color::rgba(0.086, 0.651, 0.588, 1.0),
    Color::rgba(0.961, 0.620, 0.043, 1.0),
    Color::rgba(0.863, 0.208, 0.271, 1.0),
    Color::rgba(0.545, 0.361, 0.965, 1.0),
    Color::rgba(0.055, 0.647, 0.914, 1.0),
    Color::rgba(0.518, 0.800, 0.086, 1.0),
    Color::rgba(0.925, 0.282, 0.600, 1.0),
    Color::rgba(0.471, 0.333, 0.282, 1.0),
    Color::rgba(0.392, 0.455, 0.545, 1.0),
    Color::rgba(0.706, 0.325, 0.035, 1.0),
    Color::rgba(0.051, 0.380, 0.490, 1.0),
];

/// One series' numbers, one per category.
#[derive(Clone, Debug, PartialEq)]
pub struct SeriesData {
    /// Its name in the legend.
    pub name: String,
    /// Its color.
    pub color: Color,
    /// One number per category.
    pub values: Vec<f64>,
}

/// What a chart draws, aggregated from the rows.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ChartData {
    /// What each point is about.
    pub categories: Vec<String>,
    /// Numeric positions of the categories, for scatter charts.
    pub positions: Vec<f64>,
    /// The numbers.
    pub series: Vec<SeriesData>,
}

/// Text a chart prints.
#[derive(Clone, Debug, PartialEq)]
pub struct ChartLabel {
    /// The text.
    pub text: String,
    /// The box it sits in, vertically centered.
    pub rect: Rect,
    /// How it sits in the box.
    pub align: LabelAlign,
    /// Font size.
    pub size: f32,
    /// Bold weight.
    pub bold: bool,
    /// Color.
    pub color: Color,
}

/// What a chart draws.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ChartDrawing {
    /// The shapes, back to front.
    pub shapes: Vec<Shape>,
    /// The text, to draw over the shapes.
    pub labels: Vec<ChartLabel>,
}

/// The look of a chart.
#[allow(missing_debug_implementations, reason = "holds a closure")]
pub struct ChartOptions<'a> {
    /// The kind of chart.
    pub kind: ChartKind,
    /// A title above it.
    pub title: &'a str,
    /// Whether to draw a legend when there is something to tell apart.
    pub legend: bool,
    /// The font size everything is sized from.
    pub size: f32,
    /// Title color.
    pub ink: Color,
    /// Axis and legend text color.
    pub muted: Color,
    /// Gridline color.
    pub grid: Color,
    /// How numbers are shown.
    pub format: &'a dyn Fn(f64) -> String,
    /// Slice and legend colors of pie and doughnut charts, repeated as needed;
    /// [`PALETTE`] when empty.
    pub palette: &'a [Color],
    /// What separates slices and rings the last dot of a [`ChartKind::SparkArea`]:
    /// usually the color behind the chart.
    pub separator: Color,
    /// Corner radius of bars.
    pub bar_radius: f32,
    /// The widest a bar can be, so few bars in a wide chart stay bars.
    pub max_bar_width: f32,
}

/// Roughly how wide a character is, as a share of the font size; enough to
/// decide how many labels fit.
const CHAR_WIDTH: f32 = 0.56;
/// A slice smaller than this share of the circle has no percentage on it.
const SLICE_LABEL_MIN: f64 = 0.05;
const TICKS: usize = 5;
/// The radius of a dot in a scatter chart.
const SCATTER_DOT: f32 = 2.5;
const GAP: f32 = 6.0;

/// Round numbers for an axis: lower bound, step and how many ticks.
pub fn nice_ticks(min: f64, max: f64, wanted: usize) -> (f64, f64, usize) {
    let (min, max) = if (max - min).abs() < f64::EPSILON {
        if min == 0.0 { (0.0, 1.0) } else { (min.min(0.0), max.max(0.0)) }
    } else {
        (min, max)
    };
    let rough = (max - min) / wanted.max(1) as f64;
    let magnitude = 10f64.powf(rough.log10().floor());
    let residual = rough / magnitude;
    let step = magnitude
        * if residual <= 1.0 {
            1.0
        } else if residual <= 2.0 {
            2.0
        } else if residual <= 2.5 {
            2.5
        } else if residual <= 5.0 {
            5.0
        } else {
            10.0
        };
    let low = (min / step).floor() * step;
    let high = (max / step).ceil() * step;
    (low, step, ((high - low) / step).round() as usize + 1)
}

fn label(text: impl Into<String>, rect: Rect, align: LabelAlign, size: f32, color: Color) -> ChartLabel {
    ChartLabel { text: text.into(), rect, align, size, bold: false, color }
}

fn with_alpha(color: Color, alpha: f32) -> Color {
    Color::rgba(color.r, color.g, color.b, alpha)
}

fn rectangle(rect: Rect, fill: Color) -> Shape {
    Shape::filled(vector::rectangle(rect, 0.0), fill)
}

fn dot(center: Point, radius: f32, fill: Color) -> Shape {
    let diameter = radius * 2.0;
    Shape::filled(vector::ellipse(Rect::new(center.x - radius, center.y - radius, diameter, diameter)), fill)
}

/// Appends an arc around `center`, from one angle to another (radians, clockwise on the page).
fn arc(path: &mut Vec<PathSegment>, center: Point, radius: f32, from: f32, to: f32) {
    let segments = ((to - from).abs() / std::f32::consts::FRAC_PI_2).ceil().max(1.0) as usize;
    let step = (to - from) / segments as f32;
    let handle = 4.0 / 3.0 * (step / 4.0).tan() * radius;
    let at = |angle: f32| Point::new(center.x + radius * angle.cos(), center.y + radius * angle.sin());
    for index in 0..segments {
        let a = from + step * index as f32;
        let b = a + step;
        let (start, end) = (at(a), at(b));
        path.push(PathSegment::Cubic(
            Point::new(start.x - handle * a.sin(), start.y + handle * a.cos()),
            Point::new(end.x + handle * b.sin(), end.y - handle * b.cos()),
            end,
        ));
    }
}

/// Lays a chart out inside `rect`.
pub fn draw(data: &ChartData, rect: Rect, options: &ChartOptions<'_>) -> ChartDrawing {
    let mut drawing = ChartDrawing::default();
    if matches!(options.kind, ChartKind::Sparkline | ChartKind::SparkArea) {
        sparkline(data, rect, options, &mut drawing);
        return drawing;
    }
    let mut area = rect;
    if !options.title.trim().is_empty() {
        let height = options.size * 2.0;
        let mut title = label(
            options.title,
            Rect::new(area.x(), area.y(), area.width(), height),
            LabelAlign::Left,
            options.size * 1.25,
            options.ink,
        );
        title.bold = true;
        drawing.labels.push(title);
        area = Rect::new(area.x(), area.y() + height, area.width(), (area.height() - height).max(0.0));
    }
    let round = matches!(options.kind, ChartKind::Pie | ChartKind::Doughnut);
    let entries: Vec<(String, Color)> = if round {
        data.categories.iter().enumerate().map(|(i, name)| (name.clone(), slice_color(options, i))).collect()
    } else if data.series.len() > 1 {
        data.series.iter().map(|s| (s.name.clone(), s.color)).collect()
    } else {
        Vec::new()
    };
    if options.legend && !entries.is_empty() {
        area = legend(&entries, area, options, &mut drawing);
    }
    if round {
        circle(data, area, options, &mut drawing);
    } else {
        cartesian(data, area, options, &mut drawing);
    }
    drawing
}

fn slice_color(options: &ChartOptions<'_>, index: usize) -> Color {
    let palette = if options.palette.is_empty() { &PALETTE[..] } else { options.palette };
    palette[index % palette.len()]
}

/// Draws the legend at the bottom of `area`; returns what is left above it.
fn legend(
    entries: &[(String, Color)],
    area: Rect,
    options: &ChartOptions<'_>,
    drawing: &mut ChartDrawing,
) -> Rect {
    let size = options.size * 0.85;
    let line = size * 1.6;
    let swatch = size * 0.75;
    let widths: Vec<f32> = entries
        .iter()
        .map(|(name, _)| swatch + 4.0 + name.chars().count() as f32 * size * CHAR_WIDTH + 10.0)
        .collect();
    // How many lines the entries wrap to.
    let (mut lines, mut used) = (1, 0.0);
    for width in &widths {
        if used + width > area.width() && used > 0.0 {
            lines += 1;
            used = 0.0;
        }
        used += width;
    }
    // A legend can take up to half the room; past that, the last entries are left out.
    let room = ((area.height() / 2.0 / line).floor() as usize).max(1);
    let lines = lines.min(room);
    let top = area.max_y() - line * lines as f32;
    let (mut x, mut row) = (area.x(), 0);
    for ((name, color), width) in entries.iter().zip(&widths) {
        if x + width > area.max_x() && x > area.x() {
            x = area.x();
            row += 1;
            if row >= lines {
                break;
            }
        }
        let y = top + row as f32 * line;
        drawing.shapes.push(rectangle(Rect::new(x, y + (line - swatch) / 2.0, swatch, swatch), *color));
        drawing.labels.push(label(
            name.clone(),
            Rect::new(x + swatch + 4.0, y, (width - swatch - 4.0).max(1.0), line),
            LabelAlign::Left,
            size,
            options.muted,
        ));
        x += width;
    }
    Rect::new(area.x(), area.y(), area.width(), (top - area.y() - GAP).max(0.0))
}

fn circle(data: &ChartData, area: Rect, options: &ChartOptions<'_>, drawing: &mut ChartDrawing) {
    let Some(series) = data.series.first() else { return };
    let total: f64 = series.values.iter().filter(|v| **v > 0.0).sum();
    if total <= 0.0 || area.width() <= 0.0 || area.height() <= 0.0 {
        return;
    }
    let center = area.center();
    let radius = area.width().min(area.height()) / 2.0 - 1.0;
    let hole = if options.kind == ChartKind::Doughnut { radius * 0.58 } else { 0.0 };
    let mut angle = -std::f32::consts::FRAC_PI_2;
    for (index, value) in series.values.iter().enumerate() {
        if *value <= 0.0 {
            continue;
        }
        let share = *value / total;
        let end = angle + share as f32 * std::f32::consts::TAU;
        let at = |r: f32, a: f32| Point::new(center.x + r * a.cos(), center.y + r * a.sin());
        let mut path = vec![PathSegment::Move(at(radius, angle))];
        arc(&mut path, center, radius, angle, end);
        if hole > 0.0 {
            path.push(PathSegment::Line(at(hole, end)));
            arc(&mut path, center, hole, end, angle);
        } else {
            path.push(PathSegment::Line(center));
        }
        path.push(PathSegment::Close);
        drawing.shapes.push(Shape {
            path,
            fill: Some(slice_color(options, index)),
            stroke: Some((1.0, options.separator)),
        });
        if share >= SLICE_LABEL_MIN {
            let middle = (angle + end) / 2.0;
            let position = at(if hole > 0.0 { (radius + hole) / 2.0 } else { radius * 0.66 }, middle);
            let size = options.size * 0.85;
            drawing.labels.push(label(
                format!("{:.0}%", share * 100.0),
                Rect::new(position.x - 30.0, position.y - size, 60.0, size * 2.0),
                LabelAlign::Center,
                size,
                Color::WHITE,
            ));
        }
        angle = end;
    }
}

fn cartesian(data: &ChartData, area: Rect, options: &ChartOptions<'_>, drawing: &mut ChartDrawing) {
    let values = data.series.iter().flat_map(|s| s.values.iter().copied()).filter(|v| v.is_finite());
    let (mut min, mut max) =
        values.fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), v| (a.min(v), b.max(v)));
    if !min.is_finite() || data.categories.is_empty() {
        return;
    }
    // Bars and areas start from zero, or their sizes would mislead.
    if matches!(options.kind, ChartKind::Bar | ChartKind::Area) {
        min = min.min(0.0);
        max = max.max(0.0);
    }
    let (low, step, ticks) = nice_ticks(min, max, TICKS);
    let high = low + step * (ticks.saturating_sub(1)) as f64;
    let span = (high - low).max(f64::EPSILON);
    let size = options.size * 0.85;
    let axis_labels: Vec<String> = (0..ticks).map(|i| (options.format)(low + step * i as f64)).collect();
    let axis_width =
        axis_labels.iter().map(|t| t.chars().count()).max().unwrap_or(0) as f32 * size * CHAR_WIDTH + GAP;
    let bottom_room = size * 1.8;
    let plot = Rect::new(
        area.x() + axis_width,
        area.y() + size / 2.0,
        (area.width() - axis_width).max(1.0),
        (area.height() - bottom_room - size / 2.0).max(1.0),
    );
    let y_of = |value: f64| plot.max_y() - ((value - low) / span) as f32 * plot.height();
    for (index, text) in axis_labels.iter().enumerate() {
        let y = y_of(low + step * index as f64);
        drawing.shapes.push(rectangle(Rect::new(plot.x(), y - 0.25, plot.width(), 0.5), options.grid));
        drawing.labels.push(label(
            text.clone(),
            Rect::new(area.x(), y - size, axis_width - GAP / 2.0, size * 2.0),
            LabelAlign::Right,
            size,
            options.muted,
        ));
    }
    let count = data.categories.len();
    let band = plot.width() / count as f32;
    // Only as many category labels as fit, evenly spaced.
    let longest =
        data.categories.iter().map(|c| c.chars().count()).max().unwrap_or(1) as f32 * size * CHAR_WIDTH + 4.0;
    let every = (longest / band.max(1.0)).ceil().max(1.0) as usize;
    if options.kind != ChartKind::Scatter {
        for (index, name) in data.categories.iter().enumerate().step_by(every) {
            let width = band * every as f32;
            let center = plot.x() + band * (index as f32 + 0.5);
            drawing.labels.push(label(
                name.clone(),
                Rect::new(center - width / 2.0, plot.max_y() + 2.0, width, bottom_room - 2.0),
                LabelAlign::Center,
                size,
                options.muted,
            ));
        }
    }
    match options.kind {
        ChartKind::Bar => {
            let series_count = data.series.len().max(1) as f32;
            let bar = (band * 0.72 / series_count).min(options.max_bar_width);
            let group = bar * series_count;
            let zero = y_of(0.0_f64.clamp(low, high));
            for (s, series) in data.series.iter().enumerate() {
                for (index, value) in series.values.iter().enumerate() {
                    if !value.is_finite() {
                        continue;
                    }
                    let x = plot.x() + band * index as f32 + (band - group) / 2.0 + bar * s as f32;
                    let y = y_of(*value);
                    let (top, height) = if y < zero { (y, zero - y) } else { (zero, y - zero) };
                    let rect = Rect::new(x, top, (bar - 0.5).max(0.5), height.max(0.25));
                    drawing
                        .shapes
                        .push(Shape::filled(vector::rectangle(rect, options.bar_radius), series.color));
                }
            }
        }
        ChartKind::Scatter => {
            let (x_low, x_step, x_ticks) = {
                let (a, b) = data
                    .positions
                    .iter()
                    .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), v| (a.min(*v), b.max(*v)));
                nice_ticks(if a.is_finite() { a } else { 0.0 }, if b.is_finite() { b } else { 1.0 }, TICKS)
            };
            let x_span = (x_step * x_ticks.saturating_sub(1) as f64).max(f64::EPSILON);
            // Inset by a dot's radius, so a dot at either end stays inside the plot.
            let x_of = |value: f64| {
                plot.x()
                    + SCATTER_DOT
                    + ((value - x_low) / x_span) as f32 * (plot.width() - 2.0 * SCATTER_DOT)
            };
            for index in 0..x_ticks {
                let value = x_low + x_step * index as f64;
                drawing.labels.push(label(
                    (options.format)(value),
                    Rect::new(x_of(value) - 40.0, plot.max_y() + 2.0, 80.0, bottom_room - 2.0),
                    LabelAlign::Center,
                    size,
                    options.muted,
                ));
            }
            for series in &data.series {
                for (position, value) in data.positions.iter().zip(&series.values) {
                    if value.is_finite() {
                        drawing.shapes.push(dot(
                            Point::new(x_of(*position), y_of(*value)),
                            SCATTER_DOT,
                            series.color,
                        ));
                    }
                }
            }
        }
        _ => {
            let filled = options.kind == ChartKind::Area;
            for series in &data.series {
                let points: Vec<Point> = series
                    .values
                    .iter()
                    .enumerate()
                    .filter(|(_, v)| v.is_finite())
                    .map(|(i, v)| Point::new(plot.x() + band * (i as f32 + 0.5), y_of(*v)))
                    .collect();
                if filled && let (Some(first), Some(last)) = (points.first(), points.last()) {
                    let zero = y_of(0.0_f64.clamp(low, high));
                    let mut path = vec![PathSegment::Move(Point::new(first.x, zero))];
                    path.extend(points.iter().map(|p| PathSegment::Line(*p)));
                    path.push(PathSegment::Line(Point::new(last.x, zero)));
                    path.push(PathSegment::Close);
                    drawing.shapes.push(Shape {
                        path,
                        fill: Some(with_alpha(series.color, 0.25)),
                        stroke: None,
                    });
                }
                drawing.shapes.push(polyline(&points, 1.75, series.color));
                if band > 8.0 {
                    drawing.shapes.extend(points.iter().map(|p| dot(*p, 2.25, series.color)));
                }
            }
        }
    }
}

fn polyline(points: &[Point], width: f32, color: Color) -> Shape {
    let path = points
        .iter()
        .enumerate()
        .map(|(i, p)| if i == 0 { PathSegment::Move(*p) } else { PathSegment::Line(*p) })
        .collect();
    Shape { path, fill: None, stroke: Some((width, color)) }
}

/// A curve through evenly spaced points (Catmull-Rom), without its starting move.
fn smooth(points: &[Point]) -> Vec<PathSegment> {
    let last = points.len().saturating_sub(1);
    (0..last)
        .map(|i| {
            let (p0, p1) = (points[i.saturating_sub(1)], points[i]);
            let (p2, p3) = (points[i + 1], points[(i + 2).min(last)]);
            PathSegment::Cubic(
                Point::new(p1.x + (p2.x - p0.x) / 6.0, p1.y + (p2.y - p0.y) / 6.0),
                Point::new(p2.x - (p3.x - p1.x) / 6.0, p2.y - (p3.y - p1.y) / 6.0),
                p2,
            )
        })
        .collect()
}

fn sparkline(data: &ChartData, rect: Rect, options: &ChartOptions<'_>, drawing: &mut ChartDrawing) {
    let Some(series) = data.series.first() else { return };
    let values: Vec<f64> = series.values.iter().copied().filter(|v| v.is_finite()).collect();
    if values.is_empty() {
        return;
    }
    let (min, max) =
        values.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), v| (a.min(*v), b.max(*v)));
    let span = (max - min).max(f64::EPSILON);
    let filled = options.kind == ChartKind::SparkArea;
    // Room for the last dot.
    let pad = if filled { 6.0 } else { 2.5 };
    let (width, height) = ((rect.width() - 2.0 * pad).max(1.0), (rect.height() - 2.0 * pad).max(1.0));
    let last = values.len().saturating_sub(1).max(1) as f32;
    let points: Vec<Point> = values
        .iter()
        .enumerate()
        .map(|(i, v)| {
            Point::new(
                rect.x() + pad + width * i as f32 / last,
                rect.y() + pad + height * (1.0 - ((*v - min) / span) as f32),
            )
        })
        .collect();
    let (Some(first), Some(end)) = (points.first().copied(), points.last().copied()) else { return };
    if filled {
        let curve = smooth(&points);
        let mut area = vec![PathSegment::Move(Point::new(first.x, rect.max_y())), PathSegment::Line(first)];
        area.extend(curve.iter().copied());
        area.extend([PathSegment::Line(Point::new(end.x, rect.max_y())), PathSegment::Close]);
        drawing.shapes.push(Shape::filled(area, with_alpha(series.color, 0.2)));
        let mut line = vec![PathSegment::Move(first)];
        line.extend(curve);
        drawing.shapes.push(Shape { path: line, fill: None, stroke: Some((2.0, series.color)) });
        let mut last = dot(end, 4.0, series.color);
        last.stroke = Some((2.0, options.separator));
        drawing.shapes.push(last);
    } else {
        drawing.shapes.push(polyline(&points, 1.25, series.color));
        drawing.shapes.push(dot(end, 2.0, series.color));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plain(value: f64) -> String {
        format!("{value}")
    }

    fn options<'a>(kind: ChartKind, format: &'a dyn Fn(f64) -> String) -> ChartOptions<'a> {
        ChartOptions {
            kind,
            title: "",
            legend: true,
            size: 10.0,
            ink: Color::BLACK,
            muted: Color::BLACK,
            grid: Color::BLACK,
            format,
            palette: &[],
            separator: Color::WHITE,
            bar_radius: 0.0,
            max_bar_width: f32::INFINITY,
        }
    }

    fn data(values: &[f64]) -> ChartData {
        ChartData {
            categories: (0..values.len()).map(|i| format!("C{i}")).collect(),
            positions: (0..values.len()).map(|i| i as f64).collect(),
            series: vec![SeriesData { name: "S".into(), color: PALETTE[0], values: values.to_vec() }],
        }
    }

    const AREA: Rect =
        Rect { origin: Point { x: 10.0, y: 20.0 }, size: ravenrs_core::Size { width: 400.0, height: 200.0 } };

    fn bounds(segment: &PathSegment) -> Vec<Point> {
        match segment {
            PathSegment::Move(p) | PathSegment::Line(p) => vec![*p],
            PathSegment::Cubic(_, _, p) => vec![*p],
            _ => Vec::new(),
        }
    }

    #[test]
    fn ticks_are_round_numbers_that_cover_the_values() {
        assert_eq!(nice_ticks(0.0, 100.0, 5), (0.0, 20.0, 6));
        assert_eq!(nice_ticks(0.0, 87.0, 5), (0.0, 20.0, 6));
        assert_eq!(nice_ticks(0.0, 4.2, 5), (0.0, 1.0, 6));
        let (low, step, count) = nice_ticks(-30.0, 70.0, 5);
        assert!(low <= -30.0 && low + step * (count - 1) as f64 >= 70.0);
        assert_eq!(nice_ticks(0.0, 0.0, 5), (0.0, 0.2, 6));
        let (low, step, count) = nice_ticks(5.0, 5.0, 5);
        assert!(low <= 5.0 && low + step * (count - 1) as f64 >= 5.0 && step > 0.0);
    }

    #[test]
    fn every_kind_stays_inside_its_box() {
        let format = plain;
        for kind in [
            ChartKind::Bar,
            ChartKind::Line,
            ChartKind::Area,
            ChartKind::Pie,
            ChartKind::Doughnut,
            ChartKind::Scatter,
            ChartKind::Sparkline,
            ChartKind::SparkArea,
        ] {
            let drawing = draw(&data(&[3.0, 9.0, 4.0, 12.0]), AREA, &options(kind, &format));
            assert!(!drawing.shapes.is_empty(), "{kind:?}");
            for shape in &drawing.shapes {
                for point in shape.path.iter().flat_map(bounds) {
                    assert!(
                        point.x >= AREA.x() - 1.0
                            && point.x <= AREA.max_x() + 1.0
                            && point.y >= AREA.y() - 1.0
                            && point.y <= AREA.max_y() + 1.0,
                        "{kind:?} {point:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn bars_are_as_tall_as_their_values_say() {
        let format = plain;
        let drawing = draw(&data(&[50.0, 100.0]), AREA, &options(ChartKind::Bar, &format));
        let tall = |shape: &Shape| match (shape.path.first(), shape.path.get(2)) {
            (Some(PathSegment::Move(a)), Some(PathSegment::Line(b))) => b.y - a.y,
            _ => 0.0,
        };
        let colored: Vec<f32> =
            drawing.shapes.iter().filter(|s| s.fill == Some(PALETTE[0])).map(tall).collect();
        assert_eq!(colored.len(), 2);
        assert!((colored[1] / colored[0] - 2.0).abs() < 0.01, "{colored:?}");
    }

    #[test]
    fn negative_bars_go_below_zero() {
        let format = plain;
        let drawing = draw(&data(&[-50.0, 100.0]), AREA, &options(ChartKind::Bar, &format));
        assert!(drawing.labels.iter().any(|l| l.text == "-50"), "{:?}", drawing.labels);
    }

    #[test]
    fn slices_are_labeled_by_share_and_tiny_ones_are_not() {
        let format = plain;
        let drawing = draw(&data(&[75.0, 24.0, 1.0]), AREA, &options(ChartKind::Pie, &format));
        let texts: Vec<&str> = drawing.labels.iter().map(|l| l.text.as_str()).collect();
        assert!(texts.contains(&"75%") && texts.contains(&"24%"), "{texts:?}");
        assert!(!texts.contains(&"1%"), "{texts:?}");
        assert!(texts.contains(&"C2"), "the legend still names it: {texts:?}");
    }

    #[test]
    fn the_legend_names_every_category_when_there_is_room() {
        let format = plain;
        let drawing =
            draw(&data(&[1.0; 10]), Rect::new(0.0, 0.0, 240.0, 230.0), &options(ChartKind::Pie, &format));
        for index in 0..10 {
            let name = format!("C{index}");
            assert!(drawing.labels.iter().any(|l| l.text == name), "{name}");
        }
    }

    #[test]
    fn ten_slices_have_ten_colors() {
        let unique: Vec<_> =
            PALETTE[..10].iter().map(|c| (c.r.to_bits(), c.g.to_bits(), c.b.to_bits())).collect();
        let set: std::collections::HashSet<_> = unique.iter().collect();
        assert_eq!(set.len(), 10);
    }

    #[test]
    fn nothing_to_draw_draws_nothing() {
        let format = plain;
        for kind in
            [ChartKind::Bar, ChartKind::Line, ChartKind::Pie, ChartKind::Scatter, ChartKind::Sparkline]
        {
            let drawing = draw(&ChartData::default(), AREA, &options(kind, &format));
            assert!(drawing.shapes.is_empty(), "{kind:?}");
        }
        assert!(
            draw(&data(&[0.0, 0.0]), AREA, &options(ChartKind::Pie, &format))
                .labels
                .iter()
                .all(|l| !l.text.contains('%'))
        );
    }

    #[test]
    fn a_sparkline_has_no_text() {
        let format = plain;
        assert!(draw(&data(&[1.0, 2.0]), AREA, &options(ChartKind::Sparkline, &format)).labels.is_empty());
        assert_eq!(draw(&data(&[1.0]), AREA, &options(ChartKind::Sparkline, &format)).shapes.len(), 2);
    }

    #[test]
    fn a_spark_area_is_a_curve_over_a_fill_and_has_no_text() {
        let format = plain;
        let drawing = draw(&data(&[1.0, 4.0, 2.0]), AREA, &options(ChartKind::SparkArea, &format));
        assert!(drawing.labels.is_empty());
        assert_eq!(drawing.shapes.len(), 3, "area, line and last dot");
        assert!(drawing.shapes[1].path.iter().any(|s| matches!(s, PathSegment::Cubic(..))));
        assert_eq!(drawing.shapes[2].stroke, Some((2.0, Color::WHITE)));
    }

    #[test]
    fn bars_stop_growing_at_their_widest() {
        let format = plain;
        let mut narrow = options(ChartKind::Bar, &format);
        narrow.max_bar_width = 20.0;
        let drawing = draw(&data(&[5.0]), AREA, &narrow);
        let bar = drawing.shapes.iter().find(|s| s.fill == Some(PALETTE[0])).expect("a bar");
        let xs: Vec<f32> = bar.path.iter().flat_map(bounds).map(|p| p.x).collect();
        let width = xs.iter().copied().fold(f32::MIN, f32::max) - xs.iter().copied().fold(f32::MAX, f32::min);
        assert!((width - 19.5).abs() < 0.01, "{width}");
    }

    #[test]
    fn slices_use_the_colors_they_are_given() {
        let format = plain;
        let colors = [Color::BLACK];
        let mut given = options(ChartKind::Pie, &format);
        given.palette = &colors;
        given.separator = Color::BLACK;
        let drawing = draw(&data(&[1.0, 2.0]), AREA, &given);
        assert!(drawing.shapes.iter().all(|s| s.fill == Some(Color::BLACK)));
    }

    #[test]
    fn labels_are_thinned_when_categories_do_not_fit() {
        let format = plain;
        let many = data(&vec![1.0; 200]);
        let drawing = draw(&many, AREA, &options(ChartKind::Line, &format));
        let categories = drawing.labels.iter().filter(|l| l.text.starts_with('C')).count();
        assert!(categories > 1 && categories < 60, "{categories}");
    }
}
