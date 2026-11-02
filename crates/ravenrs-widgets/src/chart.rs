//! Charts and barcodes. The layout comes from `ravenrs-graphics`, the same as in
//! reports; here its shapes become a picture and its labels become text. Both are
//! kept between frames, so a rebuild that changes nothing uploads nothing.

use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;

use ravenrs_core::{
    Color, CursorIcon, Effect, ImageData, ImageQuad, Insets, Key, Point, Quad, Rect, Role, Size,
};
use ravenrs_graphics::barcode::{self, Pattern};
use ravenrs_graphics::chart::{
    self, ChartData, ChartDrawing, ChartLabel, ChartOptions, ChartStyle, LabelAlign, SeriesData,
};
use ravenrs_graphics::plot::{self, Plot, PlotStyle};
use ravenrs_graphics::vector::{self, Shape};
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
use crate::{Card, IconName, View, util};

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
const DOT_ICON: f32 = 18.0;

/// Lays a drawing out in a rectangle, at a number of pixels per logical pixel.
type DrawingFn = Rc<dyn Fn(Rect, f32) -> Plot>;
type ValueFormat = Rc<dyn Fn(f64) -> String>;
type TickFormat = Rc<dyn Fn(&str) -> String>;
/// Which tab is chosen, and the name and the total of each.
type ChartTabs = (Binding<usize>, Vec<(String, String)>);

/// What a drawing looked like when it was last painted, relative to its frame.
#[derive(Default)]
struct Picture {
    source: Option<DrawingFn>,
    size: Size,
    scale: f32,
    drawing: Plot,
    image: Option<ImageData>,
    labels: Vec<(TextLayout, Point, Color)>,
    dot_icons: Vec<(TextLayout, Point, Color)>,
}

impl Picture {
    /// Brings the picture up to date, redoing only what changed.
    fn refresh(&mut self, source: &DrawingFn, dot_icon: Option<IconName>, size: Size, cx: &mut PaintCx<'_>) {
        let same_source = self.source.as_ref().is_some_and(|known| Rc::ptr_eq(known, source));
        let same_pixels = self.size == size && self.scale == cx.scale;
        if same_source && same_pixels {
            return;
        }
        let area = Rect::new(0.0, 0.0, size.width, size.height);
        let drawing = source(area, cx.scale);
        if !same_pixels || drawing.layers != self.drawing.layers {
            self.image = vector::rasterize_layers(&drawing.layers, area, cx.scale);
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

/// What stands out over the picture while a category is active.
#[derive(Clone, Copy)]
struct Highlight {
    index: usize,
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

fn paint_strip(highlight: &Highlight, plot: &Plot, frame: Rect, cx: &mut PaintCx<'_>) {
    if !highlight.strip {
        return;
    }
    let Some(spot) = plot.spots.iter().find(|s| s.index == highlight.index) else { return };
    let (band, area) = (spot.band, plot.area);
    let along_x = (band.width() - frame.width()).abs() > 0.5;
    let strip = if along_x {
        Rect::new(band.x(), area.y(), band.width(), area.height())
    } else {
        Rect::new(area.x(), band.y(), area.width(), band.height())
    };
    let strip = Rect::new(frame.x() + strip.x(), frame.y() + strip.y(), strip.width(), strip.height());
    cx.scene.push_quad(Quad::new(strip, highlight.strip_color));
}

fn paint_highlight(highlight: &Highlight, plot: &Plot, frame: Rect, cx: &mut PaintCx<'_>) {
    if highlight.line
        && let Some(spot) = plot.spots.iter().find(|s| s.index == highlight.index)
    {
        let line =
            Rect::new(frame.x() + spot.center.x - 0.5, frame.y() + plot.area.y(), 1.0, plot.area.height());
        cx.scene.push_quad(Quad::new(line, highlight.line_color));
    }
    if highlight.dots {
        for mark in plot.marks.iter().filter(|m| m.index == highlight.index) {
            let side = ACTIVE_DOT * 2.0;
            let rect =
                Rect::new(frame.x() + mark.at.x - ACTIVE_DOT, frame.y() + mark.at.y - ACTIVE_DOT, side, side);
            cx.scene
                .push_quad(Quad::new(rect, mark.color).radius(ACTIVE_DOT).border(2.0, highlight.ring_color));
        }
    }
}

/// A box that paints what `drawing` lays out for its frame, around `children`.
fn plot_node(
    cx: &mut BuildCx<'_>,
    style: LayoutStyle,
    picture: Rc<RefCell<Picture>>,
    drawing: DrawingFn,
    dot_icon: Option<IconName>,
    highlight: Option<Highlight>,
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
            if let Some(image) = &picture.image {
                paint.scene.push_image(ImageQuad::new(frame, image.clone()));
            }
            if let Some(highlight) = &highlight {
                paint_highlight(highlight, &picture.drawing, frame, paint);
            }
            for (layout, at, color) in picture.labels.iter().chain(&picture.dot_icons) {
                let origin = Point::new(frame.x() + at.x, frame.y() + at.y);
                layout.paint(origin, *color, paint.scene);
            }
        })),
    )
}

