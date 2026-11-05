//! Instruments for looking at and adjusting pictures: histograms, rulers, curves, color
//! wheels, zoom and pan. None of them knows photos, engines or file formats: every number
//! comes from whoever uses them.

use std::rc::Rc;

use ravenrs_core::{Color, PathElement, PathFill, Point, Quad, Rect, Role, Scene};
use ravenrs_layout::{Align, Axis, LayoutStyle};
use ravenrs_style::{Semantic, TextStyle};

use crate::View;
use crate::context::{BuildCx, NodeId, Paint, PaintCx, Semantics};
use crate::util;

// ---------------------------------------------------------------------------------------------
// Histogram
// ---------------------------------------------------------------------------------------------

/// How a [`Histogram`] turns counts into heights.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum HistogramScale {
    /// Height follows the count.
    #[default]
    Linear,
    /// Height follows the logarithm of the count, which brings small hills into view next
    /// to a tall peak.
    Logarithmic,
}

/// One series of a [`Histogram`]: ready-made bins and the color that shows them.
#[derive(Clone, Debug, PartialEq)]
pub struct HistogramChannel {
    label: String,
    color: Color,
    bins: Rc<[f32]>,
    line: bool,
}

impl HistogramChannel {
    /// A filled channel. `color` is data: it is drawn as given.
    pub fn new(
        label: impl Into<String>,
        color: Color,
        bins: impl IntoIterator<Item = impl Into<f64>>,
    ) -> Self {
        let bins = bins.into_iter().map(|v| v.into().max(0.0) as f32).collect();
        Self { label: label.into(), color, bins, line: false }
    }

    /// Draws this channel as a line over the filled ones, such as luminance.
    pub fn line(mut self) -> Self {
        self.line = true;
        self
    }

    /// The share of the channel in one bin, or zero for an empty channel.
    fn share(&self, bin: Option<&f32>) -> f32 {
        let total: f32 = self.bins.iter().sum();
        if total > 0.0 { bin.copied().unwrap_or(0.0) / total } else { 0.0 }
    }
}

/// Adds colors of light, as overlapping channels of a picture do: red and green make yellow,
/// all three make gray. On a light theme a pure sum would melt into the page, and it goes
/// down in brightness instead.
fn light_sum(colors: impl Iterator<Item = Color>, dim: f32) -> Color {
    let (mut r, mut g, mut b, mut count) = (0.0_f32, 0.0_f32, 0.0_f32, 0);
    for c in colors {
        (r, g, b, count) = (r + c.r, g + c.g, b + c.b, count + 1);
    }
    let shade = if count > 1 { dim } else { 1.0 };
    Color::rgb(r.min(1.0) * shade, g.min(1.0) * shade, b.min(1.0) * shade)
}

/// How much a sum of channels is held back, by how the background looks.
fn overlap_dim(background: Color) -> f32 {
    let brightness = 0.2126 * background.r + 0.7152 * background.g + 0.0722 * background.b;
    if brightness > 0.5 { 0.62 } else { 0.86 }
}

/// What it takes to draw a set of channels, apart from any widget.
#[derive(Clone, Debug)]
struct HistogramPicture {
    channels: Rc<[HistogramChannel]>,
    scale: HistogramScale,
    dim: f32,
    opacity: f32,
}

impl HistogramPicture {
    /// The tallest count, leaving out the two ends: a pile of clipped pixels there would
    /// otherwise flatten everything else. The clip marks tell about those.
    fn peak(&self) -> f32 {
        let top = |inner: bool| {
            self.channels
                .iter()
                .flat_map(|c| {
                    let bins = &c.bins[..];
                    if inner && bins.len() > 2 { &bins[1..bins.len() - 1] } else { bins }
                })
                .copied()
                .fold(0.0, f32::max)
        };
        let inner = top(true);
        if inner > 0.0 { inner } else { top(false) }
    }

    fn height(&self, value: f32, peak: f32) -> f32 {
        if peak <= 0.0 {
            return 0.0;
        }
        let h = match self.scale {
            HistogramScale::Linear => value / peak,
            HistogramScale::Logarithmic => (1.0 + value).ln() / (1.0 + peak).ln(),
        };
        h.clamp(0.0, 1.0)
    }

    /// The tallest bin of `bins` under column `column` of `columns`, as a height.
    fn column(&self, bins: &[f32], column: usize, columns: usize, peak: f32) -> f32 {
        let from = column * bins.len() / columns;
        let to = ((column + 1) * bins.len()).div_ceil(columns).clamp(from + 1, bins.len().max(from + 1));
        let top = bins.get(from..to).map_or(0.0, |part| part.iter().copied().fold(0.0, f32::max));
        self.height(top, peak)
    }

