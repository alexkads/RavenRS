//! Charts and barcodes. The layout comes from `ravenrs-graphics`, the same as in
//! reports; here its shapes become a picture and its labels become text. Both are
//! kept between frames, so a rebuild that changes nothing uploads nothing.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;

use ravenrs_animation::{CubicBezier, Easing, Transition};
use ravenrs_core::{
    Brush, Color, CursorIcon, Effect, ImageData, ImageQuad, Insets, Key, PathElement, PathFill, Point, Quad,
    Rect, Role, Size,
};
use ravenrs_graphics::barcode::{self, Pattern};
use ravenrs_graphics::chart::{
    self, ChartData, ChartDrawing, ChartLabel, ChartOptions, ChartStyle, LabelAlign, SeriesData,
};
use ravenrs_graphics::plot::{self, Plot, PlotStyle};
use ravenrs_graphics::vector::{self, PathSegment, Shape};
use ravenrs_layout::{Align, Axis, Justify, LayoutStyle};
use ravenrs_reactive::{Binding, State};
use ravenrs_style::{Semantic, TextStyle, Theme, Weight};
use ravenrs_text::{TextAttrs, TextLayout};

pub use ravenrs_graphics::barcode::Symbology;
pub use ravenrs_graphics::chart::ChartKind;
pub use ravenrs_graphics::plot::{
    Curve as ChartCurve, Grid as ChartGrid, LabelAt as ChartLabelAt, PolarGrid as ChartPolarGrid,
    Stack as ChartStack,
};

use crate::context::{BuildCx, NodeId, Paint, PaintCx, Placement, QuadPaint, Semantics};
use crate::motion::{self, Motion};
use crate::{AnyView, Card, IconName, View, util};

/// How a chart with [`Chart::animate_changes`] goes from its old data to the new: quick to
/// start, settling softly, in the curve the data draws in with.
const CHANGE: Transition = Transition::tween(0.75, Easing::Bezier(CubicBezier::new(0.25, 1.0, 0.5, 1.0)));
/// The widest a bar gets, so a few bars in a wide chart still read as bars.
const MAX_BAR_WIDTH: f32 = 44.0;
const LABEL_LINE_HEIGHT: f32 = 1.3;
/// How many values a screen reader hears before the rest is left out.
const SPOKEN_VALUES: usize = 12;
const BARCODE_PADDING: f32 = 8.0;

const LEGEND_SWATCH: f32 = 8.0;
const LEGEND_ICON: f32 = 12.0;
const TOOLTIP_MIN_WIDTH: f32 = 128.0;
const TOOLTIP_INDICATOR: f32 = 10.0;
const ACTIVE_DOT: f32 = 5.0;
/// How far the ring around an active dot reaches past it.
const ACTIVE_HALO: f32 = 5.0;
/// How solid that ring is.
const HALO_ALPHA: f32 = 0.22;
/// How much of its strength a chart gives up around the bar or slice in focus.
const FOCUS_DIM: f32 = 0.45;
/// How far a slice in focus comes forward. It stays under the gap a chart keeps
/// from its title and legend, so a pie needs no smaller radius to make room for it.
const SLICE_LIFT: f32 = 6.0;
/// How much a slice's way forward it takes it to be solid.
const LIFT_SOLID_FROM: f32 = 4.0;
/// How solid the line through the points of the category in focus is.
const CURSOR_ALPHA: f32 = 0.55;
const DOT_ICON: f32 = 18.0;
const CANDLE_OUTLINE: f32 = 1.5;

