//! A sankey diagram: how much flows from what to what. The layout comes from
//! `ravenrs-graphics`; here its nodes become quads, its ribbons become shapes, so
//! they can fade one by one, and its labels become text.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

use ravenrs_core::{
    ColorStop, CursorIcon, Effect, Key, PathElement, PathFill, PathPaint, Point, Quad, Rect, Role, Size,
};
use ravenrs_graphics::chart::{ChartLabel, LabelAlign};
use ravenrs_graphics::sankey::{self, SankeyHit, SankeyLayout, SankeyLink, SankeyStyle};
use ravenrs_layout::{Align, Axis, LayoutStyle};
use ravenrs_reactive::State;
use ravenrs_style::{Semantic, TextStyle, Theme, Weight};
use ravenrs_text::TextLayout;

pub use ravenrs_graphics::sankey::SankeyAlign;

use crate::chart::{self, ChartTooltip, KeyRow, TooltipPanel, ValueFormat};
use crate::context::{BuildCx, NodeId, Paint, PaintCx, Placement, Semantics};
use crate::motion::Motion;
use crate::{View, util};

/// How solid a ribbon is.
const FLOW_ALPHA: f32 = 0.38;
/// How solid a ribbon in focus is.
const FLOW_FOCUS_ALPHA: f32 = 0.7;
/// How much of its strength what is not in focus gives up.
const FOCUS_DIM: f32 = 0.7;
const NODE_RADIUS: f32 = 2.0;
const LABEL_GAP: f32 = 6.0;
/// How many flows a screen reader hears, and a tooltip shows, before the rest is left out.
const SPOKEN_FLOWS: usize = 16;
const TOOLTIP_FLOWS: usize = 8;

/// A node or a flow of the diagram, as one list: the nodes first.
type Item = usize;

/// What a layout depends on.
#[derive(PartialEq)]
struct Laid {
    size: Size,
    links: Vec<SankeyLink>,
    names: Vec<String>,
    text_size: f32,
    align: SankeyAlign,
}

/// What the diagram looked like when it was last laid out, relative to its frame.
#[derive(Default)]
struct Picture {
    key: Option<Laid>,
    layout: SankeyLayout,
    paths: Vec<Arc<[PathElement]>>,
    labels: Vec<(TextLayout, Point, ravenrs_core::Color)>,
}

impl Picture {
    fn refresh(&mut self, chart: &SankeyChart, size: Size, ink: ravenrs_core::Color, cx: &mut PaintCx<'_>) {
        let key = Laid {
            size,
            links: chart.links.clone(),
            names: chart.names.clone(),
            text_size: chart.text_size,
            align: chart.align,
        };
        if self.key.as_ref() == Some(&key) {
            return;
        }
        let mut style = SankeyStyle::default();
        style.align = chart.align;
        let area = Rect::new(0.0, 0.0, size.width, size.height);
        let layout = sankey::layout(chart.names.len(), &chart.links, area, &style).unwrap_or_default();
        self.paths = layout.flows.iter().map(|flow| chart::path_elements(&flow.path()).into()).collect();
        let height = chart.text_size * 1.3;
        self.labels = layout
            .nodes
            .iter()
            .zip(&chart.names)
            .map(|(node, name)| {
                // Inward, over the flows, so that the outer columns need no margin.
                let last = node.column + 1 == layout.columns && layout.columns > 1;
                let (x, align) = if last {
                    (node.rect.x() - LABEL_GAP - size.width, LabelAlign::Right)
                } else {
                    (node.rect.max_x() + LABEL_GAP, LabelAlign::Left)
                };
                let middle = node.rect.center().y;
                let label = ChartLabel {
                    text: name.clone(),
                    rect: Rect::new(x, middle - height / 2.0, size.width, height),
                    align,
                    size: chart.text_size,
                    bold: false,
                    color: ink,
                };
                chart::typeset(&label, cx)
            })
            .collect();
        self.layout = layout;
        self.key = Some(key);
    }
}

/// What stands out while a node or a flow is in focus, or was until a moment ago.
#[derive(Clone, Copy)]
struct Focus {
    item: Item,
    /// From nothing to in full.
    strength: f32,
}

