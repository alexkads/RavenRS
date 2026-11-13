use std::cell::Cell;
use std::rc::Rc;

use ravenrs_core::{CursorIcon, Insets, LinearGradient, Point, Quad, Rect, Role, Scene, Size};
use ravenrs_layout::{Align, Axis, Justify, LayoutStyle};
use ravenrs_reactive::State;
use ravenrs_style::{Semantic, TextStyle, Weight};

use crate::context::{BuildCx, NodeId, Paint, QuadPaint, Semantics};
use crate::overlay::{Entry, MenuList};
use crate::{
    AnyView, Button, ButtonSize, ButtonStyle, Divider, IconButton, IconName, Text, Tooltip, View, ViewSeq,
    util,
};

macro_rules! stack {
    ($(#[$doc:meta])* $name:ident, $axis:expr) => {
        $(#[$doc])*
        pub struct $name<C> {
            children: C,
            style: LayoutStyle,
        }

        #[cfg_attr(feature = "live", ravenrs_live::expose)]
        impl<C: ViewSeq> $name<C> {
            /// Children are a tuple of views (up to 12) or a `Vec`.
            pub fn new(children: C) -> Self {
                Self { children, style: LayoutStyle { spacing: 16.0, ..LayoutStyle::stack($axis) } }
            }

            /// Gap between children, in logical pixels (default 16, `space-4`).
            pub fn spacing(mut self, spacing: f32) -> Self {
                self.style.spacing = spacing;
                self
            }

            /// Inner padding in logical pixels.
            pub fn padding(mut self, padding: impl Into<Insets>) -> Self {
                self.style.padding = padding.into();
                self
            }

            /// Cross-axis alignment of the children.
            pub fn align(mut self, align: Align) -> Self {
                self.style.align = align;
                self
            }

            /// Main-axis distribution of the children.
            pub fn justify(mut self, justify: Justify) -> Self {
                self.style.justify = justify;
                self
            }

            /// Fixed width in logical pixels.
            pub fn width(mut self, width: f32) -> Self {
                self.style.width = Some(width);
                self
            }

            /// Fixed height in logical pixels.
            pub fn height(mut self, height: f32) -> Self {
                self.style.height = Some(height);
                self
            }

            /// Maximum width in logical pixels.
            pub fn max_width(mut self, width: f32) -> Self {
                self.style.max_width = Some(width);
                self
            }

            /// Takes a share of the free space in the parent stack. A growing
            /// stack may also shrink below its content (CSS `min-height: 0`), so
            /// scroll views inside it get a bounded height.
            pub fn grow(mut self, grow: f32) -> Self {
                self.style.grow = grow;
                self.style.min_width.get_or_insert(0.0);
                self.style.min_height.get_or_insert(0.0);
                self
            }

            /// Moves children that do not fit onto new lines (rows wrap down,
            /// columns wrap right).
            pub fn wrap(mut self) -> Self {
                self.style.wrap = true;
                self
            }
        }

        impl<C> std::fmt::Debug for $name<C> {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.debug_struct(stringify!($name)).field("style", &self.style).finish_non_exhaustive()
            }
        }

        impl<C: ViewSeq> View for $name<C> {
            fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
                let children = cx.children(&self.children);
                cx.container(self.style, &children, Paint::None)
            }
        }
    };
}

stack!(
    /// Arranges children top to bottom.
    VStack,
    Axis::Vertical
);
stack!(
    /// Arranges children left to right.
    HStack,
    Axis::Horizontal
);

/// Views on top of each other, the later ones above, in a box as large as the largest.
///
/// Its height comes from the last layout, so the first frame settles in a second
/// layout of the same update. A layer keeps its own height: to fill the stack, give it
/// a height. [`align`](Self::align) places the smaller ones.
///
/// ```
/// use ravenrs_widgets::{Text, ZStack, VStack};
/// use ravenrs_layout::Align;
///
/// let same_place = ZStack::new((
///     VStack::new((Text::new("Taller"), Text::new("content"))),
///     Text::new("Over it"),
/// ))
/// .align(Align::Center);
/// ```
#[derive(Debug)]
pub struct ZStack<C> {
    children: C,
    align: Align,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl<C: ViewSeq> ZStack<C> {
    /// Children are a tuple of views (up to 12) or a `Vec`, the first one at the back.
    pub fn new(children: C) -> Self {
        Self { children, align: Align::Stretch }
    }

    /// Where the smaller layers go, along both axes. `Stretch`, the default, gives them the
    /// whole width and puts them at the top.
    pub fn align(mut self, align: Align) -> Self {
        self.align = align;
        self
    }
}

/// One layer of a [`ZStack`]: a box of no height, so that the layers overlap, around
/// a box that reports the height the content really takes.
struct ZLayer<'a> {
    content: NodeId,
    height: &'a Cell<f32>,
}

impl View for ZLayer<'_> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let inner = cx.container(
            LayoutStyle { align: Align::Stretch, ..LayoutStyle::stack(Axis::Vertical) },
            &[self.content],
            Paint::None,
        );
        cx.report_frame(inner);
        self.height.set(cx.scroll_viewport().map_or(0.0, |size| size.height));
        cx.container(
            LayoutStyle { height: Some(0.0), align: Align::Stretch, ..LayoutStyle::stack(Axis::Vertical) },
            &[inner],
            Paint::None,
        )
    }
}

impl<C: ViewSeq> View for ZStack<C> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let content = cx.children(&self.children);
        let heights = vec![Cell::new(0.0_f32); content.len()];
        let layers: Vec<NodeId> = content
            .iter()
            .zip(&heights)
            .map(|(&content, height)| cx.child(&ZLayer { content, height }))
            .collect();
        let tallest = heights.iter().map(Cell::get).fold(0.0, f32::max);
        let share = match self.align {
            Align::Center => 0.5,
            Align::End => 1.0,
            _ => 0.0,
        };
        for (layer, height) in layers.iter().zip(&heights) {
            cx.translate(*layer, Point::new(0.0, (tallest - height.get()) * share));
        }
        // Across, a column already takes the widest of its children; only the height needs the
        // layers to report theirs.
        cx.container(
            LayoutStyle {
                height: Some(tallest),
                align: self.align,
                align_self: Some(Align::Stretch),
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &layers,
            Paint::None,
        )
    }
}