    fn paint(&self, scene: &mut Scene, area: Rect) {
        let widest = self.channels.iter().map(|c| c.bins.len()).max().unwrap_or(0);
        if widest == 0 || area.is_empty() {
            return;
        }
        // Never more columns than pixels: thin ones would only cost and blur.
        let columns = widest.min(area.width().floor().max(1.0) as usize);
        let peak = self.peak();
        let edge = |column: usize| area.x() + (area.width() * column as f32 / columns as f32).round();
        let mut stack: Vec<(f32, Color)> = Vec::with_capacity(self.channels.len());
        for column in 0..columns {
            stack.clear();
            stack.extend(
                self.channels
                    .iter()
                    .filter(|c| !c.line && !c.bins.is_empty())
                    .map(|c| (self.column(&c.bins, column, columns, peak), c.color)),
            );
            stack.sort_by(|a, b| b.0.total_cmp(&a.0));
            let (x, right) = (edge(column), edge(column + 1));
            // Top down: one channel alone, then two together, then all of them.
            for i in 0..stack.len() {
                let (top, bottom) = (stack[i].0, stack.get(i + 1).map_or(0.0, |s| s.0));
                if top - bottom <= 0.0 {
                    continue;
                }
                let color = light_sum(stack[..=i].iter().map(|s| s.1), self.dim);
                let y = area.max_y() - area.height() * top;
                let h = area.height() * (top - bottom);
                scene.push_quad(Quad::new(
                    Rect::new(x, y, (right - x).max(1.0), h),
                    color.with_alpha(self.opacity),
                ));
            }
        }
        for channel in self.channels.iter().filter(|c| c.line && !c.bins.is_empty()) {
            let line: Vec<Point> = (0..columns)
                .map(|column| {
                    let h = self.column(&channel.bins, column, columns, peak);
                    let x = area.x() + area.width() * (column as f32 + 0.5) / columns as f32;
                    Point::new(x, area.max_y() - area.height() * h)
                })
                .collect();
            let color = channel.color.with_alpha(self.opacity);
            scene.push_path(PathFill::solid(stroke_outline(&line, 1.25), color));
        }
    }
}

/// The outline of a line through `points`: a stroke is a fill of its expanded outline.
fn stroke_outline(points: &[Point], width: f32) -> Vec<PathElement> {
    let half = width / 2.0;
    let normal = |i: usize| {
        let (a, b) = (points[i.saturating_sub(1)], points[(i + 1).min(points.len() - 1)]);
        let (dx, dy) = (b.x - a.x, b.y - a.y);
        let length = dx.hypot(dy).max(f32::EPSILON);
        (-dy / length * half, dx / length * half)
    };
    let mut outline = Vec::with_capacity(points.len() * 2 + 2);
    for (i, p) in points.iter().enumerate() {
        let (nx, ny) = normal(i);
        let at = Point::new(p.x + nx, p.y + ny);
        outline.push(if i == 0 { PathElement::MoveTo(at) } else { PathElement::LineTo(at) });
    }
    for (i, p) in points.iter().enumerate().rev() {
        let (nx, ny) = normal(i);
        outline.push(PathElement::LineTo(Point::new(p.x - nx, p.y - ny)));
    }
    outline.push(PathElement::Close);
    outline
}

/// Overlapping channels from ready-made bins, in a linear or logarithmic scale, with a mark
/// at each end when too much of a channel piles up there. Where channels overlap, their
/// colors add up, as light does, so each stays readable.
///
/// ```
/// use ravenrs_core::Color;
/// use ravenrs_widgets::{Histogram, HistogramChannel, HistogramScale};
///
/// let red: Vec<u32> = (0..256).map(|i| (i * (256 - i)) as u32).collect();
/// let view = Histogram::new([HistogramChannel::new("Red", Color::rgb(0.9, 0.2, 0.2), red)])
///     .scale(HistogramScale::Logarithmic)
///     .zones(5);
/// ```
#[derive(Clone, Debug)]
pub struct Histogram {
    channels: Rc<[HistogramChannel]>,
    scale: HistogramScale,
    clip_share: f32,
    zones: u32,
    width: Option<f32>,
    height: f32,
    label: Option<String>,
}

impl Histogram {
    /// A histogram of these channels, 96 logical pixels tall, across its parent.
    pub fn new(channels: impl IntoIterator<Item = HistogramChannel>) -> Self {
        Self {
            channels: channels.into_iter().collect(),
            scale: HistogramScale::Linear,
            clip_share: 0.001,
            zones: 0,
            width: None,
            height: 96.0,
            label: None,
        }
    }

