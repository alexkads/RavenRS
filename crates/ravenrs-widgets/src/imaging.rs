//! Instruments for looking at and adjusting pictures: histograms, rulers, curves, color
//! wheels, zoom and pan. None of them knows photos, engines or file formats: every number
//! comes from whoever uses them.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use ravenrs_core::{
    Color, CursorIcon, ImageData, ImageQuad, Key, PathElement, PathFill, Point, Quad, Rect, Role, Scene,
};
use ravenrs_layout::{Align, Axis, LayoutStyle};
use ravenrs_reactive::Binding;
use ravenrs_style::{Semantic, TextStyle};

use crate::View;
use crate::color_picker::Hsv;
use crate::context::{BuildCx, DragPhase, NodeId, Paint, PaintCx, Semantics};
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

// ---------------------------------------------------------------------------------------------
// ColorWheel
// ---------------------------------------------------------------------------------------------

/// A color as a [`ColorWheel`] holds it, without brightness.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct HueSaturation {
    /// Degrees, from 0 up to 360. Red is 0.
    pub hue: f32,
    /// From 0 (gray, at the center of the wheel) to 1 (pure, at its edge).
    pub saturation: f32,
}

impl HueSaturation {
    /// A hue in degrees and a saturation from 0 to 1, brought within range.
    pub fn new(hue: f32, saturation: f32) -> Self {
        Self { hue: hue.rem_euclid(360.0), saturation: saturation.clamp(0.0, 1.0) }
    }

    /// The color this is, at this brightness.
    fn color(self, value: f32) -> Color {
        Hsv { hue: self.hue, saturation: self.saturation, value, alpha: 1.0 }.color()
    }

    /// Where this color sits, relative to the center of a wheel of this radius: red to the
    /// right and hue growing counterclockwise, as color tools lay it out.
    fn offset(self, radius: f32) -> Point {
        let (sin, cos) = self.hue.to_radians().sin_cos();
        Point::new(cos * self.saturation * radius, -sin * self.saturation * radius)
    }

    /// The color at an offset from the center of a wheel of this radius. Beyond the edge it
    /// is the edge, so a drag keeps working outside the disc.
    fn at(offset: Point, radius: f32) -> Self {
        let distance = offset.x.hypot(offset.y);
        let hue = (-offset.y).atan2(offset.x).to_degrees();
        Self::new(hue, distance / radius.max(1.0))
    }
}

/// Room around the disc, in logical pixels, for the puck at the edge.
const WHEEL_MARGIN: f32 = 7.0;

thread_local! {
    /// Discs already computed, by side in pixels and brightness. A panel with four wheels
    /// shares one, and a drag repaints none. The few sizes in use stay few.
    static WHEEL_DISCS: RefCell<HashMap<(u32, u32), ImageData>> = RefCell::new(HashMap::new());
}

/// The disc of colors, `side` pixels wide, with an antialiased edge. Costly to compute and
/// cheap to show, since it is a texture from then on.
fn wheel_disc(side: u32, value: f32) -> Option<ImageData> {
    WHEEL_DISCS.with_borrow_mut(|discs| {
        let key = (side, value.to_bits());
        if let Some(disc) = discs.get(&key) {
            return Some(disc.clone());
        }
        if discs.len() >= 8 {
            discs.clear();
        }
        let middle = side as f32 / 2.0;
        let disc = ImageData::from_fn(side, side, |x, y| {
            let offset = Point::new(x as f32 + 0.5 - middle, y as f32 + 0.5 - middle);
            let coverage = (middle - offset.x.hypot(offset.y)).clamp(0.0, 1.0);
            HueSaturation::at(offset, middle).color(value).with_alpha(coverage)
        })?;
        discs.insert(key, disc.clone());
        Some(disc)
    })
}

/// Picks hue and saturation on a disc, by a drag or the arrow keys: left and right turn the
/// hue, up and down change the saturation, with Shift for bigger steps. Holding Shift during
/// a drag keeps the hue. A double click or Home takes the color out, keeping the hue.
///
/// ```
/// use ravenrs_reactive::State;
/// use ravenrs_widgets::{ColorWheel, HueSaturation};
///
/// let tint = State::new(HueSaturation::new(30.0, 0.4));
/// let wheel = ColorWheel::new(tint.binding()).diameter(140.0).label("Shadows");
/// ```
#[derive(Debug)]
pub struct ColorWheel {
    value: Binding<HueSaturation>,
    diameter: f32,
    brightness: f32,
    label: Option<String>,
    disabled: bool,
}