/// A label on the left and its value on the right, for lists of properties.
///
/// A value that does not fit is cut with an ellipsis, and an empty one shows a dash.
///
/// ```
/// use ravenrs_widgets::{Badge, PropertyRow, VStack};
///
/// let details = VStack::new((
///     PropertyRow::new("Camera", "NIKON D3100"),
///     PropertyRow::new("Lens", ""),
///     PropertyRow::with("Status", Badge::new("Edited")),
/// ));
/// ```
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct PropertyRow {
    label: String,
    value: PropertyValue,
}

enum PropertyValue {
    Text(String),
    View(AnyView),
}

impl std::fmt::Debug for PropertyRow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PropertyRow").field("label", &self.label).finish_non_exhaustive()
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl PropertyRow {
    /// A row showing `value` as text.
    pub fn new(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self { label: label.into(), value: PropertyValue::Text(value.into()) }
    }

    /// A row with a view as its value, such as a badge or a switch.
    pub fn with(label: impl Into<String>, value: impl View + 'static) -> Self {
        Self { label: label.into(), value: PropertyValue::View(Box::new(value)) }
    }
}

impl View for PropertyRow {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let attrs = util::attrs(&theme, TextStyle::Callout, None);
        // The label keeps its size: it is the value that gives way.
        let min_width = Some(cx.measure_text(&self.label, &attrs).width.ceil());
        let label = cx.text(
            &self.label,
            &attrs,
            theme.color(Semantic::InkMuted),
            LayoutStyle { min_width, ..LayoutStyle::leaf() },
        );
        let (value, spoken) = match &self.value {
            PropertyValue::Text(text) => {
                let shown = if text.trim().is_empty() { "—" } else { text };
                let node = cx.text_truncated(shown, &attrs, theme.color(Semantic::Ink), LayoutStyle::leaf());
                cx.semantics(label, Semantics::hidden());
                cx.semantics(node, Semantics::hidden());
                (node, Some(shown.to_owned()))
            }
            PropertyValue::View(view) => (cx.child(view.as_ref()), None),
        };
        let node = cx.container(
            LayoutStyle {
                spacing: theme.spacing.s2,
                justify: Justify::SpaceBetween,
                align: Align::Center,
                align_self: Some(Align::Stretch),
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &[label, value],
            Paint::None,
        );
        if let Some(spoken) = spoken {
            cx.semantics(node, Semantics::new(Role::Label).name(self.label.clone()).value(spoken));
        }
        node
    }
}

/// The top of a page: its title, a line about it, badges beside the title and actions at the end.
///
/// ```
/// use ravenrs_widgets::{Badge, Button, PageHeader};
///
/// let header = PageHeader::new("Sessions")
///     .description("Every shoot, newest first.")
///     .badge(Badge::new("12"))
///     .action(Button::new("New session"));
/// ```
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct PageHeader {
    title: String,
    description: Option<String>,
    badges: Vec<AnyView>,
    actions: Vec<AnyView>,
    compact: bool,
}

impl std::fmt::Debug for PageHeader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PageHeader").field("title", &self.title).finish_non_exhaustive()
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl PageHeader {
    /// A header with this title.
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            description: None,
            badges: Vec::new(),
            actions: Vec::new(),
            compact: false,
        }
    }

    /// The line about the page. A long one is cut with an ellipsis.
    pub fn description(mut self, text: impl Into<String>) -> Self {
        self.description = Some(text.into());
        self
    }

    /// Adds a view beside the title, usually a badge.
    pub fn badge(mut self, badge: impl View + 'static) -> Self {
        self.badges.push(Box::new(badge));
        self
    }

    /// Adds a view at the end of the header, usually a button.
    pub fn action(mut self, action: impl View + 'static) -> Self {
        self.actions.push(Box::new(action));
        self
    }

    /// A single line, with a smaller title and the description beside it, for tool screens.
    pub fn compact(mut self) -> Self {
        self.compact = true;
        self
    }
}

impl View for PageHeader {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let ink = theme.color(Semantic::Ink);
        let (style, weight, level) = if self.compact {
            (TextStyle::Headline, Some(Weight::SEMIBOLD), 2)
        } else {
            (TextStyle::Title, None, 1)
        };
        let title = util::label(cx, &self.title, style, weight, ink);
        cx.semantics(title, Semantics::new(Role::Heading).level(level));
        let description = self.description.as_ref().map(|text| {
            let attrs = util::attrs(&theme, TextStyle::Callout, None);
            cx.text_truncated(text, &attrs, theme.color(Semantic::InkMuted), LayoutStyle::leaf())
        });
        let mut top = vec![title];
        if self.compact {
            top.extend(description);
        }
        top.extend(self.badges.iter().map(|badge| cx.child(badge.as_ref())));
        let top = cx.container(
            LayoutStyle {
                spacing: theme.spacing.s2,
                align: Align::Center,
                min_width: Some(0.0),
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &top,
            Paint::None,
        );
        let mut text = vec![top];
        if !self.compact {
            text.extend(description);
        }
        let text = cx.container(
            LayoutStyle {
                spacing: theme.spacing.s1,
                grow: 1.0,
                min_width: Some(0.0),
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &text,
            Paint::None,
        );
        let mut children = vec![text];
        if !self.actions.is_empty() {
            let actions: Vec<NodeId> = self.actions.iter().map(|action| cx.child(action.as_ref())).collect();
            children.push(cx.container(
                LayoutStyle {
                    spacing: theme.spacing.s2,
                    align: Align::Center,
                    ..LayoutStyle::stack(Axis::Horizontal)
                },
                &actions,
                Paint::None,
            ));
        }
        cx.container(
            LayoutStyle {
                spacing: theme.spacing.s4,
                align: Align::Center,
                align_self: Some(Align::Stretch),
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &children,
            Paint::None,
        )
    }
}

/// Flexible empty space that pushes its siblings apart inside a stack.
#[derive(Clone, Copy, Debug, Default)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct Spacer {
    min: f32,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl Spacer {
    /// A spacer with no minimum size.
    pub fn new() -> Self {
        Self::default()
    }

    /// The smallest size the spacer keeps along the stack's axis.
    pub fn min(mut self, min: f32) -> Self {
        self.min = min;
        self
    }
}

impl View for Spacer {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let style = LayoutStyle {
            grow: 1.0,
            min_width: Some(self.min),
            min_height: Some(self.min),
            ..LayoutStyle::leaf()
        };
        cx.container(style, &[], Paint::None)
    }
}

/// A vertically scrolling stack. It fills the space its parent gives it (or
/// `height`) and scrolls its children with the wheel or trackpad, showing a
/// thin scrollbar while they move.
pub struct ScrollView<C> {
    children: C,
    style: LayoutStyle,
    horizontal: bool,
    scrollbar_outside: f32,
}

impl<C> std::fmt::Debug for ScrollView<C> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ScrollView").field("style", &self.style).finish_non_exhaustive()
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl<C: ViewSeq> ScrollView<C> {
    /// A scroll view of `children` (a tuple of up to 12 views or a `Vec`).
    pub fn new(children: C) -> Self {
        Self {
            children,
            style: LayoutStyle {
                spacing: 16.0,
                grow: 1.0,
                align: Align::Stretch,
                overflow_hidden: true,
                min_height: Some(0.0),
                ..LayoutStyle::stack(Axis::Vertical)
            },
            horizontal: false,
            scrollbar_outside: 0.0,
        }
    }

    /// Puts the scrollbar this far past the view's right edge, for a view inside a
    /// padded surface such as a dialog, so that it never passes over what it scrolls.
    #[must_use]
    pub fn scrollbar_outside(mut self, by: f32) -> Self {
        self.scrollbar_outside = by;
        self
    }

    /// Also scrolls sideways, for content wider than the view (a zoomed page,
    /// a wide canvas). Children keep their own width instead of stretching.
    pub fn horizontal(mut self) -> Self {
        self.horizontal = true;
        self.style.align = Align::Start;
        self.style.min_width = Some(0.0);
        self
    }

    /// Gap between children, in logical pixels (default 16).
    pub fn spacing(mut self, spacing: f32) -> Self {
        self.style.spacing = spacing;
        self
    }

    /// Inner padding in logical pixels.
    pub fn padding(mut self, padding: impl Into<Insets>) -> Self {
        self.style.padding = padding.into();
        self
    }

    /// A fixed height instead of filling the parent.
    pub fn height(mut self, height: f32) -> Self {
        self.style.height = Some(height);
        self.style.grow = 0.0;
        self
    }

    /// Cross-axis alignment of the children (`Stretch` by default).
    pub fn align(mut self, align: Align) -> Self {
        self.style.align = align;
        self
    }
}

impl<C: ViewSeq> View for ScrollView<C> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let children = cx.children(&self.children);
        let node = cx.container(self.style, &children, Paint::None);
        cx.scrollable(node);
        if self.scrollbar_outside != 0.0 {
            cx.scrollbar_outside(node, self.scrollbar_outside);
        }
        if self.horizontal {
            cx.scroll_horizontally(node);
        }
        node
    }
}

