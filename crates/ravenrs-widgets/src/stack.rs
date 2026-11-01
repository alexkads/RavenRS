use std::rc::Rc;

use ravenrs_core::{Insets, Role};
use ravenrs_layout::{Align, Axis, Justify, LayoutStyle};
use ravenrs_style::{Semantic, TextStyle};

use crate::context::{BuildCx, NodeId, Paint, QuadPaint, Semantics};
use crate::{Button, ButtonSize, ButtonStyle, Divider, Text, View, ViewSeq};

macro_rules! stack {
    ($(#[$doc:meta])* $name:ident, $axis:expr) => {
        $(#[$doc])*
        pub struct $name<C> {
            children: C,
            style: LayoutStyle,
        }

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

/// Flexible empty space that pushes its siblings apart inside a stack.
#[derive(Clone, Copy, Debug, Default)]
pub struct Spacer {
    min: f32,
}

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
/// thin scrollbar when they overflow.
pub struct ScrollView<C> {
    children: C,
    style: LayoutStyle,
    horizontal: bool,
}

impl<C> std::fmt::Debug for ScrollView<C> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ScrollView").field("style", &self.style).finish_non_exhaustive()
    }
}

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
        }
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
        if self.horizontal {
            cx.scroll_horizontally(node);
        }
        node
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

/// A narrow bar of tools with their names under their icons, of which one is
/// open: what sits along the side of an editor, with the open tool's panel next to it.
///
/// ```
/// use ravenrs_reactive::State;
/// use ravenrs_widgets::{IconName, ToolRail};
///
/// let open = State::new(0);
/// let rail = ToolRail::new(open.binding(), [(IconName::Type, "Text"), (IconName::ZoomIn, "Zoom")]);
/// ```
#[derive(Debug)]
pub struct ToolRail {
    selected: ravenrs_reactive::Binding<usize>,
    tools: Vec<(crate::IconName, String)>,
    width: f32,
}

impl ToolRail {
    /// A rail of these tools; `selected` says which one is open.
    pub fn new<S: Into<String>>(
        selected: ravenrs_reactive::Binding<usize>,
        tools: impl IntoIterator<Item = (crate::IconName, S)>,
    ) -> Self {
        let tools = tools.into_iter().map(|(icon, name)| (icon, name.into())).collect();
        Self { selected, tools, width: 76.0 }
    }

    /// Its width, in logical pixels (76 by default).
    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }
}

struct RailTool<'a> {
    icon: crate::IconName,
    name: &'a str,
    selected: bool,
    choose: Rc<dyn Fn()>,
}

impl View for RailTool<'_> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let hovered = cx.is_hovered();
        let ink = theme.color(if self.selected { Semantic::Ink } else { Semantic::InkMuted });
        let icon = crate::util::icon(cx, self.icon, 18.0, ink);
        let name = crate::util::label(cx, self.name, TextStyle::Caption, None, ink);
        let fill = if self.selected {
            theme.color(Semantic::Accent).faded(0.2)
        } else if hovered {
            theme.color(Semantic::Ink).faded(0.05)
        } else {
            ravenrs_core::Color::TRANSPARENT
        };
        let node = cx.container(
            LayoutStyle {
                height: Some(56.0),
                spacing: theme.spacing.s1,
                align: Align::Center,
                justify: Justify::Center,
                align_self: Some(Align::Stretch),
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &[icon, name],
            Paint::Quad(QuadPaint { corner_radius: theme.radii.md, ..QuadPaint::new(fill) }),
        );
        cx.semantics(node, Semantics::new(Role::Button).name(self.name.to_owned()).selected(self.selected));
        cx.on_click(node, self.choose.clone());
        cx.focusable(node, theme.radii.md);
        cx.cursor(node, ravenrs_core::CursorIcon::Pointer);
        node
    }
}

impl View for ToolRail {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let current = self.selected.get();
        let mut children = Vec::with_capacity(self.tools.len());
        for (index, (icon, name)) in self.tools.iter().enumerate() {
            let selected = self.selected.clone();
            children.push(cx.child(&RailTool {
                icon: *icon,
                name,
                selected: index == current,
                choose: Rc::new(move || selected.set(index)),
            }));
        }
        cx.container(
            LayoutStyle {
                width: Some(self.width),
                spacing: 2.0,
                padding: Insets::uniform(theme.spacing.s1),
                align: Align::Stretch,
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &children,
            Paint::None,
        )
    }
}