impl ColorWheel {
    /// A wheel bound to `value`, 160 logical pixels wide.
    pub fn new(value: Binding<HueSaturation>) -> Self {
        Self { value, diameter: 160.0, brightness: 1.0, label: None, disabled: false }
    }

    /// Width and height in logical pixels.
    pub fn diameter(mut self, diameter: f32) -> Self {
        self.diameter = diameter;
        self
    }

    /// How bright the disc is drawn, from 0 to 1 (1 by default). It changes only the look.
    pub fn brightness(mut self, brightness: f32) -> Self {
        // Few levels, since each one is a texture.
        self.brightness = (brightness.clamp(0.0, 1.0) * 16.0).round() / 16.0;
        self
    }

    /// The name screen readers announce, instead of the one in [`Labels`](crate::Labels).
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Turns the control off: it fades, leaves the tab order and ignores input.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl View for ColorWheel {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let labels = cx.labels();
        let current = self.value.get();
        let (edge, ink, surface) =
            (theme.color(Semantic::Border), theme.color(Semantic::Ink), theme.color(Semantic::Surface));
        let (value, opacity) = (self.brightness, if self.disabled { 0.5 } else { 1.0 });
        let node = cx.container(
            LayoutStyle {
                width: Some(self.diameter),
                height: Some(self.diameter),
                align_self: Some(Align::Start),
                ..LayoutStyle::leaf()
            },
            &[],
            Paint::Rich(Rc::new(move |frame: Rect, paint: &mut PaintCx<'_>| {
                let disc_frame = frame.inflate(-WHEEL_MARGIN);
                if disc_frame.is_empty() {
                    return;
                }
                let side = (disc_frame.width() * paint.scale).ceil() as u32;
                let scene = &mut *paint.scene;
                if let Some(disc) = wheel_disc(side, value) {
                    scene.push_image(ImageQuad { opacity, ..ImageQuad::new(disc_frame, disc) });
                }
                let radius = disc_frame.width() / 2.0;
                let ring = Quad::new(disc_frame.inflate(0.5), Color::TRANSPARENT).radius(radius + 0.5);
                scene.push_quad(ring.border(1.0, edge));
                let middle = frame.center();
                let tick = |w: f32, h: f32| Rect::new(middle.x - w / 2.0, middle.y - h / 2.0, w, h);
                // A darker gray of the disc itself, since the center is the same in any theme.
                let cross = HueSaturation::default().color(value * 0.45).with_alpha(opacity);
                scene.push_quad(Quad::new(tick(9.0, 1.0), cross));
                scene.push_quad(Quad::new(tick(1.0, 9.0), cross));
                // Two rings, one of each kind of contrast, so the puck shows over any color.
                let offset = current.offset(radius);
                let puck = Rect::new(middle.x + offset.x - 6.5, middle.y + offset.y - 6.5, 13.0, 13.0);
                scene.push_quad(
                    Quad::new(puck, current.color(value).with_alpha(opacity))
                        .radius(6.5)
                        .border(2.0, surface),
                );
                scene
                    .push_quad(Quad::new(puck.inflate(1.0), Color::TRANSPARENT).radius(7.5).border(1.0, ink));
            })),
        );
        let name = self.label.clone().unwrap_or_else(|| labels.color_wheel.clone());
        let text = format!(
            "{} {}°, {} {}%",
            labels.color_hue,
            current.hue.round(),
            labels.color_saturation,
            (current.saturation * 100.0).round()
        );
        cx.semantics(node, Semantics::new(Role::Slider).name(name).value(text).disabled(self.disabled));
        if self.disabled {
            return node;
        }
        cx.focusable(node, self.diameter / 2.0);
        cx.cursor(node, CursorIcon::Pointer);
        let (dragged, modifiers) = (self.value.clone(), cx.modifiers());
        cx.on_drag(
            node,
            Rc::new(move |d| {
                if d.phase == DragPhase::End {
                    return;
                }
                let middle = d.frame.center();
                let radius = (d.frame.width() / 2.0 - WHEEL_MARGIN).max(1.0);
                let offset = Point::new(d.position.x - middle.x, d.position.y - middle.y);
                let picked = HueSaturation::at(offset, radius);
                let hue = if modifiers.get().shift { dragged.get().hue } else { picked.hue };
                dragged.set(HueSaturation::new(hue, picked.saturation));
            }),
        );
        let reset = self.value.clone();
        cx.on_double_click(node, Rc::new(move || reset.update(|v| v.saturation = 0.0)));
        let keyed = self.value.clone();
        cx.on_key(
            node,
            Rc::new(move |key, modifiers| {
                let times = if modifiers.shift { 10.0 } else { 1.0 };
                let now = keyed.get();
                keyed.set(match key {
                    Key::ArrowLeft => HueSaturation::new(now.hue - times, now.saturation),
                    Key::ArrowRight => HueSaturation::new(now.hue + times, now.saturation),
                    Key::ArrowUp => HueSaturation::new(now.hue, now.saturation + times / 100.0),
                    Key::ArrowDown => HueSaturation::new(now.hue, now.saturation - times / 100.0),
                    Key::Home => HueSaturation::new(now.hue, 0.0),
                    _ => return false,
                });
                true
            }),
        );
        node
    }
}