/// How long the fade of a [`ScrollEdgeArrow`] is, in logical pixels.
const EDGE_ARROW_LENGTH: f32 = 40.0;
/// The least an arrow of a [`ScrollEdgeArrow`] scrolls, in logical pixels.
const EDGE_ARROW_LEAST_STEP: f32 = 200.0;

/// A strip that scrolls, with an arrow over a fade at each end that has more to show. The
/// arrow scrolls most of the strip at a time, and leaves when that end is reached.
///
/// The strip is horizontal unless [`ScrollEdgeArrow::vertical`]. Children keep their size
/// when they have one, so give them a width: without it they share the room first, as in any
/// row, down to what their content needs. Every scroll builds the strip again to know
/// whether an arrow still applies, so what goes in should be cheap to build.
///
/// ```
/// use ravenrs_widgets::{Button, ScrollEdgeArrow, Ui};
///
/// let photos: Vec<Button> = (1..=30).map(|n| Button::new(format!("Photo {n}"))).collect();
/// let strip = ScrollEdgeArrow::new(photos).spacing(8.0);
/// let mut ui = Ui::new(strip, ravenrs_style::Theme::raven_dark());
/// ui.update();
/// ```
pub struct ScrollEdgeArrow<C> {
    children: C,
    vertical: bool,
    spacing: f32,
    padding: Insets,
    height: Option<f32>,
    fade: Semantic,
}

impl<C> std::fmt::Debug for ScrollEdgeArrow<C> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ScrollEdgeArrow").field("vertical", &self.vertical).finish_non_exhaustive()
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl<C: ViewSeq> ScrollEdgeArrow<C> {
    /// A horizontal strip of `children` (a tuple of up to 12 views or a `Vec`).
    pub fn new(children: C) -> Self {
        Self {
            children,
            vertical: false,
            spacing: 8.0,
            padding: Insets::default(),
            height: None,
            fade: Semantic::Canvas,
        }
    }

    /// Scrolls up and down, taking the height its parent gives it.
    pub fn vertical(mut self) -> Self {
        self.vertical = true;
        self
    }

    /// Gap between children, in logical pixels (default 8).
    pub fn spacing(mut self, spacing: f32) -> Self {
        self.spacing = spacing;
        self
    }

    /// Inner padding in logical pixels.
    pub fn padding(mut self, padding: impl Into<Insets>) -> Self {
        self.padding = padding.into();
        self
    }

    /// A fixed height. A horizontal strip takes the height of its children by default.
    pub fn height(mut self, height: f32) -> Self {
        self.height = Some(height);
        self
    }

    /// The color the arrows fade from, which should be what is behind the strip
    /// ([`Semantic::Canvas`] by default).
    pub fn fade(mut self, fade: Semantic) -> Self {
        self.fade = fade;
        self
    }
}

/// One arrow of a [`ScrollEdgeArrow`], which any view that scrolls by itself can also use.
#[allow(missing_debug_implementations)] // internal: shared with the components' crates, hidden
#[doc(hidden)]
pub struct EdgeArrow {
    /// At the end of the strip rather than at its start.
    pub forward: bool,
    pub vertical: bool,
    /// Its frame inside its parent.
    pub frame: Rect,
    pub fade: Semantic,
    pub label: String,
    pub on_click: Rc<dyn Fn()>,
}

impl View for EdgeArrow {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let ink = theme.color(Semantic::Ink);
        let color = if cx.is_hovered() { ink } else { ink.with_alpha(ink.a * 0.7) };
        let icon_name = match (self.vertical, self.forward) {
            (false, false) => IconName::ChevronLeft,
            (false, true) => IconName::ChevronRight,
            (true, false) => IconName::ChevronUp,
            (true, true) => IconName::ChevronDown,
        };
        let icon = util::icon(cx, icon_name, 20.0, color);
        let solid = if self.forward { 1.0 } else { 0.0 };
        let (start, end) = if self.vertical {
            (Point::new(0.5, solid), Point::new(0.5, 1.0 - solid))
        } else {
            (Point::new(solid, 0.5), Point::new(1.0 - solid, 0.5))
        };
        let from = theme.color(self.fade);
        let node = cx.container(
            LayoutStyle {
                absolute: Some((self.frame.x(), self.frame.y())),
                width: Some(self.frame.width()),
                height: Some(self.frame.height()),
                align: Align::Center,
                justify: Justify::Center,
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &[icon],
            Paint::Quad(QuadPaint::new(LinearGradient { start, end, from, to: from.with_alpha(0.0) })),
        );
        cx.on_click(node, self.on_click.clone());
        cx.semantics(node, Semantics::new(Role::Button).name(self.label.clone()));
        cx.focusable(node, theme.radii.sm);
        cx.cursor(node, CursorIcon::Pointer);
        node
    }
}