    /// How counts turn into heights (`Linear` by default).
    pub fn scale(mut self, scale: HistogramScale) -> Self {
        self.scale = scale;
        self
    }

    /// The share of a channel that must sit in an end bin to light that end's mark, from 0
    /// to 1 (0.001 by default, which ignores a stray reflection).
    pub fn clip_share(mut self, share: f32) -> Self {
        self.clip_share = share;
        self
    }

    /// Divides the width in this many equal zones, with light lines (none by default).
    pub fn zones(mut self, zones: u32) -> Self {
        self.zones = zones;
        self
    }

    /// Fixed size in logical pixels.
    pub fn size(mut self, width: f32, height: f32) -> Self {
        self.width = Some(width);
        self.height = height;
        self
    }

    /// Height in logical pixels (96 by default).
    pub fn height(mut self, height: f32) -> Self {
        self.height = height;
        self
    }

    /// The name screen readers announce, instead of the one in [`Labels`](crate::Labels).
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Which ends have a channel piled up: the sums of their colors, low end first.
    fn clipped(&self, dim: f32) -> (Option<Color>, Option<Color>) {
        let end = |pick: fn(&[f32]) -> Option<&f32>| {
            let over: Vec<Color> = self
                .channels
                .iter()
                .filter(|c| !c.line && c.share(pick(&c.bins)) > self.clip_share)
                .map(|c| c.color)
                .collect();
            (!over.is_empty()).then(|| light_sum(over.into_iter(), dim))
        };
        (end(<[f32]>::first), end(<[f32]>::last))
    }
}

impl View for Histogram {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let labels = cx.labels();
        let background = theme.color(Semantic::Canvas);
        let dim = overlap_dim(background);
        let picture =
            HistogramPicture { channels: self.channels.clone(), scale: self.scale, dim, opacity: 1.0 };
        let (low, high) = self.clipped(dim);
        let (border, radius, zones) = (theme.color(Semantic::Border), theme.radii.sm, self.zones);
        let node = cx.container(
            LayoutStyle {
                width: self.width,
                height: Some(self.height),
                align_self: Some(if self.width.is_some() { Align::Start } else { Align::Stretch }),
                ..LayoutStyle::leaf()
            },
            &[],
            Paint::Custom(Rc::new(move |frame: Rect, scene| {
                scene.push_quad(Quad::new(frame, background).radius(radius).border(1.0, border));
                let area = frame.inflate(-1.0);
                for zone in 1..zones {
                    let x = area.x() + (area.width() * zone as f32 / zones as f32).round();
                    let line = Rect::new(x, area.y(), 1.0, area.height());
                    scene.push_quad(Quad::new(line, border.with_alpha(border.a * 0.5)));
                }
                picture.paint(scene, area);
                paint_clip_marks(scene, area, low, high);
            })),
        );
        let mut value: Vec<&str> = self.channels.iter().map(|c| c.label.as_str()).collect();
        if low.is_some() {
            value.push(&labels.histogram_clipped_low);
        }
        if high.is_some() {
            value.push(&labels.histogram_clipped_high);
        }
        let name = self.label.clone().unwrap_or_else(|| labels.histogram.clone());
        cx.semantics(node, Semantics::new(Role::Image).name(name).value(value.join(", ")));
        node
    }
}

/// A triangle in each top corner that has clipping. Its presence says it, whatever the color.
fn paint_clip_marks(scene: &mut Scene, area: Rect, low: Option<Color>, high: Option<Color>) {
    const SIDE: f32 = 9.0;
    let (top, left, right) = (area.y() + 2.0, area.x() + 2.0, area.max_x() - 2.0);
    if let Some(color) = low {
        let corner = [(left, top), (left + SIDE, top), (left, top + SIDE)];
        scene.push_path(PathFill::solid(triangle(corner), color));
    }
    if let Some(color) = high {
        let corner = [(right, top), (right - SIDE, top), (right, top + SIDE)];
        scene.push_path(PathFill::solid(triangle(corner), color));
    }
}

fn triangle(corners: [(f32, f32); 3]) -> Vec<PathElement> {
    let at = |(x, y): (f32, f32)| Point::new(x, y);
    vec![
        PathElement::MoveTo(at(corners[0])),
        PathElement::LineTo(at(corners[1])),
        PathElement::LineTo(at(corners[2])),
        PathElement::Close,
    ]
}

// ---------------------------------------------------------------------------------------------
// Ruler
// ---------------------------------------------------------------------------------------------