// ---------------------------------------------------------------------------------------------
// CurveEditor
// ---------------------------------------------------------------------------------------------

/// Room around the plot of a [`CurveEditor`], in logical pixels, for the handles at its ends.
const CURVE_MARGIN: f32 = 7.0;
/// How close to a handle a press takes it, in logical pixels.
const CURVE_REACH: f32 = 11.0;
/// The least horizontal distance between two points, as a fraction of the plot.
const CURVE_MIN_GAP: f32 = 0.01;

/// The slope at each point of a curve that never swings past its points (Fritsch-Carlson,
/// with the same harmonic mean as the charts' monotone curve, so both read alike).
fn monotone_slopes(points: &[Point]) -> Vec<f32> {
    let secant = |a: Point, b: Point| {
        let run = b.x - a.x;
        if run.abs() < f32::EPSILON { 0.0 } else { (b.y - a.y) / run }
    };
    let last = points.len() - 1;
    (0..points.len())
        .map(|i| {
            if i == 0 {
                secant(points[0], points[1])
            } else if i == last {
                secant(points[last - 1], points[last])
            } else {
                let (before, after) = (secant(points[i - 1], points[i]), secant(points[i], points[i + 1]));
                if before * after <= 0.0 { 0.0 } else { 2.0 * before * after / (before + after) }
            }
        })
        .collect()
}

/// The points in order, or a straight line when there are too few to make a curve.
fn curve_points(points: &[Point]) -> Vec<Point> {
    let mut sorted: Vec<Point> = points
        .iter()
        .filter(|p| p.x.is_finite() && p.y.is_finite())
        .map(|p| Point::new(p.x.clamp(0.0, 1.0), p.y.clamp(0.0, 1.0)))
        .collect();
    sorted.sort_by(|a, b| a.x.total_cmp(&b.x));
    sorted.dedup_by(|a, b| (a.x - b.x).abs() < f32::EPSILON);
    if sorted.len() < 2 { vec![Point::ZERO, Point::new(1.0, 1.0)] } else { sorted }
}

/// The plot of a [`CurveEditor`] inside its frame.
#[derive(Clone, Copy)]
struct CurvePlot(Rect);

impl CurvePlot {
    fn of(frame: Rect) -> Self {
        Self(frame.inflate(-CURVE_MARGIN))
    }

    fn screen(self, p: Point) -> Point {
        Point::new(self.0.x() + p.x * self.0.width(), self.0.max_y() - p.y * self.0.height())
    }

    fn value(self, at: Point) -> Point {
        Point::new(
            ((at.x - self.0.x()) / self.0.width().max(1.0)).clamp(0.0, 1.0),
            ((self.0.max_y() - at.y) / self.0.height().max(1.0)).clamp(0.0, 1.0),
        )
    }

    /// The point whose handle is within reach of `at`, the closest one.
    fn handle_at(self, points: &[Point], at: Point) -> Option<usize> {
        let distance = |p: &Point| {
            let s = self.screen(*p);
            (s.x - at.x).hypot(s.y - at.y)
        };
        points
            .iter()
            .enumerate()
            .filter(|(_, p)| distance(p) <= CURVE_REACH)
            .min_by(|a, b| distance(a.1).total_cmp(&distance(b.1)))
            .map(|(i, _)| i)
    }
}