impl<C: ViewSeq> View for ScrollEdgeArrow<C> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let labels = cx.labels();
        let vertical = self.vertical;
        let along = move |size: Size| if vertical { size.height } else { size.width };
        let view = cx.scroll_viewport().unwrap_or(Size::ZERO);
        let content = cx.measured_content().unwrap_or(Size::ZERO);
        let last = (along(content) - along(view)).max(0.0);
        let offset = cx.scroll_offset();

        // An arrow can only ask: a scroll happens during a build.
        let wanted: State<Option<f32>> = cx.use_state(|| None);
        let asked = wanted.get();
        let here = asked.map_or(if vertical { offset.y } else { offset.x }, |to| to.clamp(0.0, last));
        if asked.is_some() {
            wanted.set(None);
            cx.set_scroll_offset(if vertical {
                Point::new(offset.x, here)
            } else {
                Point::new(here, offset.y)
            });
        }

        let children = cx.children(&self.children);
        let strip = cx.container(
            LayoutStyle {
                spacing: self.spacing,
                padding: self.padding,
                align: if vertical { Align::Stretch } else { Align::Center },
                overflow_hidden: true,
                grow: 1.0,
                min_width: Some(0.0),
                min_height: Some(0.0),
                ..LayoutStyle::stack(if vertical { Axis::Vertical } else { Axis::Horizontal })
            },
            &children,
            Paint::None,
        );
        // Rebuilt on every scroll, and told the size of what scrolls: that is what the arrows go by.
        cx.scrollable_virtual(strip, !vertical);
        cx.measure_content(strip);

        let step = (along(view) * 0.8).max(EDGE_ARROW_LEAST_STEP);
        let arrow = |forward: bool| {
            let at_end = along(view) - EDGE_ARROW_LENGTH;
            let frame = match (vertical, forward) {
                (false, false) => Rect::new(0.0, 0.0, EDGE_ARROW_LENGTH, view.height),
                (false, true) => Rect::new(at_end, 0.0, EDGE_ARROW_LENGTH, view.height),
                (true, false) => Rect::new(0.0, 0.0, view.width, EDGE_ARROW_LENGTH),
                (true, true) => Rect::new(0.0, at_end, view.width, EDGE_ARROW_LENGTH),
            };
            let label = if forward { &labels.scroll_forward } else { &labels.scroll_back };
            let wanted = wanted.clone();
            let target = if forward { here + step } else { here - step };
            EdgeArrow {
                forward,
                vertical,
                frame,
                fade: self.fade,
                label: label.clone(),
                on_click: Rc::new(move || wanted.set(Some(target))),
            }
        };
        let show_back = here > 0.5;
        let show_forward = here < last - 0.5;
        let mut nodes = vec![strip];
        let back = show_back.then(|| cx.keyed_child(1, &arrow(false)));
        let forward = show_forward.then(|| cx.keyed_child(2, &arrow(true)));
        nodes.extend(back);
        nodes.extend(forward);
        // Going off the end takes away the arrow that had the keyboard: it moves to the other one.
        if asked.is_some() {
            match (back, forward) {
                (Some(other), None) | (None, Some(other)) => cx.request_focus_node(other),
                _ => {}
            }
        }
        cx.container(
            LayoutStyle {
                height: self.height,
                grow: if vertical && self.height.is_none() { 1.0 } else { 0.0 },
                min_width: Some(0.0),
                min_height: Some(0.0),
                align: Align::Stretch,
                align_self: Some(Align::Stretch),
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &nodes,
            Paint::None,
        )
    }
}

/// Lays children out in equal-width columns, wrapping into rows.
pub struct Grid<C> {
    children: C,
    style: LayoutStyle,
}

impl<C> std::fmt::Debug for Grid<C> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Grid").field("style", &self.style).finish_non_exhaustive()
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl<C: ViewSeq> Grid<C> {
    /// A grid of `columns` equal columns (at least 1).
    pub fn new(columns: u16, children: C) -> Self {
        Self {
            children,
            style: LayoutStyle {
                spacing: 16.0,
                grid_columns: Some(columns.max(1)),
                align_self: Some(Align::Stretch),
                ..LayoutStyle::stack(Axis::Vertical)
            },
        }
    }

    /// Gap between cells, in logical pixels (default 16).
    pub fn spacing(mut self, spacing: f32) -> Self {
        self.style.spacing = spacing;
        self
    }
}

impl<C: ViewSeq> View for Grid<C> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let children = cx.children(&self.children);
        cx.container(self.style, &children, Paint::None)
    }
}

/// Window width below which a [`Footer`] stacks its areas.
const FOOTER_BREAKPOINT: f32 = 640.0;

/// The band that ends a page or an app: copyright, links, version, status.
///
/// It has a left, a center and a right area, each a tuple of views (up to 12)
/// or a `Vec`, whose items move to a new line when they do not fit. The center
/// stays centered however wide the sides are. Below 640 logical pixels of
/// window width (see [`Self::breakpoint`]) the areas stack, centered. Screen
/// readers find it as the page's `contentinfo` landmark.
///
/// ```
/// use ravenrs_widgets::{Badge, Footer, Tone};
///
/// let footer = Footer::new()
///     .left((Footer::text("© 2026 Raven Finance"),))
///     .center((Footer::link("Privacy"), Footer::link("Terms")))
///     .right((Footer::text("v27.1"), Badge::new("Online").tone(Tone::Success)));
/// ```
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct Footer {
    left: Option<Box<dyn ViewSeq>>,
    center: Option<Box<dyn ViewSeq>>,
    right: Option<Box<dyn ViewSeq>>,
    divider: bool,
    breakpoint: f32,
    stacked: bool,
}

impl std::fmt::Debug for Footer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Footer")
            .field("divider", &self.divider)
            .field("breakpoint", &self.breakpoint)
            .field("stacked", &self.stacked)
            .finish_non_exhaustive()
    }
}

