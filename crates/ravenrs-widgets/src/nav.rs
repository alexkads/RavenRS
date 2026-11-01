//! Navigation: tabs, sidebars, breadcrumbs and disclosures.

use std::fmt;
use std::rc::Rc;

use ravenrs_core::{CursorIcon, Insets, Quad, Rect, Role};
use ravenrs_layout::{Align, Axis, LayoutStyle};
use ravenrs_reactive::Binding;
use ravenrs_style::{Semantic, TextStyle, Weight};

use crate::context::{BuildCx, NodeId, Paint, QuadPaint, Semantics, WidgetId};
use crate::{AnyView, IconName, Motion, View, ViewSeq, motion, util};

/// A tab bar with one content view per tab; only the selected one is built.
/// Arrow keys move between tabs and open them; Home and End jump to the ends.
pub struct Tabs {
    selected: Binding<usize>,
    tabs: Vec<(String, AnyView)>,
}

impl fmt::Debug for Tabs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Tabs").field("tabs", &self.tabs.iter().map(|t| &t.0).collect::<Vec<_>>()).finish()
    }
}

impl Tabs {
    /// Empty tabs; `selected` holds the zero-based index of the open tab.
    pub fn new(selected: Binding<usize>) -> Self {
        Self { selected, tabs: Vec::new() }
    }

    /// Adds a tab with this label and content.
    pub fn tab(mut self, label: impl Into<String>, content: impl View + 'static) -> Self {
        self.tabs.push((label.into(), Box::new(content)));
        self
    }
}

struct TabButton {
    scope: WidgetId,
    roving: util::Roving,
    label: String,
    selected: Binding<usize>,
}

impl View for TabButton {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let on = self.selected.get() == self.roving.index;
        let color =
            if on || cx.is_hovered() { theme.color(Semantic::Ink) } else { theme.color(Semantic::InkMuted) };
        let label = util::label(cx, &self.label, TextStyle::Body, Some(Weight::SEMIBOLD), color);
        motion::control(cx, label);
        let label = cx.container(
            LayoutStyle { padding: Insets::symmetric(0.0, 2.0), ..LayoutStyle::stack(Axis::Horizontal) },
            &[label],
            Paint::None,
        );
        // The underline is one object that slides from tab to tab.
        let marker = cx.container(
            LayoutStyle { height: Some(2.0), align_self: Some(Align::Stretch), ..LayoutStyle::leaf() },
            &[],
            if on {
                let accent = theme.color(Semantic::Accent);
                Paint::Quad(QuadPaint { corner_radius: 1.0, ..QuadPaint::new(accent) })
            } else {
                Paint::None
            },
        );
        if on {
            cx.motion(marker, Motion::new().shared_layout(self.scope, 0, theme.motion.snappy));
        }
        let style = LayoutStyle {
            padding: Insets { top: 8.0, ..Insets::ZERO },
            spacing: 10.0,
            ..LayoutStyle::stack(Axis::Vertical)
        };
        let node = cx.container(style, &[label, marker], Paint::None);
        cx.semantics(node, Semantics::new(Role::Tab).selected(on));
        let (selected, index) = (self.selected.clone(), self.roving.index);
        cx.on_click(node, Rc::new(move || selected.set(index)));
        // Tabs activate as focus moves (WAI-ARIA tabs, automatic activation).
        let selected = self.selected.clone();
        self.roving.wire(cx, node, Some(Rc::new(move |i| selected.set(i))));
        cx.focusable(node, 6.0);
        cx.cursor(node, CursorIcon::Pointer);
        node
    }
}

impl View for Tabs {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let border = cx.theme().color(Semantic::Border);
        let (count, focus_to) = (self.tabs.len(), util::roving_cell(cx, None));
        let scope = cx.widget_id();
        let buttons: Vec<NodeId> = self
            .tabs
            .iter()
            .enumerate()
            .map(|(index, (label, _))| {
                let roving = util::Roving { index, count, focus_to: focus_to.clone() };
                cx.child(&TabButton { scope, roving, label: label.clone(), selected: self.selected.clone() })
            })
            .collect();
        let bar = cx.container(
            LayoutStyle {
                spacing: 24.0,
                align_self: Some(Align::Stretch),
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &buttons,
            Paint::Custom(Rc::new(move |f: Rect, scene| {
                scene.push_quad(Quad::new(Rect::new(f.x(), f.max_y() - 1.0, f.width(), 1.0), border));
            })),
        );
        cx.semantics(bar, Semantics::new(Role::TabList));
        let selected = self.selected.get().min(self.tabs.len().saturating_sub(1));
        let mut children = vec![bar];
        if let Some((label, content)) = self.tabs.get(selected) {
            // Keyed by tab so each tab keeps its own local state.
            let content = cx.keyed_child(selected as u64, content.as_ref());
            // A box that fills the space, so content that grows still does.
            let panel = cx.container(
                LayoutStyle {
                    grow: 1.0,
                    min_height: Some(0.0),
                    align: Align::Stretch,
                    ..LayoutStyle::stack(Axis::Vertical)
                },
                &[content],
                Paint::None,
            );
            cx.semantics(panel, Semantics::new(Role::TabPanel).name(label.clone()));
            children.push(panel);
        }
        cx.container(
            LayoutStyle { spacing: 20.0, align: Align::Stretch, ..LayoutStyle::stack(Axis::Vertical) },
            &children,
            Paint::None,
        )
    }
}