/// Moves a point, staying between its neighbors so the curve remains a function.
fn curve_move(points: &mut [Point], index: usize, to: Point) {
    let least = if index == 0 { 0.0 } else { points[index - 1].x + CURVE_MIN_GAP };
    let most = points.get(index + 1).map_or(1.0, |next| next.x - CURVE_MIN_GAP);
    if let Some(point) = points.get_mut(index) {
        *point = Point::new(to.x.clamp(least, most.max(least)), to.y.clamp(0.0, 1.0));
    }
}

/// Adds a point in its place, or finds none if it would sit on top of another.
fn curve_insert(points: &mut Vec<Point>, at: Point) -> Option<usize> {
    if points.iter().any(|p| (p.x - at.x).abs() < CURVE_MIN_GAP) {
        return None;
    }
    let index = points.iter().position(|p| p.x > at.x).unwrap_or(points.len());
    points.insert(index, at);
    Some(index)
}

/// Editable points over a smooth curve that never swings past them. A press on empty space
/// adds a point and a drag moves it; a double click or Delete removes one, except the two
/// ends. From the keyboard, the arrows move the chosen point (with Shift, further), Page Up
/// and Page Down choose another, and Enter adds one on the curve halfway to the next.
///
/// Points go from 0 to 1 on both axes, with the input in `x`. They are kept in order of `x`.
///
/// ```
/// use ravenrs_core::Point;
/// use ravenrs_reactive::State;
/// use ravenrs_widgets::CurveEditor;
///
/// let tones = State::new(vec![Point::ZERO, Point::new(0.4, 0.55), Point::new(1.0, 1.0)]);
/// let editor = CurveEditor::new(tones.binding()).height(220.0).label("Tone curve");
/// let middle = CurveEditor::value_at(&tones.get(), 0.5);
/// assert!(middle > 0.55 && middle < 1.0);
/// ```
#[derive(Debug)]
pub struct CurveEditor {
    points: Binding<Vec<Point>>,
    width: Option<f32>,
    height: f32,
    color: Option<Color>,
    baseline: bool,
    grid: u32,
    backdrop: Vec<HistogramChannel>,
    backdrop_scale: HistogramScale,
    label: Option<String>,
    disabled: bool,
}

impl CurveEditor {
    /// An editor of `points`, across its parent, 200 logical pixels tall.
    pub fn new(points: Binding<Vec<Point>>) -> Self {
        Self {
            points,
            width: None,
            height: 200.0,
            color: None,
            baseline: true,
            grid: 4,
            backdrop: Vec::new(),
            backdrop_scale: HistogramScale::Linear,
            label: None,
            disabled: false,
        }
    }

    /// What the curve through `points` gives for the input `x`, for applying it. It uses the
    /// same curve the editor draws.
    pub fn value_at(points: &[Point], x: f32) -> f32 {
        let points = curve_points(points);
        let slopes = monotone_slopes(&points);
        curve_value(&points, &slopes, x)
    }

    /// Fixed size in logical pixels.
    pub fn size(mut self, width: f32, height: f32) -> Self {
        self.width = Some(width);
        self.height = height;
        self
    }

    /// Height in logical pixels.
    pub fn height(mut self, height: f32) -> Self {
        self.height = height;
        self
    }

    /// The color of the curve and its handles, as data, such as a channel's (ink by default).
    pub fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }

    /// Shows the straight line of no change behind the curve (on by default).
    pub fn baseline(mut self, baseline: bool) -> Self {
        self.baseline = baseline;
        self
    }

    /// Divides the plot in this many parts each way, with light lines (4 by default).
    pub fn grid(mut self, parts: u32) -> Self {
        self.grid = parts;
        self
    }

    /// Shows these channels behind the curve, as a [`Histogram`] does, faintly.
    pub fn histogram(
        mut self,
        channels: impl IntoIterator<Item = HistogramChannel>,
        scale: HistogramScale,
    ) -> Self {
        self.backdrop = channels.into_iter().collect();
        self.backdrop_scale = scale;
        self
    }

    /// The name screen readers announce, instead of the one in [`Labels`](crate::Labels).
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Turns the control off: it fades, leaves the tab order and ignores input.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