impl Focus {
    /// Whether this flow is what is in focus, or goes in or out of the node that is.
    fn touches_flow(&self, layout: &SankeyLayout, flow: usize) -> bool {
        let nodes = layout.nodes.len();
        if self.item >= nodes {
            return self.item - nodes == flow;
        }
        layout.flows.get(flow).is_some_and(|f| f.source == self.item || f.target == self.item)
    }

    fn touches_node(&self, layout: &SankeyLayout, node: usize) -> bool {
        let nodes = layout.nodes.len();
        if self.item >= nodes {
            return layout.flows.get(self.item - nodes).is_some_and(|f| f.source == node || f.target == node);
        }
        self.item == node
            || layout.flows.iter().any(|f| {
                (f.source == self.item && f.target == node) || (f.target == self.item && f.source == node)
            })
    }
}

/// How much flows from what to what. Each [`flow`](Self::flow) names its two ends, and
/// nodes are created as they are named. Ribbons go from the color of their source to
/// that of their target; under the pointer or with the arrow keys, what is connected
/// stands out and the tooltip says how much. A screen reader hears every flow.
///
/// If the flows go in a circle there is no way to draw them, and it says which nodes
/// the circle goes through, in place of the diagram. Available with the `charts`
/// feature, on by default.
///
/// ```
/// use ravenrs_widgets::SankeyChart;
///
/// let budget = SankeyChart::new()
///     .title("Budget")
///     .flow("Salary", "Rent", 1200.0)
///     .flow("Salary", "Food", 600.0)
///     .flow("Salary", "Savings", 400.0)
///     .flow("Savings", "Funds", 400.0);
/// ```
#[derive(Clone)]
pub struct SankeyChart {
    names: Vec<String>,
    links: Vec<SankeyLink>,
    colors: Vec<(String, Semantic)>,
    title: String,
    alt: Option<String>,
    height: f32,
    width: Option<f32>,
    align: SankeyAlign,
    format: ValueFormat,
    gradient: bool,
    text_size: f32,
    no_tooltip: bool,
    appear: bool,
    appear_key: u64,
}

impl std::fmt::Debug for SankeyChart {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SankeyChart")
            .field("title", &self.title)
            .field("nodes", &self.names.len())
            .field("flows", &self.links.len())
            .finish_non_exhaustive()
    }
}

impl Default for SankeyChart {
    fn default() -> Self {
        Self::new()
    }
}

impl SankeyChart {
    /// An empty diagram, 300 logical pixels tall, as wide as its parent allows.
    pub fn new() -> Self {
        Self {
            names: Vec::new(),
            links: Vec::new(),
            colors: Vec::new(),
            title: String::new(),
            alt: None,
            height: 300.0,
            width: None,
            align: SankeyAlign::default(),
            format: Rc::new(chart::plain_number),
            gradient: true,
            text_size: 12.0,
            no_tooltip: false,
            appear: true,
            appear_key: 0,
        }
    }

    fn node_of(&mut self, name: String) -> usize {
        if let Some(found) = self.names.iter().position(|known| *known == name) {
            return found;
        }
        self.names.push(name);
        self.names.len() - 1
    }

    /// So much flows from `source` to `target`. Nothing, or less, draws nothing.
    pub fn flow(mut self, source: impl Into<String>, target: impl Into<String>, value: f64) -> Self {
        let source = self.node_of(source.into());
        let target = self.node_of(target.into());
        self.links.push(SankeyLink { source, target, value });
        self
    }

    /// A title above the diagram.
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    /// Describes the diagram for screen readers, in place of its title. Its flows are
    /// read after it.
    pub fn alt(mut self, text: impl Into<String>) -> Self {
        self.alt = Some(text.into());
        self
    }

    /// Height in logical pixels (default 300).
    pub fn height(mut self, height: f32) -> Self {
        self.height = height;
        self
    }

    /// Fixed size in logical pixels.
    pub fn size(mut self, width: f32, height: f32) -> Self {
        self.width = Some(width);
        self.height = height;
        self
    }

