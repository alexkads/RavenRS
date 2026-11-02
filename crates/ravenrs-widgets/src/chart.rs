//! Charts and barcodes. The layout comes from `ravenrs-graphics`, the same as in
//! reports; here its shapes become a picture and its labels become text. Both are
//! kept between frames, so a rebuild that changes nothing uploads nothing.

use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;

use ravenrs_core::{Color, ImageData, ImageQuad, Point, Rect, Role, Size};
use ravenrs_graphics::barcode::{self, Pattern};
use ravenrs_graphics::chart::{
    self, ChartData, ChartDrawing, ChartLabel, ChartOptions, ChartStyle, LabelAlign, SeriesData,
};
use ravenrs_graphics::vector::{self, Shape};
use ravenrs_layout::{Align, LayoutStyle};
use ravenrs_style::{Semantic, TextStyle, Theme, Weight};
use ravenrs_text::{TextAttrs, TextLayout};

pub use ravenrs_graphics::barcode::Symbology;
pub use ravenrs_graphics::chart::ChartKind;

use crate::context::{BuildCx, NodeId, Paint, PaintCx, Semantics};
use crate::{View, util};

/// The widest a bar gets, so a few bars in a wide chart still read as bars.
const MAX_BAR_WIDTH: f32 = 44.0;
const LABEL_LINE_HEIGHT: f32 = 1.3;
/// How many values a screen reader hears before the rest is left out.
const SPOKEN_VALUES: usize = 12;
const BARCODE_PADDING: f32 = 8.0;

/// Lays a drawing out in a rectangle, at a number of pixels per logical pixel.
type DrawingFn = Rc<dyn Fn(Rect, f32) -> ChartDrawing>;

/// What a drawing looked like when it was last painted, relative to its frame.
#[derive(Default)]
struct Picture {
    source: Option<DrawingFn>,
    size: Size,
    scale: f32,
    drawing: ChartDrawing,
    image: Option<ImageData>,
    labels: Vec<(TextLayout, Point, Color)>,
}

impl Picture {
    /// Brings the picture up to date, redoing only what changed.
    fn refresh(&mut self, source: &DrawingFn, size: Size, cx: &mut PaintCx<'_>) {
        let same_source = self.source.as_ref().is_some_and(|known| Rc::ptr_eq(known, source));
        let same_pixels = self.size == size && self.scale == cx.scale;
        if same_source && same_pixels {
            return;
        }
        let area = Rect::new(0.0, 0.0, size.width, size.height);
        let drawing = source(area, cx.scale);
        if !same_pixels || drawing.shapes != self.drawing.shapes {
            self.image = vector::rasterize(&drawing.shapes, area, cx.scale);
        }
        if drawing.labels != self.drawing.labels {
            self.labels = drawing.labels.iter().map(|label| typeset(label, cx)).collect();
        }
        self.source = Some(source.clone());
        self.size = size;
        self.scale = cx.scale;
        self.drawing = drawing;
    }
}

fn typeset(label: &ChartLabel, cx: &mut PaintCx<'_>) -> (TextLayout, Point, Color) {
    let attrs = TextAttrs {
        size: label.size,
        line_height: label.size * LABEL_LINE_HEIGHT,
        weight: if label.bold { Weight::SEMIBOLD.0 } else { Weight::REGULAR.0 },
        letter_spacing: 0.0,
        family: None,
    };
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

/// A box that paints what `drawing` lays out for its frame.
fn drawing_node(cx: &mut BuildCx<'_>, style: LayoutStyle, drawing: DrawingFn) -> NodeId {
    let picture = cx.use_state(|| Rc::new(RefCell::new(Picture::default()))).peek().clone();
    cx.container(
        style,
        &[],
        Paint::Rich(Rc::new(move |frame: Rect, paint: &mut PaintCx<'_>| {
            if frame.is_empty() {
                return;
            }
            let mut picture = picture.borrow_mut();
            picture.refresh(&drawing, frame.size, paint);
            if let Some(image) = &picture.image {
                paint.scene.push_image(ImageQuad::new(frame, image.clone()));
            }
            for (layout, at, color) in &picture.labels {
                let origin = Point::new(frame.x() + at.x, frame.y() + at.y);
                layout.paint(origin, *color, paint.scene);
            }
        })),
    )
}

/// The colors of a chart in this theme: status colors first, then the shared palette.
fn palette(theme: &Theme) -> Vec<Color> {
    [Semantic::Accent, Semantic::Success, Semantic::Warning, Semantic::Danger]
        .map(|role| theme.color(role))
        .into_iter()
        .chain(chart::PALETTE[4..].iter().copied())
        .collect()
}

/// Shows a number on an axis: at most two decimals, none when they are zeros.
fn plain_number(value: f64) -> String {
    let text = format!("{value:.2}");
    text.trim_end_matches('0').trim_end_matches('.').to_owned()
}

struct Series {
    name: String,
    color: Option<Color>,
    values: Vec<f64>,
}

/// A chart of any [`ChartKind`], laid out exactly as reports lay it out. The
/// shapes are drawn into a picture at the screen's density, and the labels are
/// real text.
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
pub struct Chart {
    kind: ChartKind,
    categories: Vec<String>,
    positions: Vec<f64>,
    series: Vec<Rc<Series>>,
    title: String,
    alt: Option<String>,
    legend: bool,
    format: Rc<dyn Fn(f64) -> String>,
    width: Option<f32>,
    height: f32,
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
            values: values.into_iter().collect(),
        }));
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

    /// The first series' values, as a screen reader hears them.
    fn spoken_values(&self) -> String {
        let Some(series) = self.series.first() else { return String::new() };
        let mut spoken: Vec<String> = series
            .values
            .iter()
            .take(SPOKEN_VALUES)
            .enumerate()
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
        spoken.join(", ")
    }
}

impl View for Chart {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let colors = palette(&theme);
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
        let bar_radius = theme.radii.sm;
        let drawing: DrawingFn = Rc::new(move |rect, _| {
            chart::draw_styled(
                &data,
                rect,
                &ChartOptions { kind, title: &title, legend, size, ink, muted, grid, format: &*format },
                &ChartStyle { palette: &colors, separator, bar_radius, max_bar_width: MAX_BAR_WIDTH },
            )
        });
        let style = LayoutStyle {
            width: self.width,
            height: Some(self.height),
            align_self: self.width.is_none().then_some(Align::Stretch),
            ..LayoutStyle::leaf()
        };
        let node = drawing_node(cx, style, drawing);
        let values = self.spoken_values();
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
pub struct BarChart {
    data: Vec<(String, f32)>,
    height: f32,
}

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
pub struct AreaChart {
    values: Vec<f32>,
    height: f32,
    tone: Semantic,
}

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
pub struct Barcode {
    symbology: Symbology,
    data: String,
    size: Option<(f32, f32)>,
    show_text: bool,
    alt: Option<String>,
}

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
        let drawing: DrawingFn =
            Rc::new(move |rect, scale| barcode_drawing(&pattern, show_text, text_size, radius, rect, scale));
        let node = drawing_node(cx, style, drawing);
        let name = self.alt.clone().unwrap_or_else(|| self.data.clone());
        cx.semantics(node, Semantics::new(Role::Image).name(name));
        node
    }
}