/// A navigation column: `Sidebar::new((SidebarSection::new("Workspace"), SidebarItem::new(...), ...))`.
pub struct Sidebar<C> {
    children: C,
    width: f32,
}

impl<C> fmt::Debug for Sidebar<C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Sidebar").field("width", &self.width).finish_non_exhaustive()
    }
}

impl<C: ViewSeq> Sidebar<C> {
    /// A sidebar of `children`, usually sections and items.
    pub fn new(children: C) -> Self {
        Self { children, width: 248.0 }
    }

    /// Width in logical pixels (default 248).
    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }
}

impl<C: ViewSeq> View for Sidebar<C> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let children = cx.children(&self.children);
        let border = theme.color(Semantic::Border);
        let fill = theme.color(Semantic::Surface);
        let style = LayoutStyle {
            width: Some(self.width),
            padding: Insets::symmetric(16.0, 12.0),
            spacing: 2.0,
            align: Align::Stretch,
            align_self: Some(Align::Stretch),
            overflow_hidden: true,
            ..LayoutStyle::stack(Axis::Vertical)
        };
        let node = cx.container(
            style,
            &children,
            Paint::Custom(Rc::new(move |f: Rect, scene| {
                scene.push_quad(Quad::new(f, fill));
                scene.push_quad(Quad::new(Rect::new(f.max_x() - 1.0, f.y(), 1.0, f.height()), border));
            })),
        );
        cx.scrollable(node);
        node
    }
}

/// A small caption that groups sidebar items.
#[derive(Clone, Debug)]
pub struct SidebarSection(String);

impl SidebarSection {
    /// A section caption; shown in upper case.
    pub fn new(title: impl Into<String>) -> Self {
        Self(title.into())
    }
}

impl View for SidebarSection {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let color = cx.theme().color(Semantic::InkMuted);
        let text = util::label(cx, &self.0.to_uppercase(), TextStyle::Caption, None, color);
        cx.container(
            LayoutStyle {
                padding: Insets { top: 16.0, bottom: 6.0, left: 10.0, right: 10.0 },
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &[text],
            Paint::None,
        )
    }
}

/// A sidebar row with an icon, a label and an optional count.
pub struct SidebarItem {
    icon: IconName,
    label: String,
    selected: bool,
    count: Option<String>,
    compact: bool,
    on_click: Option<Rc<dyn Fn()>>,
}

impl fmt::Debug for SidebarItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SidebarItem").field("label", &self.label).field("selected", &self.selected).finish()
    }
}

impl SidebarItem {
    /// An item with this icon and label.
    pub fn new(icon: IconName, label: impl Into<String>) -> Self {
        Self { icon, label: label.into(), selected: false, count: None, compact: false, on_click: None }
    }

    /// Highlights the item as the current location.
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// A count badge at the end of the row (e.g. unread items).
    pub fn count(mut self, count: impl Into<String>) -> Self {
        self.count = Some(count.into());
        self
    }

    /// A denser row (32 px, `callout` text) for panels with many items.
    pub fn compact(mut self) -> Self {
        self.compact = true;
        self
    }

    /// Runs `handler` when the item is clicked.
    pub fn on_click(mut self, handler: impl Fn() + 'static) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }
}

impl View for SidebarItem {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let hovered = cx.is_hovered();
        let (ink, muted) = (theme.color(Semantic::Ink), theme.color(Semantic::InkMuted));
        let icon_color = if self.selected { theme.color(Semantic::Accent).lerp(ink, 0.35) } else { muted };
        let icon = util::icon(cx, self.icon, 16.0, icon_color);
        let text_style = if self.compact { TextStyle::Callout } else { TextStyle::Body };
        let attrs = util::attrs(
            &theme,
            text_style,
            Some(if self.selected { Weight::SEMIBOLD } else { Weight::REGULAR }),
        );
        let label = cx.text_truncated(
            &self.label,
            &attrs,
            if self.selected || hovered { ink } else { muted },
            LayoutStyle::leaf(),
        );
        let label = cx.container(
            LayoutStyle { grow: 1.0, min_width: Some(0.0), ..LayoutStyle::stack(Axis::Horizontal) },
            &[label],
            Paint::None,
        );
        let mut children = vec![icon, label];
        if let Some(count) = &self.count {
            children.push(util::label(cx, count, TextStyle::Caption, None, muted));
        }
        let fill = if self.selected {
            theme.color(Semantic::Control)
        } else if hovered {
            ink.with_alpha(0.05)
        } else {
            ravenrs_core::Color::TRANSPARENT
        };
        let style = LayoutStyle {
            padding: Insets::symmetric(0.0, if self.compact { theme.spacing.s2 } else { 10.0 }),
            height: Some(if self.compact { 32.0 } else { 36.0 }),
            spacing: if self.compact { theme.spacing.s2 } else { 10.0 },
            align: Align::Center,
            ..LayoutStyle::stack(Axis::Horizontal)
        };
        let paint = Paint::Quad(QuadPaint { corner_radius: theme.radii.md, ..QuadPaint::new(fill) });
        let node = cx.container(style, &children, paint);
        motion::control(cx, node);
        cx.semantics(node, Semantics::new(Role::Button).selected(self.selected));
        if let Some(h) = &self.on_click {
            cx.on_click(node, h.clone());
        }
        cx.focusable(node, theme.radii.md);
        cx.cursor(node, CursorIcon::Pointer);
        node
    }
}