/// Largest number of tick marks a ruler draws per logical pixel of length: a guard for
/// nonsense scales, since the step already keeps real ones far below this.
const RULER_TICKS_PER_PIXEL: f32 = 0.5;

/// The round number at or above `least`: 1, 2 or 5 times a power of ten.
fn round_step(least: f32) -> f32 {
    if !least.is_finite() || least <= 0.0 {
        return 1.0;
    }
    let power = 10_f32.powf(least.log10().floor());
    [1.0, 2.0, 5.0, 10.0].into_iter().map(|m| m * power).find(|s| *s >= least * 0.999).unwrap_or(10.0 * power)
}

/// How many parts a step divides into, the most that keeps marks at least 4 pixels apart.
fn ruler_subdivisions(step: f32, pixels_per_unit: f32, whole_units: bool) -> u32 {
    [10_u32, 5, 2]
        .into_iter()
        .find(|&n| {
            let part = step / n as f32;
            part * pixels_per_unit >= 4.0 && (!whole_units || part >= 1.0)
        })
        .unwrap_or(1)
}

/// A number on a ruler, with only the decimals its step needs.
fn ruler_number(value: f32, step: f32) -> String {
    let decimals = if step >= 1.0 { 0 } else { (-step.log10()).ceil() as usize };
    let text = format!("{value:.decimals$}");
    // A value a rounding error away from zero must not read "-0".
    if text.starts_with('-') && text[1..].chars().all(|c| c == '0' || c == '.') {
        text[1..].to_string()
    } else {
        text
    }
}

/// A horizontal or vertical ruler in the caller's units. It picks a round step for the scale,
/// so marks never crowd as the zoom changes. Numbers on a vertical ruler stack digit over
/// digit, since text does not rotate.
///
/// ```
/// use ravenrs_widgets::Ruler;
///
/// // 2.5 screen pixels per unit, with unit 120 at the start and the pointer at unit 180.
/// let top = Ruler::horizontal().scale(2.5).origin(120.0).marker(180.0);
/// let side = Ruler::vertical().scale(2.5).whole_units();
/// ```
#[derive(Clone, Debug)]
pub struct Ruler {
    axis: Axis,
    scale: f32,
    origin: f32,
    marker: Option<f32>,
    thickness: f32,
    min_spacing: f32,
    whole_units: bool,
    label: Option<String>,
}

impl Ruler {
    /// A ruler across its parent, with one pixel per unit and zero at the start.
    pub fn horizontal() -> Self {
        Self::along(Axis::Horizontal)
    }

    /// A ruler down its parent, with one pixel per unit and zero at the top.
    pub fn vertical() -> Self {
        Self::along(Axis::Vertical)
    }

    fn along(axis: Axis) -> Self {
        Self {
            axis,
            scale: 1.0,
            origin: 0.0,
            marker: None,
            thickness: 18.0,
            min_spacing: 64.0,
            whole_units: false,
            label: None,
        }
    }

    /// Logical pixels per unit: the zoom.
    pub fn scale(mut self, pixels_per_unit: f32) -> Self {
        self.scale = pixels_per_unit;
        self
    }

    /// The value in units at the ruler's start, which panning moves.
    pub fn origin(mut self, units: f32) -> Self {
        self.origin = units;
        self
    }

    /// Marks this value in units, such as where the pointer is.
    pub fn marker(mut self, units: f32) -> Self {
        self.marker = Some(units);
        self
    }

    /// Thickness in logical pixels (18 by default).
    pub fn thickness(mut self, thickness: f32) -> Self {
        self.thickness = thickness;
        self
    }

    /// The least distance between numbered marks, in logical pixels (64 by default).
    pub fn min_spacing(mut self, pixels: f32) -> Self {
        self.min_spacing = pixels;
        self
    }

    /// Never divides below one unit, for what comes in whole pieces, such as pixels.
    pub fn whole_units(mut self) -> Self {
        self.whole_units = true;
        self
    }

    /// The name screen readers announce, instead of the one in [`Labels`](crate::Labels).
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// The step between numbered marks at this scale.
    fn step(&self) -> f32 {
        let least = self.min_spacing.max(1.0) / self.scale.max(f32::EPSILON);
        round_step(if self.whole_units { least.max(1.0) } else { least })
    }
}