/// The curve at `x`, as the cubic between the two points around it.
fn curve_value(points: &[Point], slopes: &[f32], x: f32) -> f32 {
    let x = x.clamp(points[0].x, points[points.len() - 1].x);
    let i = points.iter().rposition(|p| p.x <= x).unwrap_or(0).min(points.len() - 2);
    let (a, b) = (points[i], points[i + 1]);
    let run = (b.x - a.x).max(f32::EPSILON);
    let t = (x - a.x) / run;
    let (t2, t3) = (t * t, t * t * t);
    let value = (2.0 * t3 - 3.0 * t2 + 1.0) * a.y
        + (t3 - 2.0 * t2 + t) * run * slopes[i]
        + (-2.0 * t3 + 3.0 * t2) * b.y
        + (t3 - t2) * run * slopes[i + 1];
    value.clamp(0.0, 1.0)
}

impl View for CurveEditor {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let labels = cx.labels();
        let points = curve_points(&self.points.get());
        let chosen = cx.use_state(|| None::<usize>);
        let chosen_now = chosen.get().filter(|i| *i < points.len());
        let (background, grid, muted) =
            (theme.color(Semantic::Canvas), theme.color(Semantic::Border), theme.color(Semantic::InkMuted));
        let (surface, focus) = (theme.color(Semantic::Surface), theme.color(Semantic::Accent));
        let opacity = if self.disabled { 0.5 } else { 1.0 };
        let curve_color = self.color.unwrap_or_else(|| theme.color(Semantic::Ink)).with_alpha(opacity);
        let backdrop = (!self.backdrop.is_empty()).then(|| HistogramPicture {
            channels: self.backdrop.iter().cloned().collect(),
            scale: self.backdrop_scale,
            dim: overlap_dim(background),
            opacity: 0.35,
        });
        let (parts, baseline, radius) = (self.grid, self.baseline, theme.radii.sm);
        let drawn = points.clone();
        let node = cx.container(
            LayoutStyle {
                width: self.width,
                height: Some(self.height),
                align_self: Some(if self.width.is_some() { Align::Start } else { Align::Stretch }),
                ..LayoutStyle::leaf()
            },
            &[],
            Paint::Custom(Rc::new(move |frame: Rect, scene| {
                scene.push_quad(Quad::new(frame, background).radius(radius).border(1.0, grid));
                let plot = CurvePlot::of(frame);
                let area = plot.0;
                if area.is_empty() {
                    return;
                }
                if let Some(backdrop) = &backdrop {
                    backdrop.paint(scene, area);
                }
                for part in 0..=parts.max(1) {
                    let share = part as f32 / parts.max(1) as f32;
                    let line = grid.with_alpha(grid.a * 0.6);
                    let x = (area.x() + area.width() * share).round();
                    let y = (area.y() + area.height() * share).round();
                    scene.push_quad(Quad::new(Rect::new(x, area.y(), 1.0, area.height()), line));
                    scene.push_quad(Quad::new(Rect::new(area.x(), y, area.width(), 1.0), line));
                }
                if baseline {
                    let line = [plot.screen(Point::ZERO), plot.screen(Point::new(1.0, 1.0))];
                    scene.push_path(PathFill::solid(stroke_outline(&line, 1.0), muted.with_alpha(0.6)));
                }
                // One sample every two pixels looks smooth at these line widths.
                let slopes = monotone_slopes(&drawn);
                let samples = ((area.width() / 2.0).ceil() as usize).max(2);
                let (from, to) = (drawn[0].x, drawn[drawn.len() - 1].x);
                let mut line: Vec<Point> = Vec::with_capacity(samples + 3);
                if from > 0.0 {
                    line.push(plot.screen(Point::new(0.0, drawn[0].y)));
                }
                line.extend((0..=samples).map(|i| {
                    let x = from + (to - from) * i as f32 / samples as f32;
                    plot.screen(Point::new(x, curve_value(&drawn, &slopes, x)))
                }));
                if to < 1.0 {
                    line.push(plot.screen(Point::new(1.0, drawn[drawn.len() - 1].y)));
                }
                scene.push_path(PathFill::solid(stroke_outline(&line, 1.75), curve_color));
                for (i, p) in drawn.iter().enumerate() {
                    let at = plot.screen(*p);
                    // The chosen one is larger and filled, so it does not depend on color.
                    let (half, fill) = if chosen_now == Some(i) { (6.0, focus) } else { (4.5, surface) };
                    let handle = Rect::new(at.x - half, at.y - half, half * 2.0, half * 2.0);
                    scene.push_quad(
                        Quad::new(handle, fill.with_alpha(opacity)).radius(half).border(1.5, curve_color),
                    );
                }
            })),
        );
        let name = self.label.clone().unwrap_or_else(|| labels.curve_editor.clone());
        let percent = |v: f32| (v * 100.0).round();
        let value = match chosen_now {
            Some(i) => format!(
                "{} {}/{}: {} {}%, {} {}%",
                labels.curve_point,
                i + 1,
                points.len(),
                labels.curve_input,
                percent(points[i].x),
                labels.curve_output,
                percent(points[i].y),
            ),
            None => format!("{} {}", points.len(), labels.curve_points),
        };
        cx.semantics(node, Semantics::new(Role::Slider).name(name).value(value).disabled(self.disabled));
        if self.disabled {
            return node;
        }
        cx.focusable(node, theme.radii.sm);
        cx.cursor(node, CursorIcon::Pointer);
        let press = cx.use_state(|| Rc::new(Cell::new((Point::ZERO, Rect::ZERO)))).get();