    /// Which column a node goes to, among those its flows allow.
    pub fn align(mut self, align: SankeyAlign) -> Self {
        self.align = align;
        self
    }

    /// How numbers are written (at most two decimals by default).
    pub fn format(mut self, format: impl Fn(f64) -> String + 'static) -> Self {
        self.format = Rc::new(format);
        self
    }

    /// Gives this node a color of the theme, in place of the next chart color.
    pub fn node_color(mut self, name: impl Into<String>, color: Semantic) -> Self {
        self.colors.push((name.into(), color));
        self
    }

    /// Ribbons in the color of their source, instead of going to that of their target.
    pub fn no_gradient(mut self) -> Self {
        self.gradient = false;
        self
    }

    /// Says nothing under the pointer and takes no keyboard focus.
    pub fn hide_tooltip(mut self) -> Self {
        self.no_tooltip = true;
        self
    }

    /// The flows are whole from the first frame, instead of drawing in from the left
    /// the first time the chart is painted.
    pub fn no_appear(mut self) -> Self {
        self.appear = false;
        self
    }

    /// Draws the flows in again whenever `key` changes.
    pub fn appear_key(mut self, key: impl std::hash::Hash) -> Self {
        self.appear_key = chart::appear_key_of(key);
        self
    }

    fn color_of(&self, node: usize, colors: &[ravenrs_core::Color], theme: &Theme) -> ravenrs_core::Color {
        let name = &self.names[node];
        match self.colors.iter().find(|(known, _)| known == name) {
            Some((_, role)) => theme.color(*role),
            None => colors[node % colors.len()],
        }
    }

    fn usable(&self) -> impl Iterator<Item = (usize, &SankeyLink)> {
        self.links.iter().filter(|link| link.value.is_finite() && link.value > 0.0).enumerate()
    }

    /// Every flow, as a screen reader hears it.
    fn spoken(&self) -> String {
        let mut spoken: Vec<String> = self
            .usable()
            .take(SPOKEN_FLOWS)
            .map(|(_, link)| {
                let (from, to) = (&self.names[link.source], &self.names[link.target]);
                format!("{from} → {to}: {}", (self.format)(link.value))
            })
            .collect();
        if self.usable().count() > SPOKEN_FLOWS {
            spoken.push("…".to_owned());
        }
        spoken.join("; ")
    }

    /// What a tooltip says about an item.
    fn panel(&self, item: Item, colors: &[ravenrs_core::Color], theme: &Theme) -> TooltipPanel {
        let nodes = self.names.len();
        let flows: Vec<&SankeyLink> = self.usable().map(|(_, link)| link).collect();
        let row = |name: String, node: usize, value: f64| {
            KeyRow::named(name, self.color_of(node, colors, theme), None, Some(value))
        };
        let (title, rows, total) = if item < nodes {
            let incoming = flows.iter().filter(|l| l.target == item);
            let outgoing = flows.iter().filter(|l| l.source == item);
            let coming: f64 = incoming.clone().map(|l| l.value).sum();
            let going: f64 = outgoing.clone().map(|l| l.value).sum();
            let rows: Vec<KeyRow> = incoming
                .map(|l| row(format!("← {}", self.names[l.source]), l.source, l.value))
                .chain(outgoing.map(|l| row(format!("→ {}", self.names[l.target]), l.target, l.value)))
                .take(TOOLTIP_FLOWS)
                .collect();
            let name = self.names[item].clone();
            (name.clone(), rows, Some((name, coming.max(going))))
        } else {
            let link = flows.get(item - nodes).copied();
            let title = link
                .map_or(String::new(), |l| format!("{} → {}", self.names[l.source], self.names[l.target]));
            let rows =
                link.map_or_else(Vec::new, |l| vec![row(self.names[l.target].clone(), l.target, l.value)]);
            (title, rows, None)
        };
        TooltipPanel {
            title: Some(title.clone()),
            rows,
            total,
            tooltip: ChartTooltip::new(),
            format: self.format.clone(),
            index: item,
            category: title,
        }
    }