/// Lays a drawing out in a rectangle, at a number of pixels per logical pixel, and
/// with a way to know how wide its text is.
type DrawingFn = Rc<dyn Fn(Rect, f32, plot::Measure<'_>) -> Plot>;
#[doc(hidden)]
pub type ValueFormat = Rc<dyn Fn(f64) -> String>;
/// Writes the panel of a tooltip, given the index and the name of a category.
type TooltipContent = Rc<dyn Fn(usize, &str) -> AnyView>;
/// The color of a value in a tooltip, given its series and the value.
type ValueColor = Rc<dyn Fn(usize, f64) -> Option<Semantic>>;
type TickFormat = Rc<dyn Fn(&str) -> String>;
/// Which tab is chosen, and the name and the total of each.
type ChartTabs = (Binding<usize>, Vec<(String, String)>);

/// Bytes of pictures kept after their chart is gone: about ten charts filling a laptop's
/// screen at 2x.
const PICTURES_KEPT_BYTES: usize = 128 << 20;

/// A picture that is not on screen anymore.
struct KeptPicture {
    layers: Vec<vector::Layer>,
    size: (u32, u32),
    image: Option<ImageData>,
    bytes: usize,
}

thread_local! {
    /// Drawing a chart on the CPU is what costs most in opening a screen, and a screen
    /// that opens again shows the same chart. The most recent one comes last.
    static PICTURES: RefCell<Vec<KeptPicture>> = const { RefCell::new(Vec::new()) };
}

/// The part of `area` these layers draw on, on whole pixels: drawing and sending the
/// rest of the frame, empty, is what a chart's picture spends the most on.
fn drawn_part(layers: &[vector::Layer], area: Rect, scale: f32) -> Option<Rect> {
    let (mut x0, mut y0, mut x1, mut y1) =
        (f32::INFINITY, f32::INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY);
    for bounds in layers.iter().filter_map(vector::Layer::bounds) {
        x0 = x0.min(bounds.x());
        y0 = y0.min(bounds.y());
        x1 = x1.max(bounds.max_x());
        y1 = y1.max(bounds.max_y());
    }
    let (left, top) = ((x0.max(area.x()) * scale).floor(), (y0.max(area.y()) * scale).floor());
    let (right, bottom) = ((x1.min(area.max_x()) * scale).ceil(), (y1.min(area.max_y()) * scale).ceil());
    (right > left && bottom > top)
        .then(|| Rect::new(left / scale, top / scale, (right - left) / scale, (bottom - top) / scale))
}

/// The picture of these layers, drawn now only if none is kept.
fn picture_of(layers: &[vector::Layer], area: Rect, scale: f32) -> Option<ImageData> {
    let size = ((area.width() * scale).round() as u32, (area.height() * scale).round() as u32);
    PICTURES.with_borrow_mut(|kept| {
        if let Some(found) = kept.iter().position(|p| p.size == size && p.layers == layers) {
            let picture = kept.remove(found);
            let image = picture.image.clone();
            kept.push(picture);
            return image;
        }
        // A little under its width, for rounding not to make the picture a pixel wider.
        let whole = Rect::new(area.x(), area.y(), area.width() - 0.01 / scale, area.height() - 0.01 / scale);
        let image = vector::rasterize_layers(layers, whole, scale);
        let bytes = size.0 as usize * size.1 as usize * 4;
        kept.push(KeptPicture { layers: layers.to_vec(), size, image: image.clone(), bytes });
        let mut total: usize = kept.iter().map(|p| p.bytes).sum();
        while total > PICTURES_KEPT_BYTES && kept.len() > 1 {
            total -= kept.remove(0).bytes;
        }
        image
    })
}

/// What a drawing looked like when it was last painted, relative to its frame.
#[derive(Default)]
struct Picture {
    source: Option<DrawingFn>,
    size: Size,
    scale: f32,
    drawing: Plot,
    image: Option<(Rect, ImageData)>,
    labels: Vec<(TextLayout, Point, Color)>,
    dot_icons: Vec<(TextLayout, Point, Color)>,
    /// How wide each label is, as a rebuild asks again for the same ones.
    widths: HashMap<u32, HashMap<String, f32>>,
    /// The picture split in its backdrop, its data and its notes, drawn the first time
    /// a draw-in asks for it.
    parts: Option<Parts>,
}

/// The pictures of a chart's backdrop, of its data and of its notes, apart, for its
/// data to draw in over a backdrop that stays put.
#[derive(Default)]
struct Parts {
    backdrop: Option<(Rect, ImageData)>,
    data: Option<(Rect, ImageData)>,
    notes: Option<(Rect, ImageData)>,
}

/// Measured widths a chart keeps, more than even a long series has categories: one
/// forgotten while the same labels are still in use would be measured again each frame.
const WIDTHS_KEPT: usize = 1 << 15;

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl Picture {
    /// Brings the picture up to date, redoing only what changed.
    fn refresh(&mut self, source: &DrawingFn, dot_icon: Option<IconName>, size: Size, cx: &mut PaintCx<'_>) {
        let same_source = self.source.as_ref().is_some_and(|known| Rc::ptr_eq(known, source));
        let same_pixels = self.size == size && self.scale == cx.scale;
        if same_source && same_pixels {
            return;
        }
        let area = Rect::new(0.0, 0.0, size.width, size.height);
        if self.scale != cx.scale || self.widths.values().map(HashMap::len).sum::<usize>() > WIDTHS_KEPT {
            self.widths.clear();
        }
        let drawing = {
            let (text, widths) = (RefCell::new(&mut *cx.text), RefCell::new(&mut self.widths));
            let measure = |content: &str, size: f32| {
                let mut widths = widths.borrow_mut();
                let of_size = widths.entry(size.to_bits()).or_default();
                if let Some(width) = of_size.get(content) {
                    return *width;
                }
                let attrs = label_attrs(size, false);
                let width = text.borrow_mut().layout(content, &attrs).measure(None).width;
                of_size.insert(content.to_owned(), width);
                width
            };
            source(area, cx.scale, &measure)
        };
        if !same_pixels || drawing.layers != self.drawing.layers {
            self.parts = None;
            self.image = drawn_part(&drawing.layers, area, cx.scale)
                .and_then(|part| picture_of(&drawing.layers, part, cx.scale).map(|image| (part, image)));
        }
        if drawing.labels != self.drawing.labels {
            self.labels = drawing.labels.iter().map(|label| typeset(label, cx)).collect();
        }
        if drawing.marks != self.drawing.marks {
            self.dot_icons = dot_icon.map_or_else(Vec::new, |icon| {
                let mut glyph = [0u8; 4];
                let glyph = icon.glyph().encode_utf8(&mut glyph);
                let half = DOT_ICON / 2.0;
                drawing
                    .marks
                    .iter()
                    .map(|mark| {
                        let mut layout = cx.text.layout(glyph, &util::icon_attrs(DOT_ICON));
                        layout.measure(None);
                        (layout, Point::new(mark.at.x - half, mark.at.y - half), mark.color)
                    })
                    .collect()
            });
        }
        self.source = Some(source.clone());
        self.size = size;
        self.scale = cx.scale;
        self.drawing = drawing;
    }

    /// Its backdrop, its data and its notes, each a picture of its own. They are only
    /// drawn while the data draws in, and kept with the other pictures.
    fn parts(&mut self) -> &Parts {
        if self.parts.is_none() {
            let area = Rect::new(0.0, 0.0, self.size.width, self.size.height);
            let scale = self.scale;
            let image = |layers: Vec<vector::Layer>| {
                drawn_part(&layers, area, scale)
                    .and_then(|part| picture_of(&layers, part, scale).map(|image| (part, image)))
            };
            let plot = &self.drawing;
            let backdrop = plot.backdrop.min(plot.layers.len());
            let (mut data, mut notes) = (Vec::new(), Vec::new());
            for (i, layer) in plot.layers.iter().enumerate().skip(backdrop) {
                if plot.notes.contains(&i) { notes.push(layer.clone()) } else { data.push(layer.clone()) }
            }
            self.parts = Some(Parts {
                backdrop: image(plot.layers[..backdrop].to_vec()),
                data: image(data),
                notes: image(notes),
            });
        }
        self.parts.get_or_insert_with(Parts::default)
    }
}

/// How a chart's data draws in the first time it is painted. Its axes, its grid and
/// their labels are there from the first frame.
#[derive(Clone, Copy, Debug, PartialEq)]
enum DrawIn {
    /// From the left across, as a pen would: lines, areas, dots and candles.
    Reveal,
    /// Out of the zero line, all bars together, a negative one to its own side.
    Grow { horizontal: bool },
    /// Clockwise from the first slice, with the labels coming in at the end.
    Sweep { separator: f32 },
    /// Out of the center, over rings and spokes that stay put.
    Spread,
}

/// How far a chart's data has drawn in, from 0 to 1, and how it does.
#[derive(Clone, Copy, Debug)]
struct Appear {
    how: DrawIn,
    progress: f32,
}

/// How far into a sweep the labels and their guides start to come in, once the
/// slices are mostly there.
const LABELS_COME_IN: f32 = 0.7;
/// How far before the first point a reveal starts, so a dot there shows whole as
/// soon as the reveal reaches it.
const REVEAL_BLEED: f32 = ACTIVE_DOT;

/// What a chart's key to draw in again comes down to.
#[doc(hidden)]
pub fn appear_key_of(key: impl std::hash::Hash) -> u64 {
    use std::hash::{BuildHasher, BuildHasherDefault, DefaultHasher};
    BuildHasherDefault::<DefaultHasher>::default().hash_one(key)
}

/// Where a bar is once it has grown `t` of the way out of `baseline`.
fn grown(rect: Rect, baseline: f32, horizontal: bool, t: f32) -> Rect {
    let toward = |at: f32| baseline + (at - baseline) * t;
    if horizontal {
        let (a, b) = (toward(rect.x()), toward(rect.max_x()));
        Rect::new(a, rect.y(), b - a, rect.height())
    } else {
        let (a, b) = (toward(rect.y()), toward(rect.max_y()));
        Rect::new(rect.x(), a, rect.width(), b - a)
    }
}

/// The angles of a slice `t` of the way into a sweep that starts at `start`.
fn swept(from: f32, to: f32, start: f32, t: f32) -> (f32, f32) {
    (start + (from - start) * t, start + (to - start) * t)
}

/// The part of a frame a reveal shows, `t` of the way from the plot's left edge to
/// the frame's right one. It bleeds a dot's width past both, so dots show whole.
#[doc(hidden)]
pub fn revealed(frame: Rect, area: Rect, t: f32) -> Rect {
    let left = frame.x() + area.x() - REVEAL_BLEED;
    let right = left + (frame.max_x() + REVEAL_BLEED - left) * t;
    let top = frame.y() - REVEAL_BLEED;
    let x = frame.x() - REVEAL_BLEED;
    Rect::new(x, top, (right - x).max(0.0), frame.height() + REVEAL_BLEED * 2.0)
}

/// One of the pictures, at `opacity`, scaled `by` around a point when it grows.
fn paint_image(
    part: Option<&(Rect, ImageData)>,
    frame: Rect,
    opacity: f32,
    by: Option<(Point, f32)>,
    cx: &mut PaintCx<'_>,
) {
    let Some((part, image)) = part else { return };
    if opacity <= 0.0 {
        return;
    }
    let part = match by {
        Some((center, k)) => Rect::new(
            center.x + (part.x() - center.x) * k,
            center.y + (part.y() - center.y) * k,
            part.width() * k,
            part.height() * k,
        ),
        None => *part,
    };
    if part.width() <= 0.0 || part.height() <= 0.0 {
        return;
    }
    let at = Rect::new(frame.x() + part.x(), frame.y() + part.y(), part.width(), part.height());
    cx.scene.push_image(ImageQuad { opacity, ..ImageQuad::new(at, image.clone()) });
}

/// The labels, or only those on the data (`Some(true)`) or the others (`Some(false)`),
/// at a share of their strength.
fn paint_labels(picture: &Picture, frame: Rect, on_data: Option<bool>, strength: f32, cx: &mut PaintCx<'_>) {
    if strength <= 0.0 {
        return;
    }
    for (i, (layout, at, color)) in picture.labels.iter().enumerate() {
        if on_data.is_some_and(|wanted| picture.drawing.data_labels.binary_search(&i).is_ok() != wanted) {
            continue;
        }
        let origin = Point::new(frame.x() + at.x, frame.y() + at.y);
        layout.paint(origin, color.with_alpha(color.a * strength), cx.scene);
    }
}

fn paint_dot_icons(picture: &Picture, frame: Rect, strength: f32, cx: &mut PaintCx<'_>) {
    if strength <= 0.0 {
        return;
    }
    for (layout, at, color) in &picture.dot_icons {
        let origin = Point::new(frame.x() + at.x, frame.y() + at.y);
        layout.paint(origin, color.with_alpha(color.a * strength), cx.scene);
    }
}

/// The slices of a round chart `t` of the way into their sweep, each ring from its
/// own first slice.
fn paint_swept_slices(plot: &Plot, frame: Rect, t: f32, separator: f32, cx: &mut PaintCx<'_>) {
    for sector in &plot.sectors {
        let start = plot
            .sectors
            .iter()
            .filter(|s| s.series == sector.series)
            .map(|s| s.from)
            .fold(f32::INFINITY, f32::min);
        let (from, to) = swept(sector.from, sector.to, start, t);
        if to - from <= f32::EPSILON {
            continue;
        }
        let path = plot::gapped_slice(sector.center, sector.inner, sector.outer, from, to, separator);
        cx.scene.push_path(PathFill {
            transform: [1.0, 0.0, 0.0, 1.0, frame.x(), frame.y()],
            ..PathFill::solid(path_elements(&path), sector.color)
        });
    }
}

/// The data while it draws in, with its labels and notes, but not the labels of the axes.
fn paint_drawing_in(appear: Appear, picture: &mut Picture, frame: Rect, cx: &mut PaintCx<'_>) {
    let t = appear.progress.clamp(0.0, 1.0);
    // The grid is there from the first frame. Only bars, as they grow, cut it anew.
    if !matches!(appear.how, DrawIn::Grow { .. }) {
        paint_rules(&picture.drawing.rules, frame, cx);
    }
    match appear.how {
        DrawIn::Reveal => {
            cx.scene.push_clip(revealed(frame, picture.drawing.area, t));
            paint_bars(None, &picture.drawing.bars, frame, cx);
            paint_candles(None, &picture.drawing, frame, cx);
            paint_image(picture.image.as_ref(), frame, 1.0, None, cx);
            paint_labels(picture, frame, Some(true), 1.0, cx);
            paint_dot_icons(picture, frame, 1.0, cx);
            cx.scene.pop_clip();
        }
        DrawIn::Grow { horizontal } => {
            let plot = &picture.drawing;
            let baseline =
                plot.baseline.unwrap_or(if horizontal { plot.area.x() } else { plot.area.max_y() });
            let bars: Vec<plot::Bar> = plot
                .bars
                .iter()
                .map(|bar| {
                    let mut bar = *bar;
                    bar.rect = grown(bar.rect, baseline, horizontal, t);
                    bar
                })
                .collect();
            // The grid stops at the bars as they are now, and not where they will end.
            let rules = if plot.uncut_rules.is_empty() {
                plot.rules.clone()
            } else {
                plot::rules_around_bars(&plot.uncut_rules, &bars)
            };
            paint_rules(&rules, frame, cx);
            paint_bars(None, &bars, frame, cx);
            paint_image(picture.image.as_ref(), frame, t, None, cx);
            paint_labels(picture, frame, Some(true), t, cx);
            paint_dot_icons(picture, frame, t, cx);
        }
        DrawIn::Sweep { separator } => {
            paint_swept_slices(&picture.drawing, frame, t, separator, cx);
            let late = ((t - LABELS_COME_IN) / (1.0 - LABELS_COME_IN)).clamp(0.0, 1.0);
            let notes = picture.parts().notes.clone();
            paint_image(notes.as_ref(), frame, late, None, cx);
            paint_labels(picture, frame, Some(true), late, cx);
        }
        DrawIn::Spread => {
            let parts = picture.parts();
            let (backdrop, data, notes) = (parts.backdrop.clone(), parts.data.clone(), parts.notes.clone());
            let plot = &picture.drawing;
            let center = plot.sectors.first().map_or_else(|| plot.area.center(), |s| s.center);
            paint_image(backdrop.as_ref(), frame, 1.0, None, cx);
            paint_image(data.as_ref(), frame, 1.0, Some((center, t)), cx);
            paint_image(notes.as_ref(), frame, t, None, cx);
            paint_labels(picture, frame, Some(true), t, cx);
            paint_dot_icons(picture, frame, t, cx);
        }
    }
}

#[doc(hidden)]
pub fn label_attrs(size: f32, bold: bool) -> TextAttrs {
    TextAttrs {
        size,
        line_height: size * LABEL_LINE_HEIGHT,
        weight: if bold { Weight::SEMIBOLD.0 } else { Weight::REGULAR.0 },
        letter_spacing: 0.0,
        family: None,
    }
}

#[doc(hidden)]
pub fn typeset(label: &ChartLabel, cx: &mut PaintCx<'_>) -> (TextLayout, Point, Color) {
    let attrs = label_attrs(label.size, label.bold);
    let mut layout = cx.text.layout(&label.text, &attrs);
    let measured = layout.measure(None);
    let x = match label.align {
        LabelAlign::Center => label.rect.x() + (label.rect.width() - measured.width) / 2.0,
        LabelAlign::Right => label.rect.max_x() - measured.width,
        _ => label.rect.x(),
    };
    let y = label.rect.y() + (label.rect.height() - measured.height) / 2.0;
    (layout, Point::new(x, y), label.color)
}

/// What stands out over the picture while a category is in focus, or was until a
/// moment ago: all of it fades, so it is drawn for as long as `focus` is above zero.
#[derive(Clone)]
struct Highlight {
    /// The category, between two of them while it glides from one to the other.
    at: f32,
    /// From nothing to in full.
    focus: f32,
    /// How much the picture fades at full focus.
    dim: f32,
    /// How far forward each category's slice is, from 0 to 1 (a spring may pass 1).
    lifts: Vec<f32>,
    /// The gap between slices, which a slice drawn over them has to leave alone.
    separator: f32,
    /// A strip behind the bars.
    strip: bool,
    /// A line through the points.
    line: bool,
    /// A dot on each value.
    dots: bool,
    strip_color: Color,
    line_color: Color,
    ring_color: Color,
}

fn mix(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl Highlight {
    /// The two categories it is between, and how far from the first to the second.
    fn between(&self) -> (usize, usize, f32) {
        let at = self.at.max(0.0);
        (at.floor() as usize, at.ceil() as usize, at.fract())
    }

    /// The opacity of the picture.
    fn opacity(&self) -> f32 {
        1.0 - self.dim * self.focus
    }
}

/// Keeps a thin line on whole pixels, so that it stays sharp and of one width. Its
/// place on the page is only known here, and it may fall anywhere.
fn snapped(rect: Rect, scale: f32) -> Rect {
    let snap = |at: f32, length: f32| {
        let thick = (length * scale).round().max(1.0);
        ((at * scale).round() / scale, thick / scale)
    };
    if rect.width() < rect.height() {
        let (x, width) = snap(rect.x(), rect.width());
        Rect::new(x, rect.y(), width, rect.height())
    } else {
        let (y, height) = snap(rect.y(), rect.height());
        Rect::new(rect.x(), y, rect.width(), height)
    }
}

fn paint_rules(rules: &[plot::Rule], frame: Rect, cx: &mut PaintCx<'_>) {
    for rule in rules {
        let rect = Rect::new(
            frame.x() + rule.rect.x(),
            frame.y() + rule.rect.y(),
            rule.rect.width(),
            rule.rect.height(),
        );
        cx.scene.push_quad(Quad::new(snapped(rect, cx.scale), rule.color));
    }
}

fn paint_top_rules(plot: &Plot, frame: Rect, cx: &mut PaintCx<'_>) {
    for rule in &plot.top_rules {
        let rect = Rect::new(
            frame.x() + rule.rect.x(),
            frame.y() + rule.rect.y(),
            rule.rect.width(),
            rule.rect.height(),
        );
        cx.scene.push_quad(Quad::new(snapped(rect, cx.scale), rule.color));
    }
}

fn paint_strip(highlight: &Highlight, plot: &Plot, frame: Rect, cx: &mut PaintCx<'_>) {
    if !highlight.strip {
        return;
    }
    let (from, to, t) = highlight.between();
    let spot = |index: usize| plot.spots.iter().find(|s| s.index == index);
    let (Some(from), Some(to)) = (spot(from), spot(to)) else { return };
    let (a, b) = (from.band, to.band);
    let band = Rect::new(
        mix(a.x(), b.x(), t),
        mix(a.y(), b.y(), t),
        mix(a.width(), b.width(), t),
        mix(a.height(), b.height(), t),
    );
    let area = plot.area;
    let along_x = (band.width() - frame.width()).abs() > 0.5;
    let strip = if along_x {
        Rect::new(band.x(), area.y(), band.width(), area.height())
    } else {
        Rect::new(area.x(), band.y(), area.width(), band.height())
    };
    let strip = Rect::new(frame.x() + strip.x(), frame.y() + strip.y(), strip.width(), strip.height());
    let color = highlight.strip_color;
    cx.scene.push_quad(Quad::new(strip, color.with_alpha(color.a * highlight.focus)));
}

/// The bars. Around the focus they fade, less the closer they are to it, so focus
/// hands over from one category to the next as it passes.
fn paint_bars(highlight: Option<&Highlight>, bars: &[plot::Bar], frame: Rect, cx: &mut PaintCx<'_>) {
    for bar in bars {
        let shape = bar.rect;
        // One that has not started to grow yet.
        if shape.width() <= 0.0 || shape.height() <= 0.0 {
            continue;
        }
        let faded = highlight.map_or(0.0, |highlight| {
            let distance = (bar.index as f32 - highlight.at).abs().min(1.0);
            highlight.dim * highlight.focus * distance
        });
        let rect = Rect::new(frame.x() + shape.x(), frame.y() + shape.y(), shape.width(), shape.height());
        let strength = 1.0 - faded;
        let brush: Brush = match bar.gradient {
            Some(gradient) => ravenrs_core::LinearGradient {
                from: gradient.from.with_alpha(gradient.from.a * strength),
                to: gradient.to.with_alpha(gradient.to.a * strength),
                ..gradient
            }
            .into(),
            None => bar.color.with_alpha(bar.color.a * strength).into(),
        };
        let mut quad = Quad::new(rect, brush);
        quad.corner_radii = bar.radii;
        cx.scene.push_quad(quad);
    }
}

/// The candles. A body that rose is an outline and one that fell is solid, so that
/// which it did does not rest on the color alone.
fn paint_candles(highlight: Option<&Highlight>, plot: &Plot, frame: Rect, cx: &mut PaintCx<'_>) {
    let place =
        |rect: Rect| Rect::new(frame.x() + rect.x(), frame.y() + rect.y(), rect.width(), rect.height());
    for candle in &plot.candles {
        let faded = highlight.map_or(0.0, |highlight| {
            let distance = (candle.index as f32 - highlight.at).abs().min(1.0);
            highlight.dim * highlight.focus * distance
        });
        let color = candle.color.with_alpha(candle.color.a * (1.0 - faded));
        for wick in candle.wicks {
            if wick.height() > 0.0 {
                cx.scene.push_quad(Quad::new(snapped(place(wick), cx.scale), color));
            }
        }
        let body =
            Quad::new(place(candle.body), if candle.rising { Color::TRANSPARENT } else { color }).radius(1.0);
        cx.scene.push_quad(if candle.rising { body.border(CANDLE_OUTLINE, color) } else { body });
    }
}

#[doc(hidden)]
pub fn path_elements(path: &[PathSegment]) -> Vec<PathElement> {
    path.iter()
        .map(|segment| match segment {
            PathSegment::Move(p) => PathElement::MoveTo(*p),
            PathSegment::Line(p) => PathElement::LineTo(*p),
            PathSegment::Cubic(a, b, c) => PathElement::CubicTo(*a, *b, *c),
            _ => PathElement::Close,
        })
        .collect()
}

/// The slices that came forward, over a picture that faded.
fn paint_lifted_slices(highlight: &Highlight, plot: &Plot, frame: Rect, cx: &mut PaintCx<'_>) {
    let outermost = plot.sectors.iter().map(|s| s.inner).fold(0.0, f32::max);
    for sector in &plot.sectors {
        let lift = highlight.lifts.get(sector.index).copied().unwrap_or(0.0);
        if lift <= 0.001 {
            continue;
        }
        // Only the outside ring has anywhere to go.
        let outer = if sector.inner >= outermost { sector.outer + SLICE_LIFT * lift } else { sector.outer };
        // The same gap as in the picture keeps the slice the same width as it comes forward.
        let path = plot::gapped_slice(
            sector.center,
            sector.inner,
            outer,
            sector.from,
            sector.to,
            highlight.separator,
        );
        // Solid for most of its way, so that it never looks like a ghost of itself, and
        // only at the very end does it hand over to the slice in the picture.
        let color = sector.color.with_alpha(sector.color.a * (lift * LIFT_SOLID_FROM).clamp(0.0, 1.0));
        cx.scene.push_path(PathFill {
            transform: [1.0, 0.0, 0.0, 1.0, frame.x(), frame.y()],
            ..PathFill::solid(path_elements(&path), color)
        });
    }
}

fn paint_highlight(highlight: &Highlight, plot: &Plot, frame: Rect, cx: &mut PaintCx<'_>) {
    let (from, to, t) = highlight.between();
    if highlight.line {
        let spot = |index: usize| plot.spots.iter().find(|s| s.index == index);
        if let (Some(a), Some(b)) = (spot(from), spot(to)) {
            let x = frame.x() + mix(a.center.x, b.center.x, t) - 0.5;
            let line = snapped(Rect::new(x, frame.y() + plot.area.y(), 1.0, plot.area.height()), cx.scale);
            let color = highlight.line_color;
            cx.scene.push_quad(Quad::new(line, color.with_alpha(color.a * highlight.focus)));
        }
    }
    if highlight.dots {
        let focus = highlight.focus;
        let radius = ACTIVE_DOT * mix(0.6, 1.0, focus);
        let halo = radius + ACTIVE_HALO * focus;
        for mark in plot.marks.iter().filter(|m| m.index == from) {
            let next = plot.marks.iter().find(|m| m.index == to && m.series == mark.series).unwrap_or(mark);
            let x = frame.x() + mix(mark.at.x, next.at.x, t);
            let y = frame.y() + mix(mark.at.y, next.at.y, t);
            let color = if t < 0.5 { mark.color } else { next.color };
            let round = |r: f32| Rect::new(x - r, y - r, r * 2.0, r * 2.0);
            cx.scene.push_quad(Quad::new(round(halo), color.with_alpha(HALO_ALPHA * focus)).radius(halo));
            let ring = highlight.ring_color;
            cx.scene.push_quad(
                Quad::new(round(radius), color.with_alpha(color.a * focus))
                    .radius(radius)
                    .border(2.0, ring.with_alpha(ring.a * focus)),
            );
        }
    }
}

/// A box that paints what `drawing` lays out for its frame, around `children`: what
/// stands out in it now, and how far its data has drawn in.
fn plot_node(
    cx: &mut BuildCx<'_>,
    style: LayoutStyle,
    picture: Rc<RefCell<Picture>>,
    drawing: DrawingFn,
    dot_icon: Option<IconName>,
    (highlight, appear): (Option<Highlight>, Option<Appear>),
    children: &[NodeId],
) -> NodeId {
    cx.container(
        style,
        children,
        Paint::Rich(Rc::new(move |frame: Rect, paint: &mut PaintCx<'_>| {
            if frame.is_empty() {
                return;
            }
            let mut picture = picture.borrow_mut();
            picture.refresh(&drawing, dot_icon, frame.size, paint);
            if let Some(highlight) = &highlight {
                paint_strip(highlight, &picture.drawing, frame, paint);
            }
            let drawing_in = appear.filter(|appear| appear.progress < 1.0);
            if let Some(appear) = drawing_in {
                paint_drawing_in(appear, &mut picture, frame, paint);
            } else {
                paint_rules(&picture.drawing.rules, frame, paint);
                paint_bars(highlight.as_ref(), &picture.drawing.bars, frame, paint);
                paint_candles(highlight.as_ref(), &picture.drawing, frame, paint);
                let opacity = highlight.as_ref().map_or(1.0, Highlight::opacity);
                paint_image(picture.image.as_ref(), frame, opacity, None, paint);
                if let Some(highlight) = &highlight {
                    paint_lifted_slices(highlight, &picture.drawing, frame, paint);
                }
            }
            paint_top_rules(&picture.drawing, frame, paint);
            if let Some(highlight) = &highlight {
                paint_highlight(highlight, &picture.drawing, frame, paint);
            }
            if drawing_in.is_some() {
                paint_labels(&picture, frame, Some(false), 1.0, paint);
            } else {
                paint_labels(&picture, frame, None, 1.0, paint);
                paint_dot_icons(&picture, frame, 1.0, paint);
            }
        })),
    )
}

/// A box that paints what `drawing` lays out for its frame.
fn drawing_node(cx: &mut BuildCx<'_>, style: LayoutStyle, drawing: DrawingFn) -> NodeId {
    let picture = cx.use_state(|| Rc::new(RefCell::new(Picture::default()))).peek().clone();
    plot_node(cx, style, picture, drawing, None, (None, None), &[])
}

/// The colors of a chart in this theme: none of them means anything, unlike status
/// colors, and past the theme's five come those of the shared palette.
#[doc(hidden)]
pub fn palette(theme: &Theme) -> Vec<Color> {
    [Semantic::Chart1, Semantic::Chart2, Semantic::Chart3, Semantic::Chart4, Semantic::Chart5]
        .map(|role| theme.color(role))
        .into_iter()
        .chain(chart::PALETTE[5..].iter().copied())
        .collect()
}

/// Shows a number on an axis: at most two decimals, none when they are zeros.
#[doc(hidden)]
pub fn plain_number(value: f64) -> String {
    let text = format!("{value:.2}");
    text.trim_end_matches('0').trim_end_matches('.').to_owned()
}

struct Series {
    name: String,
    color: Option<Color>,
    icon: Option<IconName>,
    values: Vec<f64>,
}

/// How a row of a [`ChartTooltip`] shows its color.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "variants"))]
pub enum ChartIndicator {
    /// A small square.
    #[default]
    Dot,
    /// A thin bar.
    Line,
    /// A thin dashed bar.
    Dashed,
    /// No color at all.
    None,
}

/// What a [`Chart`] says about the category under the pointer or the arrow keys.
///
/// ```
/// use ravenrs_widgets::{ChartIndicator, ChartTooltip};
///
/// let tooltip = ChartTooltip::new()
///     .indicator(ChartIndicator::Line)
///     .label_format(|month| format!("Month of {month}"))
///     .show_total("Total");
/// ```
#[derive(Clone, Default)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct ChartTooltip {
    indicator: ChartIndicator,
    hide_label: bool,
    label: Option<String>,
    label_format: Option<TickFormat>,
    value_format: Option<ValueFormat>,
    total: Option<String>,
    icons: bool,
    no_cursor: bool,
    percent: bool,
    value_color: Option<ValueColor>,
    no_indicator: Vec<String>,
    content: Option<TooltipContent>,
}