impl Default for Footer {
    fn default() -> Self {
        Self {
            left: None,
            center: None,
            right: None,
            divider: true,
            breakpoint: FOOTER_BREAKPOINT,
            stacked: false,
        }
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl Footer {
    /// An empty footer with a divider on top.
    pub fn new() -> Self {
        Self::default()
    }

    /// Text in a footer's usual look: small and muted.
    pub fn text(text: impl Into<String>) -> Text {
        Text::new(text).style(TextStyle::Caption).foreground(Semantic::InkMuted)
    }

    /// A link in a footer's usual look: a small ghost [`Button`], so it has a
    /// focus ring and works with the keyboard. Add `on_click` to it.
    pub fn link(label: impl Into<String>) -> Button {
        Button::new(label).style(ButtonStyle::Ghost).size(ButtonSize::Small)
    }

    /// The views at the left end.
    pub fn left(mut self, items: impl ViewSeq + 'static) -> Self {
        self.left = Some(Box::new(items));
        self
    }

    /// The views in the middle.
    pub fn center(mut self, items: impl ViewSeq + 'static) -> Self {
        self.center = Some(Box::new(items));
        self
    }

    /// The views at the right end.
    pub fn right(mut self, items: impl ViewSeq + 'static) -> Self {
        self.right = Some(Box::new(items));
        self
    }

    /// Shows the hairline along the top (on by default).
    pub fn divider(mut self, divider: bool) -> Self {
        self.divider = divider;
        self
    }

    /// The window width, in logical pixels, below which the areas stack
    /// (default 640).
    pub fn breakpoint(mut self, width: f32) -> Self {
        self.breakpoint = width;
        self
    }

    /// Always stacks the areas, for a footer in a narrow panel of a wide window:
    /// the breakpoint only knows the window.
    pub fn stacked(mut self) -> Self {
        self.stacked = true;
        self
    }

    fn body(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let spacing = cx.theme().spacing;
        let area = |items, justify| FooterArea { items, justify };
        let (left, center, right) = (self.left.as_deref(), self.center.as_deref(), self.right.as_deref());
        let center_only = left.is_none() && right.is_none();
        if self.stacked || center_only || cx.viewport().width < self.breakpoint {
            let areas: Vec<_> =
                [left, center, right].into_iter().flatten().map(|a| area(Some(a), Justify::Center)).collect();
            return cx.child(&VStack::new(areas).spacing(spacing.s2).align(Align::Stretch));
        }
        let (start, end) = (area(left, Justify::Start), area(right, Justify::End));
        if center.is_some() {
            // Equal columns are what keep the center centered between uneven sides.
            cx.child(&Grid::new(3, (start, area(center, Justify::Center), end)).spacing(spacing.s4))
        } else {
            let sides = HStack::new((start, end))
                .spacing(spacing.s4)
                .align(Align::Center)
                .justify(Justify::SpaceBetween);
            cx.child(&sides.wrap())
        }
    }
}

impl View for Footer {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let spacing = cx.theme().spacing;
        let body = self.body(cx);
        let content = cx.container(
            LayoutStyle {
                padding: Insets::symmetric(spacing.s3, spacing.s4),
                align: Align::Stretch,
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &[body],
            Paint::None,
        );
        let divider = self.divider.then(|| cx.child(&Divider::horizontal()));
        let children: Vec<NodeId> = divider.into_iter().chain([content]).collect();
        let node = cx.container(
            LayoutStyle {
                align: Align::Stretch,
                align_self: Some(Align::Stretch),
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &children,
            Paint::None,
        );
        cx.semantics(node, Semantics::new(Role::ContentInfo));
        node
    }
}

/// One area of a [`Footer`]. It exists even when empty, to hold its grid column.
struct FooterArea<'a> {
    items: Option<&'a dyn ViewSeq>,
    justify: Justify,
}

impl View for FooterArea<'_> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let spacing = cx.theme().spacing.s3;
        let children = self.items.map_or_else(Vec::new, |items| cx.children(items));
        cx.container(
            LayoutStyle {
                spacing,
                align: Align::Center,
                justify: self.justify,
                wrap: true,
                // Lets a long area wrap inside its grid column instead of widening it.
                min_width: Some(0.0),
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &children,
            Paint::None,
        )
    }
}

/// The height of a [`TitleBar`], in logical pixels.
const TITLE_BAR_HEIGHT: f32 = 38.0;

/// A title bar drawn by the app: what comes first, the title, the actions and the buttons of
/// the window. A button shows only when it has a handler. A double click on the bar does the
/// same as the maximize button.
///
/// It draws and calls back, and that is all. What the handlers do to the window is up to the
/// app. **It cannot drag the window nor hide the frame the system draws**, because those belong
/// to the platform layer, which offers neither yet: for now the system's own bar stays above it.
///
/// ```
/// use ravenrs_widgets::{IconButton, IconName, TitleBar, Ui};
///
/// let bar = TitleBar::new("Report.pdf")
///     .action(IconButton::new(IconName::Search).label("Search"))
///     .on_minimize(|| {})
///     .on_maximize(|| {})
///     .on_close(|| {});
/// let mut ui = Ui::new(bar, ravenrs_style::Theme::raven_dark());
/// ui.update();
/// ```
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct TitleBar {
    title: String,
    leading: Option<AnyView>,
    actions: Vec<AnyView>,
    on_minimize: Option<Rc<dyn Fn()>>,
    on_maximize: Option<Rc<dyn Fn()>>,
    on_close: Option<Rc<dyn Fn()>>,
    on_context_menu: Option<Rc<dyn Fn(Point)>>,
    maximized: bool,
    active: bool,
}

impl std::fmt::Debug for TitleBar {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TitleBar").field("title", &self.title).finish_non_exhaustive()
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl TitleBar {
    /// A bar with this title and no buttons.
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            leading: None,
            actions: Vec::new(),
            on_minimize: None,
            on_maximize: None,
            on_close: None,
            on_context_menu: None,
            maximized: false,
            active: true,
        }
    }

    /// Any view before the title, such as an icon or a menu.
    pub fn leading(mut self, view: impl View + 'static) -> Self {
        self.leading = Some(Box::new(view));
        self
    }

    /// Appends a view to the area between the title and the window's buttons.
    pub fn action(mut self, view: impl View + 'static) -> Self {
        self.actions.push(Box::new(view));
        self
    }

    /// Shows the minimize button, which runs `handler`.
    pub fn on_minimize(mut self, handler: impl Fn() + 'static) -> Self {
        self.on_minimize = Some(Rc::new(handler));
        self
    }

    /// Shows the maximize button, which runs `handler`, as a double click on the bar does.
    pub fn on_maximize(mut self, handler: impl Fn() + 'static) -> Self {
        self.on_maximize = Some(Rc::new(handler));
        self
    }