    /// Where a tooltip about an item points at.
    fn anchor(layout: &SankeyLayout, item: Item) -> Option<Point> {
        let nodes = layout.nodes.len();
        if item < nodes {
            let rect = layout.nodes[item].rect;
            return Some(Point::new(rect.center().x, rect.center().y));
        }
        let flow = layout.flows.get(item - nodes)?;
        Some(Point::new((flow.from.x + flow.to.x) / 2.0, (flow.from.y + flow.to.y) / 2.0))
    }
}

/// What a flow shows: its own color from one end to the other, faded unless in focus.
fn flow_paint(
    from: ravenrs_core::Color,
    to: ravenrs_core::Color,
    flow: &sankey::SankeyFlow,
    alpha: f32,
) -> PathPaint {
    PathPaint::Linear {
        start: flow.from,
        end: flow.to,
        stops: Arc::from(
            [
                ColorStop { offset: 0.0, color: from.with_alpha(alpha) },
                ColorStop { offset: 1.0, color: to.with_alpha(alpha) },
            ]
            .as_slice(),
        ),
    }
}

impl View for SankeyChart {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        if let Err(problem) = sankey::check(self.names.len(), &self.links) {
            return self.problem(cx, &problem, &theme);
        }
        let colors = chart::palette(&theme);
        let picture = cx.use_state(|| Rc::new(RefCell::new(Picture::default()))).peek().clone();
        let active: State<Option<Item>> = cx.use_state(|| None::<Item>);
        let hovered = cx.is_hover_within();
        let focused = cx.is_focused();
        // The flows draw in from the left the first time it is painted, and the
        // tooltip waits until they are whole.
        let draw_in = if self.appear { theme.motion.draw_in } else { ravenrs_animation::Transition::Instant };
        let progress = cx.appear(self.appear_key, draw_in);
        let shown = active.get().filter(|_| (hovered || focused) && !self.no_tooltip && progress >= 1.0);

        // What stands out outlasts the pointer for as long as it takes to fade.
        let held = cx.use_state(|| Rc::new(Cell::new(None::<Item>))).peek().clone();
        if shown.is_some() {
            held.set(shown);
        }
        let m = theme.motion;
        let strength = cx.animated(if shown.is_some() { 1.0_f32 } else { 0.0 }, m.base);
        let focus = held.get().filter(|_| strength > 0.001).map(|item| Focus { item, strength });