impl fmt::Debug for ChartTooltip {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ChartTooltip").field("indicator", &self.indicator).finish_non_exhaustive()
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl ChartTooltip {
    /// A tooltip with the category on top and a square before each series.
    pub fn new() -> Self {
        Self::default()
    }

    /// How each row shows its color.
    pub fn indicator(mut self, indicator: ChartIndicator) -> Self {
        self.indicator = indicator;
        self
    }

    /// Leaves out the category on top.
    pub fn hide_label(mut self) -> Self {
        self.hide_label = true;
        self
    }

    /// Writes this on top, in place of the category.
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Writes the category on top in another way, such as a date in full.
    pub fn label_format(mut self, format: impl Fn(&str) -> String + 'static) -> Self {
        self.label_format = Some(Rc::new(format));
        self
    }

    /// Writes values in another way than on the axis, such as with their unit.
    pub fn value_format(mut self, format: impl Fn(f64) -> String + 'static) -> Self {
        self.value_format = Some(Rc::new(format));
        self
    }

    /// Adds a last row with the sum of the series, under this name.
    pub fn show_total(mut self, name: impl Into<String>) -> Self {
        self.total = Some(name.into());
        self
    }

    /// Shows the icons of the series in place of their colors.
    pub fn icons(mut self) -> Self {
        self.icons = true;
        self
    }

    /// Writes after each value its share: of the circle in a pie or a doughnut, of
    /// the category in the others.
    pub fn show_percent(mut self) -> Self {
        self.percent = true;
        self
    }

    /// Colors a value by what it says, such as a loss in red. It is given the index
    /// of the series and the value; `None` leaves it in the color of the text.
    pub fn value_color(mut self, color: impl Fn(usize, f64) -> Option<Semantic> + 'static) -> Self {
        self.value_color = Some(Rc::new(color));
        self
    }

    /// The row of this series has no square or bar, for a value that is not one of
    /// those the chart tells apart, such as a target.
    pub fn no_indicator_for(mut self, series: impl Into<String>) -> Self {
        self.no_indicator.push(series.into());
        self
    }

    /// Writes the panel with a view of your own, given the index and the name of the
    /// category. A screen reader still hears the values.
    pub fn content(mut self, content: impl Fn(usize, &str) -> AnyView + 'static) -> Self {
        self.content = Some(Rc::new(content));
        self
    }

    /// Leaves out the strip or line that marks the category on the chart.
    pub fn no_cursor(mut self) -> Self {
        self.no_cursor = true;
        self
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose(part = "part1"))]
impl ChartTooltip {
    fn color_of_value(&self, series: usize, value: f64, theme: &Theme) -> Option<Color> {
        self.value_color.as_ref().and_then(|color| color(series, value)).map(|role| theme.color(role))
    }
}

/// One row of a tooltip or legend.
#[allow(missing_debug_implementations)] // internal: shared with the widgets' crates, hidden
#[doc(hidden)]
pub struct KeyRow {
    #[doc(hidden)]
    pub name: String,
    color: Color,
    icon: Option<IconName>,
    value: Option<f64>,
    /// Its share of the whole, already written.
    share: Option<String>,
    value_color: Option<Color>,
    no_indicator: bool,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl KeyRow {
    #[doc(hidden)]
    pub fn named(name: String, color: Color, icon: Option<IconName>, value: Option<f64>) -> Self {
        Self { name, color, icon, value, share: None, value_color: None, no_indicator: false }
    }
}

/// A share of a whole, with no decimals to write in the reader's language.
fn percent_text(part: f64, whole: f64) -> Option<String> {
    if whole <= 0.0 || !part.is_finite() {
        return None;
    }
    let percent = part.abs() / whole * 100.0;
    Some(if percent > 0.0 && percent < 1.0 { "<1%".to_owned() } else { format!("{percent:.0}%") })
}

#[allow(missing_debug_implementations)] // internal: shared with the widgets' crates, hidden
#[doc(hidden)]
pub struct TooltipPanel {
    #[doc(hidden)]
    pub title: Option<String>,
    #[doc(hidden)]
    pub rows: Vec<KeyRow>,
    #[doc(hidden)]
    pub total: Option<(String, f64)>,
    #[doc(hidden)]
    pub tooltip: ChartTooltip,
    #[doc(hidden)]
    pub format: ValueFormat,
    #[doc(hidden)]
    pub index: usize,
    #[doc(hidden)]
    pub category: String,
}

fn indicator_node(cx: &mut BuildCx<'_>, indicator: ChartIndicator, color: Color) -> Option<NodeId> {
    let solid = |width: f32, height: f32| LayoutStyle {
        width: Some(width),
        height: Some(height),
        ..LayoutStyle::leaf()
    };
    let paint = || Paint::Quad(QuadPaint { corner_radius: 2.0, ..QuadPaint::new(color) });
    Some(match indicator {
        ChartIndicator::Dot => cx.container(solid(TOOLTIP_INDICATOR, TOOLTIP_INDICATOR), &[], paint()),
        ChartIndicator::Line => cx.container(solid(4.0, 18.0), &[], paint()),
        ChartIndicator::Dashed => {
            let dashes: Vec<NodeId> = (0..3).map(|_| cx.container(solid(2.0, 4.0), &[], paint())).collect();
            cx.container(
                LayoutStyle { spacing: 2.0, ..LayoutStyle::stack(Axis::Vertical) },
                &dashes,
                Paint::None,
            )
        }
        _ => return None,
    })
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl TooltipPanel {
    fn row(&self, cx: &mut BuildCx<'_>, row: &KeyRow, strong: bool) -> NodeId {
        let theme = *cx.theme();
        let (ink, muted) = (theme.color(Semantic::Ink), theme.color(Semantic::InkMuted));
        let mut parts = Vec::new();
        if let (true, Some(icon)) = (self.tooltip.icons, row.icon) {
            parts.push(util::icon(cx, icon, LEGEND_ICON, muted));
        } else if !strong
            && !row.no_indicator
            && let Some(mark) = indicator_node(cx, self.tooltip.indicator, row.color)
        {
            parts.push(mark);
        }
        let weight = strong.then_some(Weight::SEMIBOLD);
        parts.push(util::label(cx, &row.name, TextStyle::Caption, weight, if strong { ink } else { muted }));
        let gap = cx.container(
            LayoutStyle { grow: 1.0, min_width: Some(12.0), ..LayoutStyle::leaf() },
            &[],
            Paint::None,
        );
        parts.push(gap);
        if let Some(value) = row.value {
            let format = self.tooltip.value_format.as_ref().unwrap_or(&self.format);
            let tone = row.value_color.unwrap_or(ink);
            parts.push(util::label(cx, &format(value), TextStyle::Caption, Some(Weight::SEMIBOLD), tone));
        }
        if let Some(share) = &row.share {
            parts.push(util::label(cx, share, TextStyle::Caption, None, muted));
        }
        cx.container(
            LayoutStyle {
                spacing: 6.0,
                align: Align::Center,
                align_self: Some(Align::Stretch),
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &parts,
            Paint::None,
        )
    }
}

impl View for TooltipPanel {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let mut lines = Vec::new();
        if let Some(content) = &self.tooltip.content {
            let view = content(self.index, &self.category);
            lines.push(cx.child(&*view));
        } else {
            if let Some(title) = &self.title {
                let ink = theme.color(Semantic::Ink);
                lines.push(util::label(cx, title, TextStyle::Caption, Some(Weight::SEMIBOLD), ink));
            }
            for row in &self.rows {
                lines.push(self.row(cx, row, false));
            }
            if let Some((name, total)) = &self.total {
                let row = KeyRow::named(name.clone(), Color::TRANSPARENT, None, Some(*total));
                lines.push(self.row(cx, &row, true));
            }
        }
        let node = cx.container(
            LayoutStyle {
                padding: Insets::symmetric(6.0, 10.0),
                spacing: 6.0,
                min_width: Some(TOOLTIP_MIN_WIDTH),
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &lines,
            Paint::Quad(QuadPaint {
                corner_radius: theme.radii.md,
                border: Some(ravenrs_core::Border { width: 1.0, color: theme.color(Semantic::Border) }),
                shadow: Some(theme.elevation.raised),
                ..QuadPaint::new(theme.color(Semantic::SurfaceElevated))
            }),
        );
        // The chart says the same to a screen reader.
        cx.semantics(node, Semantics::hidden());
        // One panel that glides from place to place, and picks up from wherever it is
        // when the pointer changes its mind.
        let m = theme.motion;
        cx.motion(
            node,
            Motion::new()
                .anchored()
                .layout(m.gentle)
                .enter(Effect::scaled(motion::SURFACE_ENTER_SCALE), m.snappy)
                .exit(Effect::scaled(motion::SURFACE_EXIT_SCALE), m.fast),
        );
        node
    }
}

/// A dashed line across a [`Chart`] at a value that matters, such as a target.
///
/// ```
/// use ravenrs_style::Semantic;
/// use ravenrs_widgets::{Chart, ChartKind, ChartReference};
///
/// let chart = Chart::new(ChartKind::Bar)
///     .categories(["Jan", "Feb"])
///     .series("Sales", [12.0, 18.0])
///     .reference(ChartReference::new(15.0).label("Target").color(Semantic::Danger));
/// ```
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct ChartReference {
    value: f64,
    label: Option<String>,
    color: Option<Semantic>,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl ChartReference {
    /// A line at this value, in the color of secondary text.
    pub fn new(value: f64) -> Self {
        Self { value, label: None, color: None }
    }

    /// What it says, at its end.
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Its color.
    pub fn color(mut self, color: Semantic) -> Self {
        self.color = Some(color);
        self
    }
}

/// Where the bars of a [`Chart`] grow from.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "variants"))]
pub enum ChartBarFrom {
    /// Up from the bottom.
    #[default]
    Bottom,
    /// Down from the top, with their categories over them.
    Top,
    /// To the right from the left, as [`Chart::horizontal`] does.
    Left,
    /// To the left from the right, with their categories beside them.
    Right,
}

/// Where a [`Chart`]'s legend goes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "variants"))]
pub enum ChartLegend {
    /// Under the chart.
    #[default]
    Bottom,
    /// Over the chart.
    Top,
}

struct LegendRow {
    rows: Vec<KeyRow>,
}

impl View for LegendRow {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let (ink, muted) = (theme.color(Semantic::Ink), theme.color(Semantic::InkMuted));
        let entries: Vec<NodeId> = self
            .rows
            .iter()
            .map(|row| {
                let mark = match row.icon {
                    Some(icon) => util::icon(cx, icon, LEGEND_ICON, ink),
                    None => cx.container(
                        LayoutStyle {
                            width: Some(LEGEND_SWATCH),
                            height: Some(LEGEND_SWATCH),
                            ..LayoutStyle::leaf()
                        },
                        &[],
                        Paint::Quad(QuadPaint { corner_radius: 2.0, ..QuadPaint::new(row.color) }),
                    ),
                };
                let name = util::label(cx, &row.name, TextStyle::Caption, None, muted);
                cx.container(
                    LayoutStyle {
                        spacing: 6.0,
                        align: Align::Center,
                        ..LayoutStyle::stack(Axis::Horizontal)
                    },
                    &[mark, name],
                    Paint::None,
                )
            })
            .collect();
        cx.container(
            LayoutStyle {
                spacing: 16.0,
                wrap: true,
                justify: Justify::Center,
                align: Align::Center,
                align_self: Some(Align::Stretch),
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &entries,
            Paint::None,
        )
    }
}

/// A chart of any [`ChartKind`]. Its shapes are drawn into a picture at the screen's
/// density, and its labels are real text. Over the picture it says what each value
/// is, under the pointer or with the arrow keys. [`ChartKind::Scatter`] and the
/// sparklines stay as plain pictures, which take no focus, until given a
/// [`tooltip`](Self::tooltip).
///
/// Available with the `charts` feature, on by default.
///
/// ```
/// use ravenrs_widgets::{Chart, ChartKind};
///
/// let sales = Chart::new(ChartKind::Line)
///     .title("Sales")
///     .categories(["Jan", "Feb", "Mar"])
///     .series("2025", [12.0, 18.0, 15.0])
///     .series("2026", [14.0, 21.0, 26.0])
///     .height(240.0);
/// ```
#[derive(Clone)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct Chart {
    kind: ChartKind,
    categories: Vec<String>,
    positions: Vec<f64>,
    series: Vec<Rc<Series>>,
    title: String,
    alt: Option<String>,
    legend: bool,
    format: ValueFormat,
    width: Option<f32>,
    height: f32,
    look: Look,
    legend_at: ChartLegend,
    tooltip: Option<ChartTooltip>,
    /// The small kinds only say anything if asked to.
    tooltip_asked: bool,
    dot_icon: Option<IconName>,
    item_colors: Vec<Semantic>,
    vary_colors: bool,
    negative: Option<Semantic>,
    hover_focus: bool,
    hover_motion: bool,
    appear: bool,
    animate_changes: bool,
    appear_key: u64,
}

/// What [`PlotStyle`] takes as it is. The theme and the data fill in the rest.
#[derive(Clone)]
struct Look {
    curve: ChartCurve,
    stack: ChartStack,
    gradient: bool,
    fill_opacity: Option<f32>,
    horizontal: bool,
    reversed: bool,
    min_bar: Option<f32>,
    labels_at_zero: bool,
    bar_radius: Option<f32>,
    active: Option<usize>,
    dots: bool,
    value_labels: Option<ChartLabelAt>,
    category_labels: Option<ChartLabelAt>,
    tick_format: Option<TickFormat>,
    grid: ChartGrid,
    grid_dashed: bool,
    domain: Option<(f64, f64)>,
    references: Vec<ChartReference>,
    ends_aligned: bool,
    grid_columns: Option<usize>,
    value_ticks: Option<usize>,
    category_axis: bool,
    value_axis: bool,
    tight_scale: bool,
    no_separator: bool,
    slice_radii: Vec<f32>,
    label_guides: bool,
    hole: Option<f32>,
    concentric: bool,
    polar_grid: Option<ChartPolarGrid>,
    polar_fill: bool,
    spokes: bool,
    rings: usize,
    lines_only: bool,
    radius_axis: bool,
    start_angle: f32,
    sweep: f32,
    tracks: bool,
    center: Option<(String, String)>,
}

impl Default for Look {
    fn default() -> Self {
        Self {
            curve: ChartCurve::Natural,
            stack: ChartStack::None,
            gradient: false,
            fill_opacity: None,
            horizontal: false,
            reversed: false,
            min_bar: None,
            labels_at_zero: false,
            bar_radius: None,
            active: None,
            dots: false,
            value_labels: None,
            category_labels: None,
            tick_format: None,
            grid: ChartGrid::Horizontal,
            grid_dashed: false,
            domain: None,
            references: Vec::new(),
            ends_aligned: false,
            grid_columns: None,
            value_ticks: None,
            category_axis: true,
            value_axis: false,
            tight_scale: false,
            no_separator: false,
            slice_radii: Vec::new(),
            label_guides: false,
            hole: None,
            concentric: false,
            polar_grid: None,
            polar_fill: false,
            spokes: true,
            rings: 5,
            lines_only: false,
            radius_axis: false,
            start_angle: 0.0,
            sweep: 360.0,
            tracks: false,
            center: None,
        }
    }
}

impl fmt::Debug for Chart {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Chart")
            .field("kind", &self.kind)
            .field("title", &self.title)
            .field("categories", &self.categories.len())
            .field("series", &self.series.len())
            .finish_non_exhaustive()
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl Chart {
    /// An empty chart, 220 logical pixels tall, as wide as its parent allows.
    pub fn new(kind: ChartKind) -> Self {
        Self {
            kind,
            categories: Vec::new(),
            positions: Vec::new(),
            series: Vec::new(),
            title: String::new(),
            alt: None,
            legend: true,
            format: Rc::new(plain_number),
            width: None,
            height: 220.0,
            look: Look::default(),
            legend_at: ChartLegend::Bottom,
            tooltip: Some(ChartTooltip::new()),
            tooltip_asked: false,
            dot_icon: None,
            item_colors: Vec::new(),
            vary_colors: false,
            negative: None,
            hover_focus: true,
            hover_motion: true,
            appear: true,
            animate_changes: false,
            appear_key: 0,
        }
    }

    /// What each point is about: the labels along the bottom, or of each slice.
    pub fn categories(mut self, categories: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.categories = categories.into_iter().map(Into::into).collect();
        self
    }

    /// Where each point sits on the horizontal axis of a [`ChartKind::Scatter`].
    pub fn positions(mut self, positions: impl IntoIterator<Item = f64>) -> Self {
        self.positions = positions.into_iter().collect();
        self
    }

    /// Adds a series in the theme's next color, with one value per category. Pie
    /// and doughnut charts use the first series.
    pub fn series(mut self, name: impl Into<String>, values: impl IntoIterator<Item = f64>) -> Self {
        self.series.push(Rc::new(Series {
            name: name.into(),
            color: None,
            icon: None,
            values: values.into_iter().collect(),
        }));
        self
    }

    /// The candles of a [`ChartKind::Candlestick`], one per category, each with its open,
    /// high, low and close, in that order. Rising candles are outlines and falling ones
    /// are solid, in the theme's colors for success and danger.
    ///
    /// ```
    /// use ravenrs_widgets::{Chart, ChartKind};
    ///
    /// let prices = Chart::new(ChartKind::Candlestick)
    ///     .categories(["Mon", "Tue"])
    ///     .candles([[100.0, 112.0, 98.0, 110.0], [110.0, 111.0, 101.0, 102.0]]);
    /// ```
    pub fn candles(mut self, candles: impl IntoIterator<Item = [f64; 4]>) -> Self {
        let candles: Vec<[f64; 4]> = candles.into_iter().collect();
        // Their names come from the language of the screen, when they are shown.
        self.series = (0..4)
            .map(|at| {
                Rc::new(Series {
                    name: String::new(),
                    color: None,
                    icon: None,
                    values: candles.iter().map(|candle| candle[at]).collect(),
                })
            })
            .collect();
        self
    }

    /// Adds a series in a color of your choosing.
    pub fn series_color(
        mut self,
        name: impl Into<String>,
        color: Color,
        values: impl IntoIterator<Item = f64>,
    ) -> Self {
        self.series.push(Rc::new(Series {
            name: name.into(),
            color: Some(color),
            icon: None,
            values: values.into_iter().collect(),
        }));
        self
    }

    /// A title above the chart.
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    /// Hides the legend, which otherwise appears when there is something to tell apart.
    pub fn hide_legend(mut self) -> Self {
        self.legend = false;
        self
    }

    /// How numbers appear on the axis (at most two decimals by default).
    pub fn format(mut self, format: impl Fn(f64) -> String + 'static) -> Self {
        self.format = Rc::new(format);
        self
    }

    /// Height in logical pixels (default 220).
    pub fn height(mut self, height: f32) -> Self {
        self.height = height;
        self
    }

    /// Fixed size in logical pixels, for a sparkline in a row.
    pub fn size(mut self, width: f32, height: f32) -> Self {
        self.width = Some(width);
        self.height = height;
        self
    }

    /// Describes the chart for screen readers, in place of its title. Its values
    /// are read after it.
    pub fn alt(mut self, text: impl Into<String>) -> Self {
        self.alt = Some(text.into());
        self
    }

    /// Gives the series called `name` an icon, for the legend and the tooltip.
    pub fn series_icon(mut self, name: &str, icon: IconName) -> Self {
        for series in &mut self.series {
            if series.name == name {
                let mut changed = Series {
                    name: series.name.clone(),
                    color: series.color,
                    icon: Some(icon),
                    values: Vec::new(),
                };
                changed.values.clone_from(&series.values);
                *series = Rc::new(changed);
            }
        }
        self
    }

    /// How lines go from point to point (natural by default).
    pub fn curve(mut self, curve: ChartCurve) -> Self {
        self.look.curve = curve;
        self
    }

    /// How series share a category: side by side by default, or stacked.
    pub fn stack(mut self, stack: ChartStack) -> Self {
        self.look.stack = stack;
        self
    }

    /// Fills areas from strong at the top to faint at the bottom, and bars from strong
    /// at the tip to fainter at the base.
    pub fn gradient(mut self) -> Self {
        self.look.gradient = true;
        self
    }

    /// How solid an area or a radar polygon is, from 0 to 1.
    pub fn fill_opacity(mut self, opacity: f32) -> Self {
        self.look.fill_opacity = Some(opacity);
        self
    }

    /// Bars lie down, with their categories on the left.
    pub fn horizontal(mut self) -> Self {
        self.look.horizontal = true;
        self
    }

    /// Where bars grow from: up from the bottom by default.
    pub fn bars_from(mut self, from: ChartBarFrom) -> Self {
        self.look.horizontal = matches!(from, ChartBarFrom::Left | ChartBarFrom::Right);
        self.look.reversed = matches!(from, ChartBarFrom::Top | ChartBarFrom::Right);
        self
    }

    /// The shortest a bar with a value gets, in logical pixels, however small the value
    /// is (half a pixel by default). A value of zero stays as good as nothing.
    pub fn min_bar_length(mut self, length: f32) -> Self {
        self.look.min_bar = Some(length);
        self
    }

    /// The categories of bars sit on the line of zero, on its other side from the bar,
    /// instead of along the edge of the chart: for values both ways from zero.
    pub fn labels_at_zero(mut self) -> Self {
        self.look.labels_at_zero = true;
        self
    }

    /// The corners of bars. In a stack, only its outer corners are round.
    pub fn bar_radius(mut self, radius: f32) -> Self {
        self.look.bar_radius = Some(radius);
        self
    }

    /// Gives each category its own color from the theme's chart colors.
    pub fn vary_colors(mut self) -> Self {
        self.vary_colors = true;
        self
    }

    /// Gives the categories these colors, in order, over those of the series.
    pub fn item_colors(mut self, colors: impl IntoIterator<Item = Semantic>) -> Self {
        self.item_colors = colors.into_iter().collect();
        self
    }

    /// The color of bars below zero.
    pub fn negative_color(mut self, color: Semantic) -> Self {
        self.negative = Some(color);
        self
    }

    /// Makes one category stand out: a slice comes forward, a bar gets an outline.
    pub fn active(mut self, index: Option<usize>) -> Self {
        self.look.active = index;
        self
    }

    /// Draws a dot on each point of a line or a radar.
    pub fn dots(mut self) -> Self {
        self.look.dots = true;
        self
    }

    /// Draws this icon on each point of a line, in place of a dot.
    pub fn dot_icon(mut self, icon: IconName) -> Self {
        self.dot_icon = Some(icon);
        self
    }

    /// Writes each value beside the data.
    pub fn value_labels(mut self, at: ChartLabelAt) -> Self {
        self.look.value_labels = Some(at);
        self
    }

    /// Writes each category beside the data.
    pub fn category_labels(mut self, at: ChartLabelAt) -> Self {
        self.look.category_labels = Some(at);
        self
    }

    /// How categories are written on the axis, such as a month's first letters.
    pub fn tick_format(mut self, format: impl Fn(&str) -> String + 'static) -> Self {
        self.look.tick_format = Some(Rc::new(format));
        self
    }

    /// Which gridlines it has (level lines by default).
    pub fn grid(mut self, grid: ChartGrid) -> Self {
        self.look.grid = grid;
        self
    }

    /// Draws the gridlines in dashes.
    pub fn grid_dashed(mut self) -> Self {
        self.look.grid_dashed = true;
        self
    }

    /// Upright gridlines evenly spread into this many columns, instead of one at each
    /// category or number.
    pub fn grid_columns(mut self, columns: usize) -> Self {
        self.look.grid_columns = Some(columns);
        self
    }

    /// About how many steps the value axis has (4 by default). Round numbers decide
    /// how many exactly.
    pub fn value_ticks(mut self, ticks: usize) -> Self {
        self.look.value_ticks = Some(ticks);
        self
    }

    /// The first category along the axis starts at its point and the last one ends at
    /// its point, so that they line up with the ends of a line or an area. The last one
    /// always shows, however many others there is no room for.
    pub fn align_axis_ends(mut self) -> Self {
        self.look.ends_aligned = true;
        self
    }

    /// The value axis goes exactly from `min` to `max`, and what is past them is cut
    /// off. Its numbers are the round ones in between. Not for expanded stacks.
    pub fn value_domain(mut self, min: f64, max: f64) -> Self {
        self.look.domain = Some((min, max));
        self
    }

    /// A dashed line across the data at this value. The scale stretches to show it,
    /// unless [`Self::value_domain`] fixed it.
    pub fn reference_line(self, value: f64) -> Self {
        self.reference(ChartReference::new(value))
    }

    /// A dashed line across the data with its own text or color.
    pub fn reference(mut self, reference: ChartReference) -> Self {
        self.look.references.push(reference);
        self
    }

    /// Leaves out the categories along the axis.
    pub fn hide_category_axis(mut self) -> Self {
        self.look.category_axis = false;
        self
    }

    /// Writes numbers along the axis, which the tooltip otherwise makes unneeded.
    pub fn show_value_axis(mut self) -> Self {
        self.look.value_axis = true;
        self
    }

    /// The scale fits the values, instead of starting from zero. Not for bars.
    pub fn tight_scale(mut self) -> Self {
        self.look.tight_scale = true;
        self
    }

    /// Leaves out the gap between slices, through which what is behind the chart shows.
    pub fn no_separator(mut self) -> Self {
        self.look.no_separator = true;
        self
    }

    /// How far each slice reaches, in order, as a share from 0 to 1 of the room from
    /// the hole to the edge. A slice with no number here reaches the edge.
    pub fn slice_radii(mut self, shares: impl IntoIterator<Item = f32>) -> Self {
        self.look.slice_radii = shares.into_iter().collect();
        self
    }

    /// Labels asked for outside a pie or a doughnut sit in a column on each side, none
    /// over another, each joined to its slice by a line with an elbow. The chart gets
    /// smaller to give them the room they need.
    pub fn label_guides(mut self) -> Self {
        self.look.label_guides = true;
        self
    }

    /// The hole of a doughnut or of rings, as a share of the radius.
    pub fn hole(mut self, share: f32) -> Self {
        self.look.hole = Some(share);
        self
    }

    /// One ring per series in a pie, instead of only the first series.
    pub fn concentric(mut self) -> Self {
        self.look.concentric = true;
        self
    }

    /// The shape of the grid of a radar or radial chart.
    pub fn polar_grid(mut self, grid: ChartPolarGrid) -> Self {
        self.look.polar_grid = Some(grid);
        self
    }

    /// Fills the grid of a radar chart with the first color.
    pub fn polar_fill(mut self) -> Self {
        self.look.polar_fill = true;
        self
    }

    /// Leaves out the lines from the center of a radar chart.
    pub fn no_spokes(mut self) -> Self {
        self.look.spokes = false;
        self
    }

    /// How many rings the grid of a radar chart has (5 by default).
    pub fn rings(mut self, rings: usize) -> Self {
        self.look.rings = rings;
        self
    }

    /// Radar polygons with no fill.
    pub fn lines_only(mut self) -> Self {
        self.look.lines_only = true;
        self
    }

    /// Writes numbers along the radius of a radar chart.
    pub fn radius_axis(mut self) -> Self {
        self.look.radius_axis = true;
        self
    }

    /// Where the rings of a radial chart start, in degrees clockwise from the top,
    /// and how many degrees the largest covers.
    pub fn angles(mut self, start: f32, sweep: f32) -> Self {
        self.look.start_angle = start;
        self.look.sweep = sweep;
        self
    }

    /// Draws a track behind each ring of a radial chart.
    pub fn tracks(mut self) -> Self {
        self.look.tracks = true;
        self
    }

    /// A big number and a caption in the middle of a doughnut or of rings.
    pub fn center_text(mut self, value: impl Into<String>, caption: impl Into<String>) -> Self {
        self.look.center = Some((value.into(), caption.into()));
        self
    }

    /// What it says about the category under the pointer. A plain tooltip by default,
    /// except on a [`ChartKind::Scatter`] and a sparkline: with this they also get the
    /// cursor, the dots in focus, and the arrow keys.
    pub fn tooltip(mut self, tooltip: ChartTooltip) -> Self {
        self.tooltip = Some(tooltip);
        self.tooltip_asked = true;
        self
    }

    /// Says nothing under the pointer and takes no keyboard focus.
    pub fn hide_tooltip(mut self) -> Self {
        self.tooltip = None;
        self
    }

    /// Keeps the chart as it is around the category under the pointer: the other
    /// bars and slices do not fade, and no slice comes forward. The tooltip and the
    /// cursor stay.
    pub fn no_hover_focus(mut self) -> Self {
        self.hover_focus = false;
        self
    }

    /// The cursor, its dots and the focus go straight from one category to the next,
    /// with no glide and no fade, as they do when the system asks for less motion.
    pub fn no_hover_motion(mut self) -> Self {
        self.hover_motion = false;
        self
    }

    /// The data is whole from the first frame, instead of drawing in the first time
    /// the chart is painted.
    pub fn no_appear(mut self) -> Self {
        self.appear = false;
        self
    }

    /// When the data changes to as many series and categories as before, the chart goes
    /// from its old shape to the new one: lines and bars rise or fall to their new values,
    /// slices turn and grow, and the value axis slides along with them. A change in the
    /// middle starts from where the chart is. Under reduced motion it changes at once.
    /// For a filter over the same categories; data that scrolls along, as a live feed
    /// does, reads better without it.
    pub fn animate_changes(mut self) -> Self {
        self.animate_changes = true;
        self
    }

    /// Draws the data in again whenever `key` changes, such as for a new period or
    /// another tab. With none, it only draws in the first time the chart is painted.
    pub fn appear_key(mut self, key: impl std::hash::Hash) -> Self {
        self.appear_key = appear_key_of(key);
        self
    }

    /// How its data draws in, or `None` when it only fades in.
    fn draw_in(&self, separator: f32) -> Option<DrawIn> {
        match self.kind {
            ChartKind::Bar => Some(DrawIn::Grow { horizontal: self.look.horizontal }),
            ChartKind::Pie | ChartKind::Doughnut => Some(DrawIn::Sweep { separator }),
            ChartKind::Radar => Some(DrawIn::Spread),
            ChartKind::RadialBar => None,
            _ => Some(DrawIn::Reveal),
        }
    }

    /// Where the legend goes (under the chart by default).
    pub fn legend_at(mut self, at: ChartLegend) -> Self {
        self.legend_at = at;
        self
    }

    /// Every series' values, as a screen reader hears them.
    fn spoken_values(&self, labels: &crate::Labels) -> String {
        if self.kind == ChartKind::Candlestick {
            return self.spoken_candles(labels);
        }
        let named = self.series.len() > 1;
        // A ring draws no slice for nothing, so there is nothing to hear for it either.
        let round = matches!(self.kind, ChartKind::Pie | ChartKind::Doughnut);
        let parts: Vec<String> = self
            .series
            .iter()
            .map(|series| {
                let mut spoken: Vec<String> = series
                    .values
                    .iter()
                    .enumerate()
                    .filter(|(_, value)| !round || **value > 0.0)
                    .take(SPOKEN_VALUES)
                    .map(|(i, value)| {
                        let number = (self.format)(*value);
                        match self.categories.get(i) {
                            Some(category) => format!("{category}: {number}"),
                            None => number,
                        }
                    })
                    .collect();
                if series.values.len() > SPOKEN_VALUES {
                    spoken.push("…".to_owned());
                }
                let values = spoken.join(", ");
                if named { format!("{}: {values}", series.name) } else { values }
            })
            .collect();
        parts.join("; ")
    }

    fn candle_names(labels: &crate::Labels) -> [&str; 4] {
        [&labels.candle_open, &labels.candle_high, &labels.candle_low, &labels.candle_close]
    }

    /// Each candle, as a screen reader hears it.
    fn spoken_candles(&self, labels: &crate::Labels) -> String {
        let names = Self::candle_names(labels);
        let mut spoken: Vec<String> = self
            .categories
            .iter()
            .take(SPOKEN_VALUES)
            .enumerate()
            .map(|(i, category)| {
                let values = self.series.iter().zip(names).filter_map(|(series, name)| {
                    series.values.get(i).map(|value| format!("{name} {}", (self.format)(*value)))
                });
                format!("{category}: {}", values.collect::<Vec<_>>().join(", "))
            })
            .collect();
        if self.categories.len() > SPOKEN_VALUES {
            spoken.push("…".to_owned());
        }
        spoken.join("; ")
    }

    /// Whether this chart is one of the plain pictures.
    fn is_plain(&self) -> bool {
        matches!(self.kind, ChartKind::Scatter | ChartKind::Sparkline | ChartKind::SparkArea)
    }

    /// Whether its categories are what tells colors apart, and not its series.
    fn colors_by_category(&self) -> bool {
        matches!(self.kind, ChartKind::Pie | ChartKind::Doughnut)
            || (self.kind == ChartKind::RadialBar && self.look.stack == ChartStack::None)
    }

    fn color_of_series(&self, index: usize, colors: &[Color]) -> Color {
        self.series[index].color.unwrap_or(colors[index % colors.len()])
    }

    fn category_color(&self, index: usize, colors: &[Color], theme: &Theme) -> Color {
        self.item_colors.get(index).map_or(colors[index % colors.len()], |role| theme.color(*role))
    }

    fn plot_style(&self, theme: &Theme, colors: &[Color]) -> PlotStyle {
        let look = &self.look;
        let mut style = PlotStyle::new(self.kind);
        style.colors = colors.to_vec();
        if self.vary_colors {
            style.item_colors = colors.to_vec();
        }
        if !self.item_colors.is_empty() {
            style.item_colors = self.item_colors.iter().map(|role| theme.color(*role)).collect();
        }
        style.ink = theme.color(Semantic::Ink);
        style.muted = theme.color(Semantic::InkMuted);
        style.grid = theme.color(Semantic::ChartGrid);
        style.surface = theme.color(Semantic::SurfaceElevated);
        style.track = theme.color(Semantic::Control);
        style.size = theme.text(TextStyle::Caption).size;
        style.curve = look.curve;
        style.stack = look.stack;
        style.gradient = look.gradient;
        if let Some(opacity) = look.fill_opacity {
            style.fill_opacity = opacity;
        }
        style.horizontal = look.horizontal;
        style.reversed = look.reversed;
        if let Some(length) = look.min_bar {
            style.min_bar = length.max(0.0);
        }
        style.labels_at_zero = look.labels_at_zero;
        let bars = if self.series.len() > 1 { theme.radii.sm } else { theme.radii.md };
        style.bar_radius = look.bar_radius.unwrap_or(bars);
        style.negative = self.negative.map(|role| theme.color(role));
        style.rising = theme.color(Semantic::Success);
        style.falling = theme.color(Semantic::Danger);
        style.active = look.active;
        style.dots = look.dots && self.dot_icon.is_none();
        style.value_labels = look.value_labels;
        style.category_labels = look.category_labels;
        let format = self.format.clone();
        style.format = Rc::new(move |value| format(value));
        if let Some(format) = &look.tick_format {
            let format = format.clone();
            style.tick_format = Rc::new(move |text| format(text));
        }
        style.grid_lines = look.grid;
        style.grid_dashed = look.grid_dashed;
        style.ends_aligned = look.ends_aligned;
        style.domain = look.domain;
        style.references = look
            .references
            .iter()
            .map(|r| plot::Reference::new(r.value, r.label.clone(), r.color.map(|role| theme.color(role))))
            .collect();
        style.grid_columns = look.grid_columns;
        if let Some(ticks) = look.value_ticks {
            style.value_ticks = ticks;
        }
        style.category_axis = look.category_axis;
        style.value_axis = look.value_axis || self.kind == ChartKind::Candlestick;
        style.tight_scale = look.tight_scale;
        if look.no_separator {
            style.separator = 0.0;
        }
        style.slice_radii.clone_from(&look.slice_radii);
        style.label_guides = look.label_guides;
        if let Some(hole) = look.hole {
            style.hole = hole;
        }
        style.concentric = look.concentric;
        if let Some(grid) = look.polar_grid {
            style.polar_grid = grid;
        }
        style.polar_fill = look.polar_fill;
        style.spokes = look.spokes;
        style.rings = look.rings;
        style.lines_only = look.lines_only;
        style.radius_axis = look.radius_axis;
        style.start_angle = look.start_angle;
        style.sweep = look.sweep;
        style.tracks = look.tracks;
        style.center.clone_from(&look.center);
        style.rules_apart = true;
        style.bars_apart = true;
        style
    }

    fn legend_rows(&self, colors: &[Color], theme: &Theme) -> Vec<KeyRow> {
        // Up and down are no series to tell apart.
        if self.kind == ChartKind::Candlestick {
            return Vec::new();
        }
        if self.colors_by_category() {
            let first = self.series.first().map(|s| s.values.as_slice());
            return self
                .categories
                .iter()
                .enumerate()
                .filter(|(i, _)| first.is_none_or(|values| values.get(*i).is_some_and(|v| *v > 0.0)))
                .map(|(i, name)| {
                    KeyRow::named(name.clone(), self.category_color(i, colors, theme), None, None)
                })
                .collect();
        }
        if self.series.len() < 2 && self.series.iter().all(|s| s.icon.is_none()) {
            return Vec::new();
        }
        self.series
            .iter()
            .enumerate()
            .map(|(i, series)| {
                KeyRow::named(series.name.clone(), self.color_of_series(i, colors), series.icon, None)
            })
            .collect()
    }

    /// What a tooltip says about a category.
    fn tooltip_panel(
        &self,
        tooltip: &ChartTooltip,
        index: usize,
        colors: &[Color],
        theme: &Theme,
        labels: &crate::Labels,
    ) -> TooltipPanel {
        // A dot may have only its position to go by.
        let named = self.categories.get(index).filter(|name| !name.is_empty()).cloned();
        let category = named.unwrap_or_else(|| {
            self.positions.get(index).map(|position| (self.format)(*position)).unwrap_or_default()
        });
        let rows: Vec<KeyRow> = if self.colors_by_category() && !self.look.concentric {
            let values = self.series.first().map_or(&[][..], |s| s.values.as_slice());
            let value = values.get(index).copied();
            let mut row =
                KeyRow::named(category.clone(), self.category_color(index, colors, theme), None, value);
            if tooltip.percent {
                let whole: f64 = values.iter().filter(|v| **v > 0.0).sum();
                row.share = value.and_then(|value| percent_text(value, whole));
            }
            row.value_color = value.and_then(|value| tooltip.color_of_value(0, value, theme));
            vec![row]
        } else {
            let whole: f64 = self
                .series
                .iter()
                .filter_map(|s| s.values.get(index))
                .filter(|v| v.is_finite())
                .map(|v| v.abs())
                .sum();
            self.series
                .iter()
                .enumerate()
                .filter_map(|(i, series)| {
                    let value = series.values.get(index).copied().filter(|v| v.is_finite())?;
                    let color = if self.look.concentric || self.vary_colors || !self.item_colors.is_empty() {
                        self.category_color(index, colors, theme)
                    } else {
                        self.color_of_series(i, colors)
                    };
                    let mut row = KeyRow::named(series.name.clone(), color, series.icon, Some(value));
                    if tooltip.percent {
                        row.share = percent_text(value, whole);
                    }
                    row.value_color = tooltip.color_of_value(i, value, theme);
                    row.no_indicator = tooltip.no_indicator.contains(&series.name);
                    Some(row)
                })
                .collect()
        };
        let mut rows = rows;
        if self.kind == ChartKind::Candlestick {
            let at = |s: usize| self.series.get(s).and_then(|series| series.values.get(index)).copied();
            let rising = at(3) >= at(0);
            let color = theme.color(if rising { Semantic::Success } else { Semantic::Danger });
            for (row, name) in rows.iter_mut().zip(Self::candle_names(labels)) {
                row.name = name.to_owned();
                row.color = color;
            }
        }
        let by_category = self.colors_by_category() && !self.look.concentric;
        let name = category.clone();
        let title = if tooltip.hide_label || by_category || (category.is_empty() && tooltip.label.is_none()) {
            None
        } else if let Some(label) = &tooltip.label {
            Some(label.clone())
        } else {
            Some(match &tooltip.label_format {
                Some(format) => format(&category),
                None => category,
            })
        };
        let total = tooltip
            .total
            .as_ref()
            .map(|name| (name.clone(), rows.iter().filter_map(|row| row.value).sum::<f64>()));
        TooltipPanel {
            title,
            rows,
            total,
            tooltip: tooltip.clone(),
            format: self.format.clone(),
            index,
            category: name,
        }
    }

    /// With [`Self::animate_changes`], puts the data, and the value axis of a chart that
    /// has one, where they are on the way to the new values. Asked for on every build, so
    /// the states after it keep their places.
    fn ease_changes(&self, cx: &mut BuildCx<'_>, data: &mut ChartData, style: &mut PlotStyle) {
        let mut target: Vec<f64> = Vec::new();
        let ranged =
            matches!(self.kind, ChartKind::Line | ChartKind::Area | ChartKind::Bar) && style.domain.is_none();
        let range = if self.animate_changes && ranged { plot::value_range(data, style) } else { None };
        if self.animate_changes {
            target.extend(data.series.iter().flat_map(|s| s.values.iter().copied()));
            target.extend(range.into_iter().flat_map(|(low, high)| [low, high]));
        }
        let now = cx.animated_numbers(&target, CHANGE);
        if now.iter().map(|v| v.to_bits()).eq(target.iter().map(|v| v.to_bits())) {
            return;
        }
        let mut values = now.iter().copied();
        for series in &mut data.series {
            for value in &mut series.values {
                *value = values.next().unwrap_or(*value);
            }
        }
        if let (Some(low), Some(high)) = (values.next(), values.next()) {
            style.domain = Some((low, high));
        }
    }

    fn build_plot(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let labels = cx.labels();
        let colors = palette(&theme);
        let mut style = self.plot_style(&theme, &colors);
        let series = self.series.iter().enumerate().map(|(i, series)| SeriesData {
            name: series.name.clone(),
            color: self.color_of_series(i, &colors),
            values: series.values.clone(),
        });
        let mut data = ChartData {
            categories: self.categories.clone(),
            positions: Vec::new(),
            series: series.collect(),
        };
        self.ease_changes(cx, &mut data, &mut style);
        let plain = self.is_plain();
        let picture = cx.use_state(|| Rc::new(RefCell::new(Picture::default()))).peek().clone();
        let active: State<Option<usize>> = cx.use_state(|| None::<usize>);
        let hovered = cx.is_hover_within();
        let focused = cx.is_focused();
        let draw_in = self.draw_in(style.separator).filter(|_| self.appear);
        let m = theme.motion;
        // Asked for on every build, so the states after it keep their places.
        let progress =
            cx.appear(self.appear_key, if draw_in.is_some() { m.draw_in } else { Transition::Instant });
        let appear = draw_in.map(|how| Appear { how, progress });
        // The tooltip waits until the data is whole.
        let whole = appear.is_none_or(|appear| appear.progress >= 1.0);
        let shown = active.get().filter(|_| (hovered || focused) && self.tooltip.is_some() && whole);
        let pointer: State<Option<Point>> = cx.use_state(|| None::<Point>);

        // The anchor of the tooltip is where the last picture put this category.
        let mut children = Vec::new();
        let spot = shown.and_then(|index| {
            let picture = picture.borrow();
            let place = picture.drawing.anchor_of(index)?;
            // It follows the pointer without covering what is in focus: along a column
            // or a row it stays beside the strip, and around a circle it goes outward.
            // Each way it takes the side with the most room. With the keyboard it stays
            // on the category.
            let follows = pointer.get().filter(|_| hovered);
            let size = picture.size;
            let spot = picture.drawing.spots.iter().find(|s| s.index == index);
            let round = spot.is_none();
            let along = follows.unwrap_or(place);
            let (past_x, past_y) = (along.x > size.width / 2.0, along.y > size.height / 2.0);
            let is_bar = matches!(self.kind, ChartKind::Bar | ChartKind::Candlestick);
            let at = match (follows, spot) {
                (Some(p), Some(spot)) if self.look.horizontal && is_bar => {
                    Point::new(p.x, if past_y { spot.band.y() } else { spot.band.max_y() })
                }
                (Some(p), Some(spot)) if is_bar => {
                    Point::new(if past_x { spot.band.x() } else { spot.band.max_x() }, p.y)
                }
                (Some(p), Some(_)) => Point::new(place.x, p.y),
                (Some(p), None) => p,
                (None, _) => place,
            };
            let to_the_left = if round { !past_x } else { past_x };
            let rises = if round { !past_y } else { past_y };
            let placement = match (follows.is_some(), to_the_left, rises) {
                (false, false, _) => Placement::RightCenter,
                (false, true, _) => Placement::LeftCenter,
                (true, false, false) => Placement::Right,
                (true, false, true) => Placement::RightEnd,
                (true, true, false) => Placement::LeftStart,
                (true, true, true) => Placement::LeftEnd,
            };
            Some((at, placement))
        });
        let anchor = spot.map(|(at, _)| {
            cx.container(
                LayoutStyle {
                    absolute: Some((at.x, at.y)),
                    width: Some(0.0),
                    height: Some(0.0),
                    ..LayoutStyle::leaf()
                },
                &[],
                Paint::None,
            )
        });
        children.extend(anchor);

        let cursor = self.tooltip.as_ref().is_some_and(|t| !t.no_cursor);
        let is_bar = matches!(self.kind, ChartKind::Bar | ChartKind::Candlestick);
        let is_line = plain || matches!(self.kind, ChartKind::Line | ChartKind::Area);
        let is_round = matches!(self.kind, ChartKind::Pie | ChartKind::Doughnut);
        let moving =
            |transition: Transition| if self.hover_motion { transition } else { Transition::Instant };

        // What stands out outlasts the pointer for as long as it takes to fade.
        let held = cx.use_state(|| Rc::new(Cell::new(None::<usize>))).peek().clone();
        if shown.is_some() {
            held.set(shown);
        }
        let focus = cx.animated(if shown.is_some() { 1.0_f32 } else { 0.0 }, moving(m.base));
        // Coming from nothing, it starts in place: it only glides between categories.
        let target = held.get().unwrap_or(0) as f32;
        let at = cx.animated(target, if focus < 0.02 { Transition::Instant } else { moving(m.snappy) });
        // Kept last among the states of this view, as their number follows the data.
        let wants_lift = is_round && self.hover_focus && self.tooltip.is_some();
        let lifts: Vec<f32> = if wants_lift {
            (0..self.categories.len())
                .map(|i| cx.animated(if shown == Some(i) { 1.0_f32 } else { 0.0 }, moving(m.snappy)))
                .collect()
        } else {
            Vec::new()
        };
        let lifting = lifts.iter().any(|lift| *lift > 0.001);
        let highlight = (focus > 0.001 || lifting).then(|| Highlight {
            at,
            focus,
            dim: if self.hover_focus && (is_bar || is_round) { FOCUS_DIM } else { 0.0 },
            lifts,
            separator: style.separator,
            strip: is_bar && cursor,
            line: is_line && cursor,
            dots: is_line || self.kind == ChartKind::Radar,
            strip_color: theme.color(Semantic::Control).with_alpha(0.6),
            line_color: theme.color(Semantic::InkMuted).with_alpha(CURSOR_ALPHA),
            ring_color: theme.color(Semantic::SurfaceElevated),
        });
        let drawing: DrawingFn = if plain {
            self.small_drawing(&theme, &colors, true)
        } else {
            Rc::new(move |rect, _, measure| plot::draw_measured(&data, rect, &style, measure))
        };
        let node_style = LayoutStyle { grow: 1.0, align_self: Some(Align::Stretch), ..LayoutStyle::leaf() };
        let node = plot_node(
            cx,
            node_style,
            picture.clone(),
            drawing,
            self.dot_icon,
            (highlight, appear),
            &children,
        );

        if self.tooltip.is_some() {
            let (write, seen, follow) = (active.clone(), picture.clone(), pointer.clone());
            cx.on_pointer_move(
                node,
                Rc::new(move |position, frame| {
                    let local = Point::new(position.x - frame.x(), position.y - frame.y());
                    follow.set(Some(local));
                    let picture = seen.borrow();
                    let hit = picture.drawing.hit(local).map(|hit| hit.index).or_else(|| {
                        // A slice that came forward answers for what it covers now.
                        let lifted = write.get().filter(|_| wants_lift)?;
                        picture.drawing.over_grown_slice(local, lifted, SLICE_LIFT).then_some(lifted)
                    });
                    if write.get() != hit {
                        write.set(hit);
                    }
                }),
            );
            cx.cursor(node, CursorIcon::Default);
            let (write, seen) = (active.clone(), picture);
            cx.on_key(
                node,
                Rc::new(move |key, _| {
                    let count = seen.borrow().drawing.categories();
                    if count == 0 {
                        return false;
                    }
                    let last = count - 1;
                    let now = write.get();
                    let next = match key {
                        Key::ArrowRight | Key::ArrowDown => now.map_or(0, |i| (i + 1).min(last)),
                        Key::ArrowLeft | Key::ArrowUp => now.map_or(last, |i| i.saturating_sub(1)),
                        Key::Home => 0,
                        Key::End => last,
                        Key::Escape if now.is_some() => {
                            write.set(None);
                            return true;
                        }
                        _ => return false,
                    };
                    write.set(Some(next));
                    true
                }),
            );
            cx.focusable(node, theme.radii.sm);
        }

        if let (Some(tooltip), Some(index), Some((_, placement)), Some(anchor)) =
            (&self.tooltip, shown, spot, anchor)
        {
            let panel = self.tooltip_panel(tooltip, index, &colors, &theme, &labels);
            cx.overlay(anchor, placement, &panel, None);
        }

        let name = self.alt.as_deref().unwrap_or(&self.title);
        let values = self.spoken_values(&labels);
        let mut semantics = if name.is_empty() {
            Semantics::new(Role::Image).name(values)
        } else {
            Semantics::new(Role::Image).name(name.to_owned()).description(values)
        };
        if let (Some(tooltip), Some(index)) = (&self.tooltip, shown) {
            let panel = self.tooltip_panel(tooltip, index, &colors, &theme, &labels);
            semantics = semantics.value(panel.spoken());
        }
        cx.semantics(node, semantics);
        // What does not draw in fades in.
        if appear.is_none() {
            cx.motion(node, Motion::new().enter(Effect::hidden(), m.base));
        }

        let mut parts = Vec::new();
        // The small kinds have their title and legend in their picture.
        if !self.title.is_empty() && !plain {
            let ink = theme.color(Semantic::Ink);
            parts.push(util::label(cx, &self.title, TextStyle::Callout, Some(Weight::SEMIBOLD), ink));
        }
        let rows = if self.legend && !plain { self.legend_rows(&colors, &theme) } else { Vec::new() };
        let legend = (!rows.is_empty()).then(|| cx.child(&LegendRow { rows }));
        if self.legend_at == ChartLegend::Top {
            parts.extend(legend);
        }
        parts.push(node);
        if self.legend_at == ChartLegend::Bottom {
            parts.extend(legend);
        }
        cx.container(
            LayoutStyle {
                width: self.width,
                height: Some(self.height),
                spacing: 8.0,
                align: Align::Stretch,
                align_self: self.width.is_none().then_some(Align::Stretch),
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &parts,
            Paint::None,
        )
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose(part = "part1"))]
impl TooltipPanel {
    /// The same as the panel, for a screen reader.
    #[doc(hidden)]
    pub fn spoken(&self) -> String {
        let format = self.tooltip.value_format.as_ref().unwrap_or(&self.format);
        let mut parts: Vec<String> = self.title.iter().cloned().collect();
        parts.extend(self.rows.iter().map(|row| match row.value {
            Some(value) => match &row.share {
                Some(share) => format!("{}: {} ({share})", row.name, format(value)),
                None => format!("{}: {}", row.name, format(value)),
            },
            None => row.name.clone(),
        }));
        if let Some((name, total)) = &self.total {
            parts.push(format!("{name}: {}", format(*total)));
        }
        parts.join(", ")
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose(part = "part1"))]
impl Chart {
    /// How a scatter chart or a sparkline is drawn, the same with a tooltip or without.
    fn small_drawing(&self, theme: &Theme, colors: &[Color], placed: bool) -> DrawingFn {
        let series = self.series.iter().enumerate().map(|(i, series)| SeriesData {
            name: series.name.clone(),
            color: series.color.unwrap_or(colors[i % colors.len()]),
            values: series.values.clone(),
        });
        let data = ChartData {
            categories: self.categories.clone(),
            positions: self.positions.clone(),
            series: series.collect(),
        };
        let (kind, legend, title, format) = (self.kind, self.legend, self.title.clone(), self.format.clone());
        // The axis text is the caption size, and the rest is sized from it.
        let size = theme.text(TextStyle::Caption).size / 0.85;
        let (ink, muted) = (theme.color(Semantic::Ink), theme.color(Semantic::InkMuted));
        let (grid, separator) = (theme.color(Semantic::Border), theme.color(Semantic::SurfaceElevated));
        let (bar_radius, colors) = (theme.radii.sm, colors.to_vec());
        Rc::new(move |rect, _, _| {
            let options =
                ChartOptions { kind, title: &title, legend, size, ink, muted, grid, format: &*format };
            let look = ChartStyle { palette: &colors, separator, bar_radius, max_bar_width: MAX_BAR_WIDTH };
            if placed {
                plot::draw_small(&data, rect, &options, &look)
            } else {
                chart::draw_styled(&data, rect, &options, &look).into()
            }
        })
    }
}

impl View for Chart {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        if !self.is_plain() || (self.tooltip_asked && self.tooltip.is_some()) {
            return self.build_plot(cx);
        }
        let theme = *cx.theme();
        let colors = palette(&theme);
        let drawing = self.small_drawing(&theme, &colors, false);
        let style = LayoutStyle {
            width: self.width,
            height: Some(self.height),
            align_self: self.width.is_none().then_some(Align::Stretch),
            ..LayoutStyle::leaf()
        };
        let node = drawing_node(cx, style, drawing);
        let values = self.spoken_values(&cx.labels());
        let name = self.alt.as_deref().unwrap_or(&self.title);
        let semantics = if name.is_empty() {
            Semantics::new(Role::Image).name(values)
        } else {
            Semantics::new(Role::Image).name(name.to_owned()).description(values)
        };
        cx.semantics(node, semantics);
        node
    }
}

/// A bar chart with labels under each bar and faint gridlines.
///
/// Available with the `charts` feature, on by default.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct BarChart {
    data: Vec<(String, f32)>,
    height: f32,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl BarChart {
    /// A chart of `(label, value)` bars; negative values are clamped to 0.
    pub fn new(data: impl IntoIterator<Item = (impl Into<String>, f32)>) -> Self {
        Self { data: data.into_iter().map(|(l, v)| (l.into(), v.max(0.0))).collect(), height: 180.0 }
    }

    /// Height of the bars in logical pixels, without their labels (default 180).
    pub fn height(mut self, height: f32) -> Self {
        self.height = height;
        self
    }
}

impl View for BarChart {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let labels_height = cx.theme().text(TextStyle::Caption).line_height + 8.0;
        let chart = Chart::new(ChartKind::Bar)
            .categories(self.data.iter().map(|(label, _)| label.as_str()))
            .series("", self.data.iter().map(|(_, value)| f64::from(*value)))
            .height(self.height + labels_height);
        cx.child(&chart)
    }
}

/// A smooth area chart (sparkline style) of evenly spaced values.
///
/// Available with the `charts` feature, on by default.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct AreaChart {
    values: Vec<f32>,
    height: f32,
    tone: Semantic,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl AreaChart {
    /// A chart of these values, evenly spaced from left to right.
    pub fn new(values: impl IntoIterator<Item = f32>) -> Self {
        Self { values: values.into_iter().collect(), height: 120.0, tone: Semantic::Accent }
    }

    /// Chart height in logical pixels (default 120).
    pub fn height(mut self, height: f32) -> Self {
        self.height = height;
        self
    }

    /// The line color role (Accent by default; Success, Danger...).
    pub fn tone(mut self, tone: Semantic) -> Self {
        self.tone = tone;
        self
    }
}

impl View for AreaChart {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let color = cx.theme().color(self.tone);
        let chart = Chart::new(ChartKind::SparkArea)
            .series_color("", color, self.values.iter().map(|value| f64::from(*value)))
            .height(self.height);
        cx.child(&chart)
    }
}

/// A chart's card: its title and description, what changes it, and what it says at
/// the bottom, such as how it compares to last month.
///
/// ```
/// use ravenrs_widgets::{Chart, ChartCard, ChartKind, IconName};
///
/// let card = ChartCard::new(Chart::new(ChartKind::Bar))
///     .title("Sales")
///     .description("January to June")
///     .trend("Up by 5.2% this month", IconName::TrendingUp)
///     .note("Showing all channels");
/// ```
pub struct ChartCard<V> {
    content: V,
    title: String,
    description: String,
    action: Option<Rc<dyn View>>,
    tabs: Option<ChartTabs>,
    trend: Option<(String, IconName)>,
    note: Option<String>,
    centered: bool,
}

impl<V> fmt::Debug for ChartCard<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ChartCard").field("title", &self.title).finish_non_exhaustive()
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl<V: View> ChartCard<V> {
    /// A card around `content`.
    pub fn new(content: V) -> Self {
        Self {
            content,
            title: String::new(),
            description: String::new(),
            action: None,
            tabs: None,
            trend: None,
            note: None,
            centered: false,
        }
    }

    /// The title.
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    /// What the chart shows, under the title.
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }

    /// Something beside the title that changes the chart, such as a period.
    pub fn action(mut self, action: impl View + 'static) -> Self {
        self.action = Some(Rc::new(action));
        self
    }

    /// Choices beside the title, each with a big number: what to show, and how much it is.
    pub fn tabs(
        mut self,
        selected: Binding<usize>,
        tabs: impl IntoIterator<Item = (impl Into<String>, impl Into<String>)>,
    ) -> Self {
        let tabs = tabs.into_iter().map(|(name, total)| (name.into(), total.into())).collect();
        self.tabs = Some((selected, tabs));
        self
    }

    /// A line at the bottom that says what the chart means, with an icon.
    pub fn trend(mut self, text: impl Into<String>, icon: IconName) -> Self {
        self.trend = Some((text.into(), icon));
        self
    }

    /// A fainter line at the bottom.
    pub fn note(mut self, text: impl Into<String>) -> Self {
        self.note = Some(text.into());
        self
    }

    /// Centers the title and the bottom lines, as round charts do.
    pub fn centered(mut self) -> Self {
        self.centered = true;
        self
    }
}

struct ChartTab {
    selected: Binding<usize>,
    index: usize,
    name: String,
    total: String,
}

impl View for ChartTab {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let (ink, muted) = (theme.color(Semantic::Ink), theme.color(Semantic::InkMuted));
        let on = self.selected.get() == self.index;
        let name = util::label(cx, &self.name, TextStyle::Caption, None, muted);
        let total = util::label(cx, &self.total, TextStyle::Title, Some(Weight::SEMIBOLD), ink);
        let fill = if on || cx.is_hovered() { theme.color(Semantic::Control) } else { Color::TRANSPARENT };
        let node = cx.container(
            LayoutStyle {
                padding: Insets::symmetric(10.0, 16.0),
                spacing: 2.0,
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &[name, total],
            Paint::Quad(QuadPaint { corner_radius: theme.radii.md, ..QuadPaint::new(fill) }),
        );
        let (selected, index) = (self.selected.clone(), self.index);
        cx.on_click(node, Rc::new(move || selected.set(index)));
        cx.cursor(node, CursorIcon::Pointer);
        cx.focusable(node, theme.radii.md);
        cx.semantics(
            node,
            Semantics::new(Role::RadioButton)
                .name(format!("{}: {}", self.name, self.total))
                .selected(on)
                .toggled(on),
        );
        node
    }
}

impl<V: View> View for ChartCard<V> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        cx.child(&Card::new(CardBody(self)))
    }
}