    /// Shows the close button, which runs `handler`.
    pub fn on_close(mut self, handler: impl Fn() + 'static) -> Self {
        self.on_close = Some(Rc::new(handler));
        self
    }

    /// Called with the pointer's place on a right click on the bar, for a menu of the window.
    pub fn on_context_menu(mut self, handler: impl Fn(Point) + 'static) -> Self {
        self.on_context_menu = Some(Rc::new(handler));
        self
    }

    /// The window is maximized: the maximize button shows and says that it restores.
    pub fn maximized(mut self, maximized: bool) -> Self {
        self.maximized = maximized;
        self
    }

    /// Whether the window has the keyboard (it does by default). Without it the bar goes quiet,
    /// which is how to tell which of two windows side by side is listening.
    pub fn active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }
}

impl View for TitleBar {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let labels = cx.labels();
        let mut children = Vec::new();
        if let Some(leading) = &self.leading {
            children.push(cx.keyed_child(1, leading));
        }
        let ink = theme.color(if self.active { Semantic::Ink } else { Semantic::InkMuted });
        children.push(cx.text_truncated(
            &self.title,
            &util::attrs(&theme, TextStyle::Callout, Some(Weight::MEDIUM)),
            ink,
            LayoutStyle { grow: 1.0, min_width: Some(0.0), ..LayoutStyle::leaf() },
        ));
        for (i, action) in self.actions.iter().enumerate() {
            children.push(cx.keyed_child(10 + i as u64, action));
        }
        let (middle_icon, middle_label) = if self.maximized {
            (IconName::Copy, &labels.window_restore)
        } else {
            (IconName::Square, &labels.window_maximize)
        };
        let buttons = [
            (&self.on_minimize, IconName::Minus, &labels.window_minimize),
            (&self.on_maximize, middle_icon, middle_label),
            (&self.on_close, IconName::X, &labels.close),
        ];
        for (i, (handler, icon, label)) in buttons.into_iter().enumerate() {
            if let Some(handler) = handler.clone() {
                let button = Tooltip::new(
                    IconButton::new(icon)
                        .label(label.clone())
                        .size(ButtonSize::Mini)
                        .on_click(move || handler()),
                    label.clone(),
                );
                children.push(cx.keyed_child(2 + i as u64, &button));
            }
        }
        let fill = theme.color(if self.active { Semantic::Surface } else { Semantic::Canvas });
        let line = theme.color(Semantic::Border);
        let node = cx.container(
            LayoutStyle {
                height: Some(TITLE_BAR_HEIGHT),
                padding: Insets::symmetric(0.0, theme.spacing.s3),
                spacing: theme.spacing.s2,
                align: Align::Center,
                align_self: Some(Align::Stretch),
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &children,
            Paint::Custom(Rc::new(move |f: Rect, scene: &mut Scene| {
                scene.push_quad(Quad::new(f, fill));
                scene.push_quad(Quad::new(Rect::new(f.x(), f.max_y() - 1.0, f.width(), 1.0), line));
            })),
        );
        if let Some(maximize) = &self.on_maximize {
            cx.on_double_click(node, maximize.clone());
        }
        if let Some(menu) = &self.on_context_menu {
            cx.on_context_menu(node, menu.clone());
        }
        cx.semantics(node, Semantics::new(Role::Group).name(self.title.clone()));
        node
    }
}

/// The one line at the foot of an app window that is always there: the version,
/// what is going on in the background, a notice with its actions.
///
/// Where a [`Footer`] ends a page and wraps or stacks to show everything, a
/// status bar keeps one fixed line: its sides keep their width and the center
/// takes the rest, cutting what does not fit, so it never pushes the app around.
/// Tinting the center with [`Self::center_tone`] makes a notice stand out without
/// the whole bar shouting. The center is announced to screen readers as a status.
///
/// ```
/// use ravenrs_widgets::{StatusBar, Tone, ViewExt};
///
/// let bar = StatusBar::new()
///     .left((StatusBar::action("Version 1.2"),))
///     .center((StatusBar::text("Version 1.3 is available").truncate().grow(1.0), StatusBar::action("Update")))
///     .center_tone(Tone::Warning)
///     .right((StatusBar::text("12 items"),));
/// ```
#[derive(Default)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct StatusBar {
    left: Option<Box<dyn ViewSeq>>,
    center: Option<Box<dyn ViewSeq>>,
    right: Option<Box<dyn ViewSeq>>,
    center_tone: Option<crate::Tone>,
}