        let mut children = Vec::new();
        let spot = shown.and_then(|item| {
            let picture = picture.borrow();
            let at = Self::anchor(&picture.layout, item)?;
            let past = at.x > picture.key.as_ref().map_or(0.0, |key| key.size.width) / 2.0;
            Some((at, if past { Placement::LeftCenter } else { Placement::RightCenter }))
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

        let mut own = self.clone();
        own.text_size = theme.text(TextStyle::Caption).size;
        let (ink, radius) = (theme.color(Semantic::Ink), NODE_RADIUS);
        let node_colors: Vec<ravenrs_core::Color> =
            (0..self.names.len()).map(|node| self.color_of(node, &colors, &theme)).collect();
        let (seen, gradient) = (picture.clone(), self.gradient);
        let node = cx.container(
            LayoutStyle { grow: 1.0, align_self: Some(Align::Stretch), ..LayoutStyle::leaf() },
            &children,
            Paint::Rich(Rc::new(move |frame: Rect, paint: &mut PaintCx<'_>| {
                if frame.is_empty() {
                    return;
                }
                let mut picture = seen.borrow_mut();
                picture.refresh(&own, frame.size, ink, paint);
                let drawing_in = progress < 1.0;
                if drawing_in {
                    let whole = Rect::new(0.0, 0.0, frame.width(), frame.height());
                    paint.scene.push_clip(chart::revealed(frame, whole, progress.max(0.0)));
                }
                let layout = &picture.layout;
                for (index, flow) in layout.flows.iter().enumerate() {
                    let on = focus.is_some_and(|f| f.touches_flow(layout, index));
                    let dim = focus.map_or(0.0, |f| if on { 0.0 } else { FOCUS_DIM * f.strength });
                    let lift = focus.map_or(0.0, |f| if on { f.strength } else { 0.0 });
                    let alpha = (FLOW_ALPHA + (FLOW_FOCUS_ALPHA - FLOW_ALPHA) * lift) * (1.0 - dim);
                    let from = node_colors[flow.source];
                    let to = if gradient { node_colors[flow.target] } else { from };
                    let Some(elements) = picture.paths.get(index) else { continue };
                    paint.scene.push_path(PathFill {
                        elements: elements.clone(),
                        paint: flow_paint(from, to, flow, alpha),
                        transform: [1.0, 0.0, 0.0, 1.0, frame.x(), frame.y()],
                        ..PathFill::solid(elements.clone(), from)
                    });
                }
                for (index, node) in layout.nodes.iter().enumerate() {
                    let on = focus.is_none_or(|f| f.touches_node(layout, index));
                    let dim = focus.map_or(0.0, |f| if on { 0.0 } else { FOCUS_DIM * f.strength });
                    let rect = Rect::new(
                        frame.x() + node.rect.x(),
                        frame.y() + node.rect.y(),
                        node.rect.width(),
                        node.rect.height(),
                    );
                    let color = node_colors[index];
                    paint
                        .scene
                        .push_quad(Quad::new(rect, color.with_alpha(color.a * (1.0 - dim))).radius(radius));
                }
                for (layout, at, color) in &picture.labels {
                    let origin = Point::new(frame.x() + at.x, frame.y() + at.y);
                    layout.paint(origin, *color, paint.scene);
                }
                if drawing_in {
                    paint.scene.pop_clip();
                }
            })),
        );

        if !self.no_tooltip {
            let (write, seen) = (active.clone(), picture.clone());
            cx.on_pointer_move(
                node,
                Rc::new(move |position, frame| {
                    let local = Point::new(position.x - frame.x(), position.y - frame.y());
                    let picture = seen.borrow();
                    let hit = picture.layout.hit(local).map(|hit| match hit {
                        SankeyHit::Node(node) => node,
                        SankeyHit::Flow(flow) => picture.layout.nodes.len() + flow,
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
                    let picture = seen.borrow();
                    let count = picture.layout.nodes.len() + picture.layout.flows.len();
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

        if let (Some(item), Some((_, placement)), Some(anchor)) = (shown, spot, anchor) {
            cx.overlay(anchor, placement, &self.panel(item, &colors, &theme), None);
        }

        let name = self.alt.as_deref().unwrap_or(&self.title);
        let values = self.spoken();
        let mut semantics = if name.is_empty() {
            Semantics::new(Role::Image).name(values)
        } else {
            Semantics::new(Role::Image).name(name.to_owned()).description(values)
        };
        if let Some(item) = shown {
            semantics = semantics.value(self.panel(item, &colors, &theme).spoken());
        }
        cx.semantics(node, semantics);
        if !self.appear {
            cx.motion(node, Motion::new().enter(Effect::hidden(), m.base));
        }

        let mut parts = Vec::new();
        if !self.title.is_empty() {
            let ink = theme.color(Semantic::Ink);
            parts.push(util::label(cx, &self.title, TextStyle::Callout, Some(Weight::SEMIBOLD), ink));
        }
        parts.push(node);
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

impl SankeyChart {
    /// Says what is wrong with the flows, in place of a diagram that cannot exist.
    fn problem(&self, cx: &mut BuildCx<'_>, problem: &sankey::SankeyError, theme: &Theme) -> NodeId {
        let names = |nodes: &[usize]| {
            let names: Vec<&str> =
                nodes.iter().filter_map(|n| self.names.get(*n)).map(String::as_str).collect();
            names.join(" → ")
        };
        let text = match problem {
            sankey::SankeyError::Cycle(nodes) => format!("{}: {}", cx.labels().sankey_cycle, names(nodes)),
            other => other.to_string(),
        };
        let node = util::label(cx, &text, TextStyle::Caption, None, theme.color(Semantic::Danger));
        cx.semantics(node, Semantics::new(Role::Label).name(text));
        node
    }
}