/// A box that paints what `drawing` lays out for its frame.
fn drawing_node(cx: &mut BuildCx<'_>, style: LayoutStyle, drawing: DrawingFn) -> NodeId {
    let picture = cx.use_state(|| Rc::new(RefCell::new(Picture::default()))).peek().clone();
    plot_node(cx, style, picture, drawing, None, None, &[])
}

/// The colors of a chart in this theme: none of them means anything, unlike status
/// colors, and past the theme's five come those of the shared palette.
fn palette(theme: &Theme) -> Vec<Color> {
    [Semantic::Chart1, Semantic::Chart2, Semantic::Chart3, Semantic::Chart4, Semantic::Chart5]
        .map(|role| theme.color(role))
        .into_iter()
        .chain(chart::PALETTE[5..].iter().copied())
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
    icon: Option<IconName>,
    values: Vec<f64>,
}

/// How a row of a [`ChartTooltip`] shows its color.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
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
pub struct ChartTooltip {
    indicator: ChartIndicator,
    hide_label: bool,
    label: Option<String>,
    label_format: Option<TickFormat>,
    value_format: Option<ValueFormat>,
    total: Option<String>,
    icons: bool,
    no_cursor: bool,
}

impl fmt::Debug for ChartTooltip {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ChartTooltip").field("indicator", &self.indicator).finish_non_exhaustive()
    }
}

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

    /// Leaves out the strip or line that marks the category on the chart.
    pub fn no_cursor(mut self) -> Self {
        self.no_cursor = true;
        self
    }
}

/// One row of a tooltip or legend.
struct KeyRow {
    name: String,
    color: Color,
    icon: Option<IconName>,
    value: Option<f64>,
}

struct TooltipPanel {
    title: Option<String>,
    rows: Vec<KeyRow>,
    total: Option<(String, f64)>,
    tooltip: ChartTooltip,
    format: ValueFormat,
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

impl TooltipPanel {
    fn row(&self, cx: &mut BuildCx<'_>, row: &KeyRow, strong: bool) -> NodeId {
        let theme = *cx.theme();
        let (ink, muted) = (theme.color(Semantic::Ink), theme.color(Semantic::InkMuted));
        let mut parts = Vec::new();
        if let (true, Some(icon)) = (self.tooltip.icons, row.icon) {
            parts.push(util::icon(cx, icon, LEGEND_ICON, muted));
        } else if !strong && let Some(mark) = indicator_node(cx, self.tooltip.indicator, row.color) {
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
            parts.push(util::label(cx, &format(value), TextStyle::Caption, Some(Weight::SEMIBOLD), ink));
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
        if let Some(title) = &self.title {
            let ink = theme.color(Semantic::Ink);
            lines.push(util::label(cx, title, TextStyle::Caption, Some(Weight::SEMIBOLD), ink));
        }
        for row in &self.rows {
            lines.push(self.row(cx, row, false));
        }
        if let Some((name, total)) = &self.total {
            let row =
                KeyRow { name: name.clone(), color: Color::TRANSPARENT, icon: None, value: Some(*total) };
            lines.push(self.row(cx, &row, true));
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
        motion::popup(cx, node);
        node
    }
}

/// Where a [`Chart`]'s legend goes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
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
/// is, under the pointer or with the arrow keys; [`ChartKind::Scatter`] and the
/// sparklines stay as plain pictures.
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
    format: ValueFormat,
    width: Option<f32>,
    height: f32,
    look: Look,
    legend_at: ChartLegend,
    tooltip: Option<ChartTooltip>,
    dot_icon: Option<IconName>,
    item_colors: Vec<Semantic>,
    vary_colors: bool,
    negative: Option<Semantic>,
}

/// What [`PlotStyle`] takes as it is. The theme and the data fill in the rest.
#[derive(Clone)]
struct Look {
    curve: ChartCurve,
    stack: ChartStack,
    gradient: bool,
    fill_opacity: Option<f32>,
    horizontal: bool,
    bar_radius: Option<f32>,
    active: Option<usize>,
    dots: bool,
    value_labels: Option<ChartLabelAt>,
    category_labels: Option<ChartLabelAt>,
    tick_format: Option<TickFormat>,
    grid: ChartGrid,
    category_axis: bool,
    value_axis: bool,
    tight_scale: bool,
    no_separator: bool,
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
            bar_radius: None,
            active: None,
            dots: false,
            value_labels: None,
            category_labels: None,
            tick_format: None,
            grid: ChartGrid::Horizontal,
            category_axis: true,
            value_axis: false,
            tight_scale: false,
            no_separator: false,
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
            dot_icon: None,
            item_colors: Vec::new(),
            vary_colors: false,
            negative: None,
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