impl std::fmt::Debug for StatusBar {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StatusBar").field("center_tone", &self.center_tone).finish_non_exhaustive()
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl StatusBar {
    /// Its height, in logical pixels, for what has to clear it.
    pub const HEIGHT: f32 = 28.0;

    /// An empty status bar.
    pub fn new() -> Self {
        Self::default()
    }

    /// Text in a status bar's usual look: small and muted.
    pub fn text(text: impl Into<String>) -> Text {
        Text::new(text).style(TextStyle::Caption).foreground(Semantic::InkMuted)
    }

    /// An action that fits the line: a mini ghost [`Button`], with a focus ring
    /// and the keyboard. Add `on_click` to it.
    pub fn action(label: impl Into<String>) -> Button {
        Button::new(label).style(ButtonStyle::Ghost).size(ButtonSize::Mini)
    }

    /// The views at the left end.
    pub fn left(mut self, items: impl ViewSeq + 'static) -> Self {
        self.left = Some(Box::new(items));
        self
    }

    /// The views in the middle, which get the width the sides leave. Give the
    /// text `.truncate().grow(1.0)` so that it is what gets cut.
    pub fn center(mut self, items: impl ViewSeq + 'static) -> Self {
        self.center = Some(Box::new(items));
        self
    }

    /// The views at the right end.
    pub fn right(mut self, items: impl ViewSeq + 'static) -> Self {
        self.right = Some(Box::new(items));
        self
    }

    /// Tints the middle, and only the middle, to make a notice stand out.
    pub fn center_tone(mut self, tone: crate::Tone) -> Self {
        self.center_tone = Some(tone);
        self
    }

    fn side(cx: &mut BuildCx<'_>, items: Option<&dyn ViewSeq>) -> NodeId {
        let spacing = cx.theme().spacing.s1;
        let children = items.map_or_else(Vec::new, |items| cx.children(items));
        cx.container(
            LayoutStyle { spacing, align: Align::Center, ..LayoutStyle::stack(Axis::Horizontal) },
            &children,
            Paint::None,
        )
    }
}

impl View for StatusBar {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let left = Self::side(cx, self.left.as_deref());
        let right = Self::side(cx, self.right.as_deref());
        let items = self.center.as_deref().map_or_else(Vec::new, |items| cx.children(items));
        let has_center = !items.is_empty();
        let fill = self.center_tone.filter(|_| has_center).map(|tone| tone.color(&theme).faded(0.16));
        let center = cx.container(
            LayoutStyle {
                spacing: theme.spacing.s1,
                padding: Insets::symmetric(0.0, theme.spacing.s2),
                align: Align::Center,
                align_self: Some(Align::Stretch),
                grow: 1.0,
                min_width: Some(0.0),
                overflow_hidden: true,
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &items,
            fill.map_or(Paint::None, |fill| Paint::Quad(QuadPaint::new(fill))),
        );
        if has_center {
            cx.semantics(center, Semantics::new(Role::Status));
        }
        let line = cx.container(
            LayoutStyle {
                spacing: theme.spacing.s2,
                padding: Insets::symmetric(0.0, theme.spacing.s2),
                align: Align::Center,
                height: Some(Self::HEIGHT),
                align_self: Some(Align::Stretch),
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &[left, center, right],
            Paint::None,
        );
        let divider = cx.child(&Divider::horizontal());
        let node = cx.container(
            LayoutStyle {
                align: Align::Stretch,
                align_self: Some(Align::Stretch),
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &[divider, line],
            Paint::None,
        );
        cx.semantics(node, Semantics::new(Role::ContentInfo));
        node
    }
}

/// One place of a [`ToolRail`]: a tool, and the others that can take its place.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct ToolRailItem {
    tools: Vec<RailToolInfo>,
}

#[derive(Clone, Debug)]
struct RailToolInfo {
    icon: crate::IconName,
    name: String,
    shortcut: Option<String>,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl ToolRailItem {
    /// A place holding this tool.
    pub fn new(icon: crate::IconName, name: impl Into<String>) -> Self {
        Self { tools: vec![RailToolInfo { icon, name: name.into(), shortcut: None }] }
    }

    /// Shows this key beside the tool added last, such as `"M"` (display only).
    pub fn shortcut(mut self, keys: impl Into<String>) -> Self {
        if let Some(tool) = self.tools.last_mut() {
            tool.shortcut = Some(keys.into());
        }
        self
    }

    /// Adds a tool that shares this place. The others open in a menu at its side, by
    /// holding the button down, by a right click or with the Right arrow, and the
    /// place shows the last one used.
    pub fn variant(mut self, icon: crate::IconName, name: impl Into<String>) -> Self {
        self.tools.push(RailToolInfo { icon, name: name.into(), shortcut: None });
        self
    }
}

#[derive(Clone, Debug)]
enum RailEntry {
    Tool(ToolRailItem),
    Separator,
}

/// How long a press on a tool with variants takes to open them, in seconds.
const RAIL_HOLD: f64 = 0.4;
/// The side of a tool's button when the rail shows only icons.
const RAIL_ICON_BUTTON: f32 = 36.0;

/// A narrow bar of tools with their names under their icons, of which one is
/// open: what sits along the side of an editor, with the open tool's panel next to it.
///
/// `selected` counts every tool in order, the variants of a place included. It can
/// also show only icons, in one column or more, with lines between groups of tools.
///
/// ```
/// use ravenrs_reactive::State;
/// use ravenrs_widgets::{IconName, ToolRail, ToolRailItem};
///
/// let open = State::new(0);
/// let rail = ToolRail::new(open.binding(), [(IconName::Type, "Text"), (IconName::ZoomIn, "Zoom")]);
///
/// let tool = State::new(0);
/// let editor = ToolRail::new(tool.binding(), [(IconName::Pencil, "Brush")])
///     .separator()
///     .tool(
///         ToolRailItem::new(IconName::Square, "Rectangle")
///             .shortcut("M")
///             .variant(IconName::Circle, "Ellipse"),
///     )
///     .columns(2);
/// ```
#[derive(Debug)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct ToolRail {
    selected: ravenrs_reactive::Binding<usize>,
    entries: Vec<RailEntry>,
    width: Option<f32>,
    columns: usize,
    icons_only: bool,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl ToolRail {
    /// A rail of these tools; `selected` says which one is open.
    pub fn new<S: Into<String>>(
        selected: ravenrs_reactive::Binding<usize>,
        tools: impl IntoIterator<Item = (crate::IconName, S)>,
    ) -> Self {
        let entries =
            tools.into_iter().map(|(icon, name)| RailEntry::Tool(ToolRailItem::new(icon, name))).collect();
        Self { selected, entries, width: None, columns: 1, icons_only: false }
    }

    /// Adds a place with a tool and its variants.
    pub fn tool(mut self, item: ToolRailItem) -> Self {
        self.entries.push(RailEntry::Tool(item));
        self
    }

    /// Adds a line between two groups of tools.
    pub fn separator(mut self) -> Self {
        self.entries.push(RailEntry::Separator);
        self
    }

    /// Its width, in logical pixels (76 by default, or what the columns take).
    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }

    /// Shows only icons, with the names as tooltips.
    pub fn icons_only(mut self, icons_only: bool) -> Self {
        self.icons_only = icons_only;
        self
    }