struct CardBody<'a, V>(&'a ChartCard<V>);

impl<V: View> View for CardBody<'_, V> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let card = self.0;
        let theme = *cx.theme();
        let (ink, muted) = (theme.color(Semantic::Ink), theme.color(Semantic::InkMuted));
        let along = if card.centered { Align::Center } else { Align::Start };
        let mut heading = Vec::new();
        if !card.title.is_empty() {
            heading.push(util::label(cx, &card.title, TextStyle::Headline, Some(Weight::SEMIBOLD), ink));
        }
        if !card.description.is_empty() {
            heading.push(util::label(cx, &card.description, TextStyle::Callout, None, muted));
        }
        let heading = cx.container(
            LayoutStyle {
                spacing: 2.0,
                grow: if card.centered { 0.0 } else { 1.0 },
                min_width: Some(0.0),
                align: along,
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &heading,
            Paint::None,
        );
        let mut head = vec![heading];
        if let Some(action) = &card.action {
            head.push(cx.child(&**action));
        }
        if let Some((selected, tabs)) = &card.tabs {
            for (index, (name, total)) in tabs.iter().enumerate() {
                head.push(cx.child(&ChartTab {
                    selected: selected.clone(),
                    index,
                    name: name.clone(),
                    total: total.clone(),
                }));
            }
        }
        let head = cx.container(
            LayoutStyle {
                spacing: 12.0,
                align: Align::Center,
                justify: if card.centered { Justify::Center } else { Justify::Start },
                wrap: true,
                align_self: Some(Align::Stretch),
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &head,
            Paint::None,
        );
        let content = cx.child(&card.content);
        let mut lines = vec![head, content];
        let mut foot = Vec::new();
        if let Some((text, icon)) = &card.trend {
            let label = util::label(cx, text, TextStyle::Callout, Some(Weight::MEDIUM), ink);
            let icon = util::icon(cx, *icon, 16.0, ink);
            foot.push(cx.container(
                LayoutStyle { spacing: 6.0, align: Align::Center, ..LayoutStyle::stack(Axis::Horizontal) },
                &[label, icon],
                Paint::None,
            ));
        }
        if let Some(note) = &card.note {
            foot.push(util::label(cx, note, TextStyle::Callout, None, muted));
        }
        if !foot.is_empty() {
            lines.push(cx.container(
                LayoutStyle {
                    spacing: 4.0,
                    align: along,
                    align_self: Some(Align::Stretch),
                    ..LayoutStyle::stack(Axis::Vertical)
                },
                &foot,
                Paint::None,
            ));
        }
        cx.container(
            LayoutStyle {
                spacing: 16.0,
                align: Align::Stretch,
                align_self: Some(Align::Stretch),
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &lines,
            Paint::None,
        )
    }
}

/// A QR Code or a linear barcode, drawn so that every module lands on whole
/// pixels. Scanners need dark on light, so it stays that way in dark themes.
///
/// Text the format cannot hold shows why instead of a code nobody can read.
/// Available with the `charts` feature, on by default.
///
/// ```
/// use ravenrs_widgets::{Barcode, Symbology};
///
/// let link = Barcode::qr("https://example.com").alt("Link to the example site");
/// let product = Barcode::new(Symbology::Ean13, "789123456789").size(240.0, 110.0);
/// ```
#[derive(Clone, Debug)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct Barcode {
    symbology: Symbology,
    data: String,
    size: Option<(f32, f32)>,
    show_text: bool,
    alt: Option<String>,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl Barcode {
    /// A code of this format holding `data`.
    pub fn new(symbology: Symbology, data: impl Into<String>) -> Self {
        Self { symbology, data: data.into(), size: None, show_text: true, alt: None }
    }

    /// A QR Code holding `data`.
    pub fn qr(data: impl Into<String>) -> Self {
        Self::new(Symbology::Qr, data)
    }

    /// Size in logical pixels (160 by 160 for a QR Code, 240 by 110 for the others).
    pub fn size(mut self, width: f32, height: f32) -> Self {
        self.size = Some((width, height));
        self
    }

    /// Leaves out the text under the bars of a linear code.
    pub fn hide_text(mut self) -> Self {
        self.show_text = false;
        self
    }

    /// Describes the code for screen readers, in place of the text it holds.
    pub fn alt(mut self, text: impl Into<String>) -> Self {
        self.alt = Some(text.into());
        self
    }
}

/// The largest box that fits in `room` and holds whole pixels per module.
fn whole_modules(room: Rect, across: f32, square: bool, scale: f32) -> Rect {
    let side = if square { room.width().min(room.height()) } else { room.width() };
    let pixels_per_module = (side * scale / across).floor().max(1.0);
    let side = pixels_per_module * across / scale;
    let height = if square { side } else { room.height() };
    let snap = |value: f32| (value * scale).round() / scale;
    Rect::new(
        snap(room.x() + (room.width() - side) / 2.0),
        snap(room.y() + (room.height() - height) / 2.0),
        side,
        height,
    )
}

fn barcode_drawing(
    pattern: &Pattern,
    show_text: bool,
    text_size: f32,
    radius: f32,
    rect: Rect,
    scale: f32,
) -> ChartDrawing {
    // Paper first: the code's own quiet zone needs a light background.
    let paper = Shape::filled(vector::rectangle(rect, radius), Color::WHITE);
    let mut labels = Vec::new();
    let mut room = rect;
    if let (Pattern::Linear { text, .. }, true) = (pattern, show_text) {
        let height = text_size * LABEL_LINE_HEIGHT + BARCODE_PADDING;
        room = Rect::new(rect.x(), rect.y(), rect.width(), (rect.height() - height).max(0.0));
        labels.push(ChartLabel {
            text: text.clone(),
            rect: Rect::new(rect.x(), room.max_y(), rect.width(), height),
            align: LabelAlign::Center,
            size: text_size,
            bold: false,
            color: Color::BLACK,
        });
    }
    let square = matches!(pattern, Pattern::Matrix { .. });
    if !square {
        room = Rect::new(
            room.x(),
            room.y() + BARCODE_PADDING,
            room.width(),
            (room.height() - BARCODE_PADDING).max(0.0),
        );
    }
    let across = barcode::modules_across(pattern) as f32;
    let code = barcode::shape(pattern, whole_modules(room, across, square, scale), Color::BLACK);
    ChartDrawing { shapes: vec![paper, code], labels }
}

impl View for Barcode {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let pattern = match barcode::encode(self.symbology, &self.data) {
            Ok(pattern) => pattern,
            Err(problem) => {
                let color = theme.color(Semantic::Danger);
                return util::label(cx, &problem, TextStyle::Caption, None, color);
            }
        };
        let default = if matches!(pattern, Pattern::Matrix { .. }) { (160.0, 160.0) } else { (240.0, 110.0) };
        let (width, height) = self.size.unwrap_or(default);
        let style = LayoutStyle { width: Some(width), height: Some(height), ..LayoutStyle::leaf() };
        let (show_text, text_size, radius) =
            (self.show_text, theme.text(TextStyle::Caption).size, theme.radii.sm);
        let drawing: DrawingFn = Rc::new(move |rect, scale, _| {
            barcode_drawing(&pattern, show_text, text_size, radius, rect, scale).into()
        });
        let node = drawing_node(cx, style, drawing);
        let name = self.alt.clone().unwrap_or_else(|| self.data.clone());
        cx.semantics(node, Semantics::new(Role::Image).name(name));
        node
    }
}