/// A path of locations, the last one being the current page.
#[derive(Clone, Debug)]
pub struct Breadcrumb(Vec<String>);

impl Breadcrumb {
    /// A breadcrumb of `items`, from the root to the current page.
    pub fn new(items: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self(items.into_iter().map(Into::into).collect())
    }
}

impl View for Breadcrumb {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let mut children = Vec::new();
        for (i, item) in self.0.iter().enumerate() {
            let last = i + 1 == self.0.len();
            if i > 0 {
                children.push(util::icon(cx, IconName::ChevronRight, 14.0, theme.color(Semantic::InkMuted)));
            }
            let color = if last { theme.color(Semantic::Ink) } else { theme.color(Semantic::InkMuted) };
            let weight = if last { Some(Weight::SEMIBOLD) } else { None };
            children.push(util::label(cx, item, TextStyle::Callout, weight, color));
        }
        let node = cx.container(
            LayoutStyle { spacing: 6.0, align: Align::Center, ..LayoutStyle::stack(Axis::Horizontal) },
            &children,
            Paint::None,
        );
        cx.semantics(node, Semantics::new(Role::Group).name("Breadcrumb"));
        node
    }
}

/// A header that shows or hides its content. Remembers whether it is open.
pub struct Disclosure {
    title: String,
    content: AnyView,
    open_by_default: bool,
}

impl fmt::Debug for Disclosure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Disclosure").field("title", &self.title).finish_non_exhaustive()
    }
}

impl Disclosure {
    /// A collapsed disclosure with this title and content.
    pub fn new(title: impl Into<String>, content: impl View + 'static) -> Self {
        Self { title: title.into(), content: Box::new(content), open_by_default: false }
    }

    /// Whether it starts open (closed by default).
    pub fn open(mut self, open: bool) -> Self {
        self.open_by_default = open;
        self
    }
}

struct DisclosureBody<'a>(&'a dyn View);

impl View for DisclosureBody<'_> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let content = cx.child(self.0);
        cx.container(
            LayoutStyle {
                padding: Insets { top: 2.0, bottom: 10.0, left: 32.0, right: 8.0 },
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &[content],
            Paint::None,
        )
    }
}

impl View for Disclosure {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let default = self.open_by_default;
        let open = cx.use_state(move || default);
        let is_open = open.get();
        let hovered = cx.is_hovered();
        let chevron = if is_open { IconName::ChevronDown } else { IconName::ChevronRight };
        let icon = util::icon(cx, chevron, 16.0, theme.color(Semantic::InkMuted));
        let title =
            util::label(cx, &self.title, TextStyle::Body, Some(Weight::SEMIBOLD), theme.color(Semantic::Ink));
        let header_fill = if hovered {
            theme.color(Semantic::Ink).with_alpha(0.04)
        } else {
            ravenrs_core::Color::TRANSPARENT
        };
        let header = cx.container(
            LayoutStyle {
                padding: Insets::symmetric(10.0, 8.0),
                spacing: 8.0,
                align: Align::Center,
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &[icon, title],
            Paint::Quad(QuadPaint { corner_radius: 8.0, ..QuadPaint::new(header_fill) }),
        );
        motion::control(cx, header);
        cx.semantics(header, Semantics::new(Role::Button).expanded(is_open));
        let toggle = open.clone();
        cx.on_click(header, Rc::new(move || toggle.update(|o| *o = !*o)));
        cx.focusable(header, 8.0);
        cx.cursor(header, CursorIcon::Pointer);
        let body = motion::collapse(cx, is_open, &DisclosureBody(self.content.as_ref()));
        cx.container(
            LayoutStyle { align: Align::Stretch, ..LayoutStyle::stack(Axis::Vertical) },
            &[header, body],
            Paint::None,
        )
    }
}

/// Disclosures separated by hairlines.
#[derive(Debug)]
pub struct Accordion(Vec<Disclosure>);

impl Accordion {
    /// An accordion of these disclosures.
    pub fn new(items: Vec<Disclosure>) -> Self {
        Self(items)
    }
}

impl View for Accordion {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let mut children = Vec::new();
        for (i, item) in self.0.iter().enumerate() {
            if i > 0 {
                children.push(cx.child(&crate::Divider::horizontal()));
            }
            children.push(cx.keyed_child(i as u64, item));
        }
        cx.container(
            LayoutStyle { spacing: 2.0, align: Align::Stretch, ..LayoutStyle::stack(Axis::Vertical) },
            &children,
            Paint::None,
        )
    }
}