impl View for Ruler {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let labels = cx.labels();
        let horizontal = self.axis == Axis::Horizontal;
        let (surface, tick, ink, accent) = (
            theme.color(Semantic::Surface),
            theme.color(Semantic::Border),
            theme.color(Semantic::InkMuted),
            theme.color(Semantic::Accent),
        );
        let attrs = {
            let mut attrs = util::attrs(&theme, TextStyle::Caption, None);
            attrs.size = 9.0;
            attrs.line_height = 10.0;
            attrs
        };
        let ruler = self.clone();
        let step = self.step();
        let node = cx.container(
            LayoutStyle {
                width: (!horizontal).then_some(self.thickness),
                height: horizontal.then_some(self.thickness),
                align_self: Some(Align::Stretch),
                overflow_hidden: true,
                ..LayoutStyle::leaf()
            },
            &[],
            Paint::Rich(Rc::new(move |frame: Rect, paint: &mut PaintCx<'_>| {
                if frame.is_empty() {
                    return;
                }
                let scene = &mut *paint.scene;
                scene.push_quad(Quad::new(frame, surface));
                // The edge next to what it measures.
                let edge = if horizontal {
                    Rect::new(frame.x(), frame.max_y() - 1.0, frame.width(), 1.0)
                } else {
                    Rect::new(frame.max_x() - 1.0, frame.y(), 1.0, frame.height())
                };
                scene.push_quad(Quad::new(edge, tick));
                scene.push_clip(frame);
                let length = if horizontal { frame.width() } else { frame.height() };
                let across = if horizontal { frame.height() } else { frame.width() };
                let place = |units: f32| (units - ruler.origin) * ruler.scale;
                let parts = ruler_subdivisions(step, ruler.scale, ruler.whole_units);
                let part = step / parts as f32;
                let first = (ruler.origin / part).floor() as i64;
                let limit = (length * RULER_TICKS_PER_PIXEL) as i64 + 2;
                for index in first..first.saturating_add(limit) {
                    let units = index as f32 * part;
                    let at = place(units);
                    if at > length {
                        break;
                    }
                    let major = index.rem_euclid(i64::from(parts)) == 0;
                    let half =
                        parts.is_multiple_of(2) && index.rem_euclid(i64::from(parts)) == i64::from(parts / 2);
                    let size = if major {
                        across
                    } else if half {
                        across * 0.45
                    } else {
                        across * 0.25
                    };
                    let mark = if horizontal {
                        Rect::new(frame.x() + at.round(), frame.max_y() - size, 1.0, size)
                    } else {
                        Rect::new(frame.max_x() - size, frame.y() + at.round(), size, 1.0)
                    };
                    paint.scene.push_quad(Quad::new(mark, tick));
                    if !major {
                        continue;
                    }
                    let number = ruler_number(units, step);
                    if horizontal {
                        let origin = Point::new(frame.x() + at.round() + 3.0, frame.y() + 1.0);
                        let mut layout = paint.text.layout(&number, &attrs);
                        layout.measure(None);
                        layout.paint(origin, ink, paint.scene);
                    } else {
                        // One character per line, each centered on its own width.
                        let mut y = frame.y() + at.round() + 2.0;
                        for digit in number.chars() {
                            let mut buffer = [0_u8; 4];
                            let mut layout = paint.text.layout(digit.encode_utf8(&mut buffer), &attrs);
                            let width = layout.measure(None).width;
                            let x = frame.x() + ((across - 3.0 - width) / 2.0).max(0.0);
                            layout.paint(Point::new(x, y), ink, paint.scene);
                            y += attrs.line_height - 1.0;
                        }
                    }
                }
                if let Some(marker) = ruler.marker {
                    let at = place(marker);
                    let line = if horizontal {
                        Rect::new(frame.x() + at - 0.5, frame.y(), 1.0, frame.height())
                    } else {
                        Rect::new(frame.x(), frame.y() + at - 0.5, frame.width(), 1.0)
                    };
                    paint.scene.push_quad(Quad::new(line, accent));
                    let corners = if horizontal {
                        let (x, y) = (frame.x() + at, frame.max_y());
                        [(x - 4.0, y - 5.0), (x + 4.0, y - 5.0), (x, y)]
                    } else {
                        let (x, y) = (frame.max_x(), frame.y() + at);
                        [(x - 5.0, y - 4.0), (x - 5.0, y + 4.0), (x, y)]
                    };
                    paint.scene.push_path(PathFill::solid(triangle(corners), accent));
                }
                paint.scene.pop_clip();
            })),
        );
        let name = self.label.clone().unwrap_or_else(|| labels.ruler.clone());
        let mut semantics = Semantics::new(Role::Image).name(name);
        if let Some(marker) = self.marker {
            semantics = semantics.value(ruler_number(marker, step));
        }
        cx.semantics(node, semantics);
        node
    }
}