        let (binding, picked, pressed) = (self.points.clone(), chosen.clone(), press.clone());
        cx.on_drag(
            node,
            Rc::new(move |d| {
                let plot = CurvePlot::of(d.frame);
                let mut points = curve_points(&binding.get());
                match d.phase {
                    DragPhase::Start => {
                        pressed.set((d.position, d.frame));
                        let index = plot.handle_at(&points, d.position).or_else(|| {
                            let added = curve_insert(&mut points, plot.value(d.position));
                            if added.is_some() {
                                binding.set(points.clone());
                            }
                            added
                        });
                        picked.set(index);
                    }
                    DragPhase::Move => {
                        if let Some(index) = *picked.peek() {
                            curve_move(&mut points, index, plot.value(d.position));
                            binding.set(points);
                        }
                    }
                    DragPhase::End => {}
                }
            }),
        );

        let (binding, picked) = (self.points.clone(), chosen.clone());
        cx.on_double_click(
            node,
            Rc::new(move || {
                let (position, frame) = press.get();
                let mut points = curve_points(&binding.get());
                let hit = CurvePlot::of(frame).handle_at(&points, position);
                if let Some(index) = hit.filter(|i| *i != 0 && *i + 1 != points.len()) {
                    points.remove(index);
                    binding.set(points);
                    picked.set(None);
                }
            }),
        );

        let (binding, picked) = (self.points.clone(), chosen);
        cx.on_key(
            node,
            Rc::new(move |key, modifiers| {
                let mut points = curve_points(&binding.get());
                let current = picked.peek().filter(|i| *i < points.len());
                let last = points.len() - 1;
                let step = if modifiers.shift { 0.05 } else { 0.01 };
                let nudge = |x: f32, y: f32| {
                    let index = current.unwrap_or(0);
                    let was = points[index];
                    (index, Point::new(was.x + x * step, was.y + y * step))
                };
                let moved = match key {
                    Key::ArrowLeft => nudge(-1.0, 0.0),
                    Key::ArrowRight => nudge(1.0, 0.0),
                    Key::ArrowUp => nudge(0.0, 1.0),
                    Key::ArrowDown => nudge(0.0, -1.0),
                    Key::PageUp => {
                        picked.set(Some(current.map_or(last, |i| i.saturating_sub(1))));
                        return true;
                    }
                    Key::PageDown => {
                        picked.set(Some(current.map_or(0, |i| (i + 1).min(last))));
                        return true;
                    }
                    Key::Enter => {
                        let index = current.unwrap_or(0).min(last - 1);
                        let x = (points[index].x + points[index + 1].x) / 2.0;
                        let slopes = monotone_slopes(&points);
                        let at = Point::new(x, curve_value(&points, &slopes, x));
                        if let Some(added) = curve_insert(&mut points, at) {
                            binding.set(points);
                            picked.set(Some(added));
                        }
                        return true;
                    }
                    Key::Delete | Key::Backspace => {
                        let Some(index) = current.filter(|i| *i != 0 && *i != last) else {
                            return false;
                        };
                        points.remove(index);
                        binding.set(points);
                        picked.set(Some(index.min(last - 1)));
                        return true;
                    }
                    _ => return false,
                };
                curve_move(&mut points, moved.0, moved.1);
                binding.set(points);
                picked.set(Some(moved.0));
                true
            }),
        );
        node
    }
}