    /// Lays the tools out in this many columns (1 by default). With more than one, it
    /// shows only icons.
    pub fn columns(mut self, columns: usize) -> Self {
        self.columns = columns.max(1);
        self
    }
}

struct RailTool<'a> {
    tools: &'a [RailToolInfo],
    /// Where this place's tools start, counting every tool of the rail.
    first: usize,
    selected: ravenrs_reactive::Binding<usize>,
    icons_only: bool,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl RailTool<'_> {
    /// The menu of the tools that share this place.
    fn variants(&self, shown: usize, last_used: &ravenrs_reactive::State<usize>) -> Vec<Entry> {
        self.tools
            .iter()
            .enumerate()
            .map(|(i, tool)| {
                let (selected, last_used, tool_at) =
                    (self.selected.clone(), last_used.clone(), self.first + i);
                let mut item = crate::MenuItem::new(tool.name.clone())
                    .icon(tool.icon)
                    .checked(i == shown)
                    .on_select(move || {
                        last_used.set(i);
                        selected.set(tool_at);
                    });
                if let Some(keys) = &tool.shortcut {
                    item = item.shortcut(keys.clone());
                }
                Entry::Item(item)
            })
            .collect()
    }
}

impl View for RailTool<'_> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let hovered = cx.is_hovered();
        let last_used = cx.use_state(|| 0_usize);
        let current = self.selected.get();
        let ours = current.checked_sub(self.first).filter(|i| *i < self.tools.len());
        // A place shows the tool in use, or the last one used.
        let shown = ours.unwrap_or_else(|| (*last_used.peek()).min(self.tools.len() - 1));
        let tool = &self.tools[shown];
        let has_variants = self.tools.len() > 1;
        let is_selected = ours.is_some();
        let ink = theme.color(if is_selected { Semantic::Ink } else { Semantic::InkMuted });
        let mut parts = vec![crate::util::icon(cx, tool.icon, 18.0, ink)];
        if !self.icons_only {
            parts.push(crate::util::label(cx, &tool.name, TextStyle::Caption, None, ink));
        }
        if has_variants {
            // A small mark in the corner says there is more here.
            parts.push(cx.container(
                LayoutStyle {
                    width: Some(4.0),
                    height: Some(4.0),
                    absolute: Some((
                        if self.icons_only { RAIL_ICON_BUTTON - 8.0 } else { 6.0 },
                        if self.icons_only { RAIL_ICON_BUTTON - 8.0 } else { 6.0 },
                    )),
                    ..LayoutStyle::leaf()
                },
                &[],
                Paint::Quad(QuadPaint { corner_radius: 2.0, ..QuadPaint::new(ink) }),
            ));
        }
        let fill = if is_selected {
            theme.color(Semantic::Accent).faded(0.2)
        } else if hovered {
            theme.color(Semantic::Ink).faded(0.05)
        } else {
            ravenrs_core::Color::TRANSPARENT
        };
        let style = if self.icons_only {
            LayoutStyle {
                width: Some(RAIL_ICON_BUTTON),
                height: Some(RAIL_ICON_BUTTON),
                align: Align::Center,
                justify: Justify::Center,
                ..LayoutStyle::stack(Axis::Vertical)
            }
        } else {
            LayoutStyle {
                height: Some(56.0),
                spacing: theme.spacing.s1,
                align: Align::Center,
                justify: Justify::Center,
                align_self: Some(Align::Stretch),
                ..LayoutStyle::stack(Axis::Vertical)
            }
        };
        let node = cx.container(
            style,
            &parts,
            Paint::Quad(QuadPaint { corner_radius: theme.radii.md, ..QuadPaint::new(fill) }),
        );
        let open: ravenrs_reactive::State<bool> = cx.use_state(|| false);
        let is_open = has_variants && open.get();
        let mut semantics = Semantics::new(Role::Button).name(tool.name.clone()).selected(is_selected);
        if has_variants {
            semantics = semantics.expanded(is_open);
        }
        cx.semantics(node, semantics);
        // A press that opened the variants does not also choose when it is released.
        let held = cx.use_state(|| Rc::new(Cell::new(None::<f64>))).get();
        let opened_by_hold = cx.use_state(|| Rc::new(Cell::new(false))).get();
        if has_variants {
            if cx.is_pressed() && !is_open {
                let now = cx.now();
                let due = held.get().unwrap_or(now + RAIL_HOLD);
                held.set(Some(due));
                if now >= due {
                    opened_by_hold.set(true);
                    open.set(true);
                } else {
                    cx.wake_at(due);
                }
            } else {
                held.set(None);
            }
        }
        let (selected, tool_at, swallow) =
            (self.selected.clone(), self.first + shown, opened_by_hold.clone());
        cx.on_click(
            node,
            Rc::new(move || {
                if !swallow.replace(false) {
                    selected.set(tool_at);
                }
            }),
        );
        cx.focusable(node, theme.radii.md);
        cx.cursor(node, ravenrs_core::CursorIcon::Pointer);
        if !has_variants {
            return node;
        }
        let at_right = open.clone();
        cx.on_key(
            node,
            Rc::new(move |key, _| {
                let opens = *key == ravenrs_core::Key::ArrowRight;
                if opens {
                    at_right.set(true);
                }
                opens
            }),
        );
        let on_menu = open.clone();
        cx.on_context_menu(node, Rc::new(move |_| on_menu.set(true)));
        let was_open = cx.use_state(|| Rc::new(Cell::new(false))).get();
        if is_open {
            was_open.set(true);
            let close: Rc<dyn Fn()> = Rc::new(move || open.set(false));
            let entries = self.variants(shown, &last_used);
            cx.overlay(
                node,
                crate::Placement::Right,
                &MenuList { entries: &entries, close: close.clone(), on_side: None },
                Some(close),
            );
        } else if was_open.replace(false) {
            cx.request_focus_node(node);
        }
        node
    }
}

impl View for ToolRail {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let icons_only = self.icons_only || self.columns > 1;
        let gap = 2.0;
        let padding = theme.spacing.s1;
        let columns = self.columns as f32;
        let width = self.width.unwrap_or(if icons_only {
            columns * RAIL_ICON_BUTTON + (columns - 1.0) * gap + padding * 2.0
        } else {
            76.0
        });
        let mut children = Vec::with_capacity(self.entries.len());
        let mut row = Vec::with_capacity(self.columns);
        let mut first = 0;
        let flush = |cx: &mut BuildCx<'_>, row: &mut Vec<NodeId>, children: &mut Vec<NodeId>| {
            if !row.is_empty() {
                children.push(cx.container(
                    LayoutStyle { spacing: gap, ..LayoutStyle::stack(Axis::Horizontal) },
                    row,
                    Paint::None,
                ));
                row.clear();
            }
        };
        for entry in &self.entries {
            match entry {
                RailEntry::Tool(item) => {
                    let place =
                        RailTool { tools: &item.tools, first, selected: self.selected.clone(), icons_only };
                    let name = item.tools.first().map_or("", |tool| tool.name.as_str());
                    let node = if icons_only {
                        cx.keyed_child(first as u64, &crate::Tooltip::new(place, name.to_owned()))
                    } else {
                        cx.keyed_child(first as u64, &place)
                    };
                    first += item.tools.len();
                    if self.columns > 1 {
                        row.push(node);
                        if row.len() == self.columns {
                            flush(cx, &mut row, &mut children);
                        }
                    } else {
                        children.push(node);
                    }
                }
                RailEntry::Separator => {
                    flush(cx, &mut row, &mut children);
                    children.push(cx.child(&Divider::horizontal()));
                }
            }
        }
        flush(cx, &mut row, &mut children);
        cx.container(
            LayoutStyle {
                width: Some(width),
                spacing: gap,
                padding: Insets::uniform(padding),
                align: Align::Stretch,
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &children,
            Paint::None,
        )
    }
}