    /// Fills areas from strong at the top to faint at the bottom.
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

    /// Leaves out the line between slices.
    pub fn no_separator(mut self) -> Self {
        self.look.no_separator = true;
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

    /// What it says about the category under the pointer. A plain tooltip by default.
    pub fn tooltip(mut self, tooltip: ChartTooltip) -> Self {
        self.tooltip = Some(tooltip);
        self
    }

    /// Says nothing under the pointer and takes no keyboard focus.
    pub fn hide_tooltip(mut self) -> Self {
        self.tooltip = None;
        self
    }

    /// Where the legend goes (under the chart by default).
    pub fn legend_at(mut self, at: ChartLegend) -> Self {
        self.legend_at = at;
        self
    }

    /// Every series' values, as a screen reader hears them.
    fn spoken_values(&self) -> String {
        let named = self.series.len() > 1;
        let parts: Vec<String> = self
            .series
            .iter()
            .map(|series| {
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
                let values = spoken.join(", ");
                if named { format!("{}: {values}", series.name) } else { values }
            })
            .collect();
        parts.join("; ")
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
        style.grid = theme.color(Semantic::Border);
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
        let bars = if self.series.len() > 1 { theme.radii.sm } else { theme.radii.md };
        style.bar_radius = look.bar_radius.unwrap_or(bars);
        style.negative = self.negative.map(|role| theme.color(role));
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
        style.category_axis = look.category_axis;
        style.value_axis = look.value_axis;
        style.tight_scale = look.tight_scale;
        if look.no_separator {
            style.separator = 0.0;
        }
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
        style
    }

    fn legend_rows(&self, colors: &[Color], theme: &Theme) -> Vec<KeyRow> {
        if self.colors_by_category() {
            let first = self.series.first().map(|s| s.values.as_slice());
            return self
                .categories
                .iter()
                .enumerate()
                .filter(|(i, _)| first.is_none_or(|values| values.get(*i).is_some_and(|v| *v > 0.0)))
                .map(|(i, name)| KeyRow {
                    name: name.clone(),
                    color: self.category_color(i, colors, theme),
                    icon: None,
                    value: None,
                })
                .collect();
        }
        if self.series.len() < 2 && self.series.iter().all(|s| s.icon.is_none()) {
            return Vec::new();
        }
        self.series
            .iter()
            .enumerate()
            .map(|(i, series)| KeyRow {
                name: series.name.clone(),
                color: self.color_of_series(i, colors),
                icon: series.icon,
                value: None,
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
    ) -> TooltipPanel {
        let category = self.categories.get(index).cloned().unwrap_or_default();
        let rows: Vec<KeyRow> = if self.colors_by_category() && !self.look.concentric {
            let value = self.series.first().and_then(|s| s.values.get(index)).copied();
            vec![KeyRow {
                name: category.clone(),
                color: self.category_color(index, colors, theme),
                icon: None,
                value,
            }]
        } else {
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
                    Some(KeyRow { name: series.name.clone(), color, icon: series.icon, value: Some(value) })
                })
                .collect()
        };
        let by_category = self.colors_by_category() && !self.look.concentric;
        let title = if tooltip.hide_label || by_category {
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
        TooltipPanel { title, rows, total, tooltip: tooltip.clone(), format: self.format.clone() }
    }

    fn build_plot(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let colors = palette(&theme);
        let style = self.plot_style(&theme, &colors);
        let series = self.series.iter().enumerate().map(|(i, series)| SeriesData {
            name: series.name.clone(),
            color: self.color_of_series(i, &colors),
            values: series.values.clone(),
        });
        let data = ChartData {
            categories: self.categories.clone(),
            positions: Vec::new(),
            series: series.collect(),
        };
        let picture = cx.use_state(|| Rc::new(RefCell::new(Picture::default()))).peek().clone();
        let active: State<Option<usize>> = cx.use_state(|| None::<usize>);
        let hovered = cx.is_hover_within();
        let focused = cx.is_focused();
        let shown = active.get().filter(|_| (hovered || focused) && self.tooltip.is_some());

        // The anchor of the tooltip is where the last picture put this category.
        let mut children = Vec::new();
        let spot = shown.and_then(|index| {
            let picture = picture.borrow();
            let at = picture.drawing.anchor_of(index)?;
            Some((at, at.x > picture.size.width / 2.0))
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
        let is_bar = self.kind == ChartKind::Bar;
        let is_line = matches!(self.kind, ChartKind::Line | ChartKind::Area);
        let highlight = shown.map(|index| Highlight {
            index,
            strip: is_bar && cursor,
            line: is_line && cursor,
            dots: is_line || self.kind == ChartKind::Radar,
            strip_color: theme.color(Semantic::Control).with_alpha(0.6),
            line_color: theme.color(Semantic::Border),
            ring_color: theme.color(Semantic::SurfaceElevated),
        });
        let drawing: DrawingFn = Rc::new(move |rect, _| plot::draw(&data, rect, &style));
        let node_style = LayoutStyle { grow: 1.0, align_self: Some(Align::Stretch), ..LayoutStyle::leaf() };
        let node = plot_node(cx, node_style, picture.clone(), drawing, self.dot_icon, highlight, &children);

        if self.tooltip.is_some() {
            let (write, seen) = (active.clone(), picture.clone());
            cx.on_pointer_move(
                node,
                Rc::new(move |position, frame| {
                    let local = Point::new(position.x - frame.x(), position.y - frame.y());
                    let hit = seen.borrow().drawing.hit(local).map(|hit| hit.index);
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

        if let (Some(tooltip), Some(index), Some((_, on_right)), Some(anchor)) =
            (&self.tooltip, shown, spot, anchor)
        {
            let panel = self.tooltip_panel(tooltip, index, &colors, &theme);
            let placement = if on_right { Placement::LeftCenter } else { Placement::RightCenter };
            cx.overlay(anchor, placement, &panel, None);
        }

        let name = self.alt.as_deref().unwrap_or(&self.title);
        let values = self.spoken_values();
        let mut semantics = if name.is_empty() {
            Semantics::new(Role::Image).name(values)
        } else {
            Semantics::new(Role::Image).name(name.to_owned()).description(values)
        };
        if let (Some(tooltip), Some(index)) = (&self.tooltip, shown) {
            let panel = self.tooltip_panel(tooltip, index, &colors, &theme);
            semantics = semantics.value(panel.spoken());
        }
        cx.semantics(node, semantics);
        let m = theme.motion;
        cx.motion(node, Motion::new().enter(Effect::hidden(), m.base));

        let mut parts = Vec::new();
        if !self.title.is_empty() {
            let ink = theme.color(Semantic::Ink);
            parts.push(util::label(cx, &self.title, TextStyle::Callout, Some(Weight::SEMIBOLD), ink));
        }
        let rows = if self.legend { self.legend_rows(&colors, &theme) } else { Vec::new() };
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

impl TooltipPanel {
    /// The same as the panel, for a screen reader.
    fn spoken(&self) -> String {
        let format = self.tooltip.value_format.as_ref().unwrap_or(&self.format);
        let mut parts: Vec<String> = self.title.iter().cloned().collect();
        parts.extend(self.rows.iter().map(|row| match row.value {
            Some(value) => format!("{}: {}", row.name, format(value)),
            None => row.name.clone(),
        }));
        if let Some((name, total)) = &self.total {
            parts.push(format!("{name}: {}", format(*total)));
        }
        parts.join(", ")
    }
}

impl View for Chart {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        if !self.is_plain() {
            return self.build_plot(cx);
        }
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
            .into()
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
        let drawing: DrawingFn = Rc::new(move |rect, scale| {
            barcode_drawing(&pattern, show_text, text_size, radius, rect, scale).into()
        });
        let node = drawing_node(cx, style, drawing);
        let name = self.alt.clone().unwrap_or_else(|| self.data.clone());
        cx.semantics(node, Semantics::new(Role::Image).name(name));
        node
    }
}
