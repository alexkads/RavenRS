//! Navigation: tabs, sidebars, breadcrumbs and disclosures.

use std::cell::Cell;
use std::fmt;
use std::rc::Rc;

use ravenrs_core::{Border, Color, CursorIcon, Insets, Key, Quad, Rect, Role};
use ravenrs_layout::{Align, Axis, Justify, LayoutStyle};
use ravenrs_reactive::{Binding, State};
use ravenrs_style::{Semantic, TextStyle, Weight};

use crate::context::{BuildCx, NodeId, Paint, Placement, QuadPaint, Semantics, WidgetId};
use crate::floating::SheetLayer;
use crate::overlay::Bubble;
use crate::{
    AnyView, Badge, ButtonSize, Divider, IconButton, IconName, Motion, ScrollView, Side, Skeleton, Tooltip,
    View, ViewSeq, motion, util,
};

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

/// Width of a sidebar reduced to icons, in logical pixels. It keeps each icon
/// where it was when the sidebar is open: padding, row padding, icon, and back.
const SIDEBAR_RAIL: f32 = 60.0;

/// What a [`Sidebar`] does when it closes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum SidebarCollapse {
    /// It stays open; there is nothing to toggle.
    #[default]
    Fixed,
    /// It narrows to a rail of icons, with the labels as tooltips.
    Icons,
    /// It leaves the screen.
    Hidden,
}

/// How a [`Sidebar`] sits next to the page.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum SidebarVariant {
    /// A full-height column with a hairline toward the page.
    #[default]
    Standard,
    /// A rounded, raised panel away from the window's edges.
    Floating,
    /// No surface of its own; the page sits in a [`SidebarInset`] beside it.
    Inset,
}

/// Whether a [`Sidebar`] is open, shared with whatever toggles it.
///
/// A sidebar narrower windows show as a sheet keeps that apart: closing the
/// sheet does not change what the sidebar does when the window grows again.
#[derive(Clone)]
pub struct SidebarState {
    open: Binding<bool>,
    sheet: State<bool>,
    narrow: Rc<Cell<bool>>,
}

impl fmt::Debug for SidebarState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SidebarState").field("narrow", &self.narrow.get()).finish_non_exhaustive()
    }
}

impl SidebarState {
    /// A state that starts open or closed.
    pub fn new(open: bool) -> Self {
        Self::controlled(State::new(open).binding())
    }

    /// A state read from and written to `open`, for an app that saves or drives it.
    pub fn controlled(open: Binding<bool>) -> Self {
        Self { open, sheet: State::new(false), narrow: Rc::new(Cell::new(false)) }
    }

    /// Whether the sidebar is showing: its sheet in a narrow window, otherwise
    /// the column at full width.
    pub fn is_open(&self) -> bool {
        if self.narrow.get() { self.sheet.get() } else { self.open.get() }
    }

    /// Opens or closes it.
    pub fn set_open(&self, open: bool) {
        if self.narrow.get() {
            self.sheet.set(open);
        } else {
            self.open.set(open);
        }
    }

    /// Opens it if closed, closes it if open.
    pub fn toggle(&self) {
        self.set_open(!self.is_open());
    }

    /// The window is narrow enough for this sidebar to be a sheet; see
    /// [`Sidebar::narrow_below`]. Known once the sidebar has been built.
    pub fn is_narrow(&self) -> bool {
        self.narrow.get()
    }
}

/// What the parts of a sidebar need to know about the sidebar around them.
pub(crate) struct SidebarScope {
    icons: bool,
    in_sheet: bool,
    side: Side,
    state: SidebarState,
    focus_to: util::FocusTarget,
    next: Cell<usize>,
    count: Rc<Cell<usize>>,
}

/// Up, Down, Home and End move between a sidebar's rows; Right and Left open
/// and close a row that has content below it.
fn wire_row_keys(
    cx: &mut BuildCx<'_>,
    node: NodeId,
    scope: Option<&SidebarScope>,
    open: Option<Binding<bool>>,
) {
    let place = scope.map(|s| {
        let index = s.next.get();
        s.next.set(index + 1);
        if s.focus_to.take(index) {
            cx.request_focus_node(node);
        }
        (index, s.focus_to.clone(), s.count.clone())
    });
    if place.is_none() && open.is_none() {
        return;
    }
    cx.on_key(
        node,
        Rc::new(move |key, _| {
            if let Some(open) = &open {
                match key {
                    Key::ArrowRight if !open.get() => {
                        open.set(true);
                        return true;
                    }
                    Key::ArrowLeft if open.get() => {
                        open.set(false);
                        return true;
                    }
                    _ => {}
                }
            }
            let Some((index, focus_to, count)) = &place else { return false };
            let (index, count) = (*index, count.get().max(1));
            let target = match key {
                Key::ArrowDown => (index + 1) % count,
                Key::ArrowUp => (index + count - 1) % count,
                Key::Home => 0,
                Key::End => count - 1,
                _ => return false,
            };
            focus_to.request(target);
            true
        }),
    );
}

/// A navigation column: `Sidebar::new((SidebarSection::new("Workspace"), SidebarItem::new(...), ...))`.
///
/// Its children scroll between an optional header and footer. With
/// [`Self::collapsible`] it closes to icons or off the screen, by Cmd/Ctrl+B or
/// a [`SidebarTrigger`]. With [`Self::narrow_below`] small windows show it as a
/// sheet over the page.
///
/// ```
/// use ravenrs_widgets::*;
///
/// let state = SidebarState::new(true);
/// let sidebar = Sidebar::new((
///     SidebarGroup::new("Workspace", (
///         SidebarItem::new(IconName::House, "Home").selected(true),
///         SidebarItem::new(IconName::Users, "People").badge(Badge::new("3")),
///     )),
/// ))
/// .collapsible(SidebarCollapse::Icons)
/// .narrow_below(768.0)
/// .state(state.clone());
/// let trigger = SidebarTrigger::new(state);
/// # let _ = (sidebar, trigger);
/// ```
pub struct Sidebar<C, P = ()> {
    children: C,
    width: f32,
    collapse: SidebarCollapse,
    variant: SidebarVariant,
    side: Side,
    narrow_below: Option<f32>,
    shortcut: bool,
    label: Option<String>,
    parts: P,
}

/// What [`Sidebar::state`], [`Sidebar::header`] and [`Sidebar::footer`] add to a
/// sidebar. Its own type parameter, so a sidebar without them stays `Send`.
#[derive(Default)]
pub struct SidebarParts {
    state: Option<SidebarState>,
    header: Option<AnyView>,
    footer: Option<AnyView>,
}

impl fmt::Debug for SidebarParts {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SidebarParts").field("state", &self.state).finish_non_exhaustive()
    }
}

impl From<()> for SidebarParts {
    fn from((): ()) -> Self {
        Self::default()
    }
}

impl<C, P> fmt::Debug for Sidebar<C, P> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Sidebar").field("width", &self.width).finish_non_exhaustive()
    }
}

impl<C: ViewSeq> Sidebar<C> {
    /// A sidebar of `children`, usually sections and items.
    pub fn new(children: C) -> Self {
        Self {
            children,
            width: 248.0,
            collapse: SidebarCollapse::Fixed,
            variant: SidebarVariant::Standard,
            side: Side::Left,
            narrow_below: None,
            shortcut: true,
            label: None,
            parts: (),
        }
    }
}

impl<C: ViewSeq, P> Sidebar<C, P> {
    /// Width in logical pixels (default 248).
    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    /// What it does when it closes ([`SidebarCollapse::Fixed`] by default).
    /// Rows, groups and scroll position that are not bound to app state start
    /// over after [`SidebarCollapse::Hidden`], since nothing is built meanwhile.
    pub fn collapsible(mut self, collapse: SidebarCollapse) -> Self {
        self.collapse = collapse;
        self
    }

    /// How it sits next to the page ([`SidebarVariant::Standard`] by default).
    pub fn variant(mut self, variant: SidebarVariant) -> Self {
        self.variant = variant;
        self
    }

    /// The window edge it belongs to ([`Side::Left`] by default). Place it on
    /// that side of the page yourself; this sets its hairline, the side of its
    /// tooltips and where its sheet comes from.
    pub fn side(mut self, side: Side) -> Self {
        self.side = side;
        self
    }

    /// Below this window width, in logical pixels, it takes no room and opens
    /// as a sheet over the page. It goes by the window, not by its container.
    pub fn narrow_below(mut self, width: f32) -> Self {
        self.narrow_below = Some(width);
        self
    }

    /// Whether Cmd/Ctrl+B toggles it, if it can close (on by default). Turn it
    /// off when a window has more than one sidebar, or the app uses that key.
    pub fn shortcut(mut self, enabled: bool) -> Self {
        self.shortcut = enabled;
        self
    }

    /// The name screen readers announce for this landmark (the `navigation`
    /// word of [`Labels`](crate::Labels) by default).
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }
}

impl<C: ViewSeq, P: Into<SidebarParts>> Sidebar<C, P> {
    fn with_parts(self, change: impl FnOnce(&mut SidebarParts)) -> Sidebar<C, SidebarParts> {
        let mut parts = self.parts.into();
        change(&mut parts);
        Sidebar {
            children: self.children,
            width: self.width,
            collapse: self.collapse,
            variant: self.variant,
            side: self.side,
            narrow_below: self.narrow_below,
            shortcut: self.shortcut,
            label: self.label,
            parts,
        }
    }

    /// Shares whether it is open with a [`SidebarTrigger`] or the app. Without
    /// this it starts open and keeps the state itself.
    pub fn state(self, state: SidebarState) -> Sidebar<C, SidebarParts> {
        self.with_parts(|parts| parts.state = Some(state))
    }

    /// A view above the content that does not scroll. Reduced to icons, it
    /// keeps its layout and shows what fits, so start it with an icon.
    pub fn header(self, header: impl View + 'static) -> Sidebar<C, SidebarParts> {
        self.with_parts(|parts| parts.header = Some(Box::new(header)))
    }

    /// A view below the content that does not scroll, treated like the header.
    pub fn footer(self, footer: impl View + 'static) -> Sidebar<C, SidebarParts> {
        self.with_parts(|parts| parts.footer = Some(Box::new(footer)))
    }
}

/// A header or footer: laid out at full width, so closing only covers it, and
/// on the rail what fits between the rail's margins shows.
struct SidebarEnd<'a> {
    content: &'a dyn View,
    width: f32,
}

impl View for SidebarEnd<'_> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let content = cx.child(self.content);
        let inner = cx.container(
            LayoutStyle {
                width: Some((self.width - 24.0).max(0.0)),
                align: Align::Stretch,
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &[content],
            Paint::None,
        );
        let window = cx.container(
            LayoutStyle {
                grow: 1.0,
                min_width: Some(0.0),
                overflow_hidden: true,
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &[inner],
            Paint::None,
        );
        // Only on the rail: open, this edge would cut focus rings.
        if cx.sidebar().is_some_and(|s| s.icons) {
            cx.clip(window);
        }
        cx.container(
            LayoutStyle { padding: Insets::uniform(12.0), ..LayoutStyle::stack(Axis::Horizontal) },
            &[window],
            Paint::None,
        )
    }
}

/// Header, scrolling content and footer, and the scope their parts read.
struct SidebarColumn<'a, C> {
    children: &'a C,
    parts: Option<&'a SidebarParts>,
    scope: Rc<SidebarScope>,
    width: f32,
    /// The width the header and footer lay out at.
    full_width: f32,
    height: Option<f32>,
}

impl<C: ViewSeq> View for SidebarColumn<'_, C> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let (header, footer) = self.parts.map_or((None, None), |p| (p.header.as_ref(), p.footer.as_ref()));
        let outer = cx.replace_sidebar(Some(self.scope.clone()));
        let mut children = Vec::new();
        if let Some(header) = header {
            children.push(cx.child(&SidebarEnd { content: header.as_ref(), width: self.full_width }));
            children.push(cx.child(&Divider::horizontal()));
        }
        children.push(
            cx.child(&ScrollView::new(self.children).spacing(2.0).padding(Insets::symmetric(16.0, 12.0))),
        );
        if let Some(footer) = footer {
            children.push(cx.child(&Divider::horizontal()));
            children.push(cx.child(&SidebarEnd { content: footer.as_ref(), width: self.full_width }));
        }
        cx.replace_sidebar(outer);
        self.scope.count.set(self.scope.next.get());
        cx.container(
            LayoutStyle {
                width: Some(self.width),
                height: self.height,
                align: Align::Stretch,
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &children,
            Paint::None,
        )
    }
}

impl<C: ViewSeq> View for Sidebar<C> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        self.build_with(cx, None)
    }
}

impl<C: ViewSeq> View for Sidebar<C, SidebarParts> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        self.build_with(cx, Some(&self.parts))
    }
}

impl<C: ViewSeq, P> Sidebar<C, P> {
    fn build_with(&self, cx: &mut BuildCx<'_>, parts: Option<&SidebarParts>) -> NodeId {
        let theme = *cx.theme();
        let own = cx.use_state(|| SidebarState::new(true));
        let focus_to = util::roving_cell(cx, None);
        let count = cx.use_state(|| Rc::new(Cell::new(0_usize))).get();
        let state = parts.and_then(|p| p.state.clone()).unwrap_or_else(|| own.get());
        let viewport = cx.viewport();
        let narrow = self.narrow_below.is_some_and(|below| viewport.width < below);
        state.narrow.set(narrow);
        if !narrow && *state.sheet.peek() {
            state.sheet.set(false);
        }
        if self.shortcut && (narrow || self.collapse != SidebarCollapse::Fixed) {
            let toggle = state.clone();
            cx.shortcut(Key::Character("b".into()), true, Rc::new(move || toggle.toggle()));
        }
        let margin = if self.variant == SidebarVariant::Floating { theme.spacing.s2 } else { 0.0 };
        let open = state.open.get();
        let target = match self.collapse {
            _ if narrow => 0.0,
            SidebarCollapse::Fixed => self.width,
            _ if open => self.width,
            SidebarCollapse::Icons => SIDEBAR_RAIL + 2.0 * margin,
            SidebarCollapse::Hidden => 0.0,
        };
        // Reduced motion makes this instant. Labels stay until the column has
        // narrowed, and the column's edge uncovers them on the way back.
        let width = cx.animated(target, theme.motion.gentle);
        let scope = Rc::new(SidebarScope {
            icons: self.collapse == SidebarCollapse::Icons && !open && width <= target + 0.5,
            in_sheet: narrow,
            side: self.side,
            state: state.clone(),
            focus_to,
            next: Cell::new(0),
            count,
        });
        let label = self.label.clone().unwrap_or_else(|| cx.labels().navigation.clone());

        if narrow {
            let anchor = cx.empty();
            if state.sheet.get() {
                let panel = self.width.min(viewport.width);
                let column = SidebarColumn {
                    children: &self.children,
                    parts,
                    scope,
                    width: panel,
                    full_width: panel,
                    height: Some(viewport.height),
                };
                let layer = SheetLayer {
                    open: &state.sheet.binding(),
                    title: &label,
                    description: None,
                    content: Some(&column),
                    side: self.side,
                    width: panel,
                    bare: true,
                };
                let close = state.sheet.clone();
                cx.overlay(anchor, Placement::Fill, &layer, Some(Rc::new(move || close.set(false))));
            }
            return anchor;
        }
        if target == 0.0 && width <= 0.5 {
            return cx.empty();
        }

        let inset = margin.min(width / 2.0);
        let card_width = (width - 2.0 * inset).max(0.0);
        let full_width = (self.width - 2.0 * margin).max(0.0);
        let column = SidebarColumn {
            children: &self.children,
            parts,
            scope,
            // Off-screen closing uncovers a column of constant width; closing to
            // icons reflows it, so the icons can end up centered.
            width: if self.collapse == SidebarCollapse::Hidden { full_width } else { card_width },
            full_width,
            height: None,
        };
        let column = cx.child(&column);
        let (fill, border) = (theme.color(Semantic::Surface), theme.color(Semantic::Border));
        let side = self.side;
        let paint = match self.variant {
            SidebarVariant::Inset => Paint::None,
            SidebarVariant::Floating => Paint::Quad(QuadPaint {
                corner_radius: theme.radii.lg,
                border: Some(Border { width: 1.0, color: border }),
                shadow: Some(theme.elevation.soft),
                ..QuadPaint::new(fill)
            }),
            _ => Paint::Custom(Rc::new(move |f: Rect, scene| {
                let x = if side == Side::Left { f.max_x() - 1.0 } else { f.x() };
                scene.push_quad(Quad::new(f, fill));
                scene.push_quad(Quad::new(Rect::new(x, f.y(), 1.0, f.height()), border));
            })),
        };
        let card = cx.container(
            LayoutStyle {
                width: Some(card_width),
                align: Align::Stretch,
                align_self: Some(Align::Stretch),
                overflow_hidden: true,
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &[column],
            paint,
        );
        cx.clip(card);
        cx.semantics(card, Semantics::new(Role::Navigation).name(label));
        if margin == 0.0 {
            return card;
        }
        cx.container(
            LayoutStyle {
                width: Some(width),
                padding: Insets::uniform(inset),
                align: Align::Stretch,
                align_self: Some(Align::Stretch),
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &[card],
            Paint::None,
        )
    }
}

/// The page beside a [`SidebarVariant::Inset`] sidebar: a raised surface that
/// takes the remaining room.
#[derive(Debug)]
pub struct SidebarInset<V>(V);

impl<V: View> SidebarInset<V> {
    /// A page with this content.
    pub fn new(content: V) -> Self {
        Self(content)
    }
}

impl<V: View> View for SidebarInset<V> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let content = cx.child(&self.0);
        let page = cx.container(
            LayoutStyle {
                grow: 1.0,
                min_width: Some(0.0),
                align: Align::Stretch,
                overflow_hidden: true,
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &[content],
            Paint::Quad(QuadPaint {
                corner_radius: theme.radii.lg,
                border: Some(Border { width: 1.0, color: theme.color(Semantic::Border) }),
                shadow: Some(theme.elevation.soft),
                ..QuadPaint::new(theme.color(Semantic::Surface))
            }),
        );
        cx.container(
            LayoutStyle {
                grow: 1.0,
                min_width: Some(0.0),
                padding: Insets::uniform(theme.spacing.s2),
                align: Align::Stretch,
                align_self: Some(Align::Stretch),
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &[page],
            Paint::None,
        )
    }
}

/// A button that opens and closes a [`Sidebar`], sharing its [`SidebarState`].
#[derive(Clone, Debug)]
pub struct SidebarTrigger(SidebarState);

impl SidebarTrigger {
    /// A trigger for the sidebar that uses `state`.
    pub fn new(state: SidebarState) -> Self {
        Self(state)
    }
}

struct SidebarTriggerButton(SidebarState);

impl View for SidebarTriggerButton {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let state = self.0.clone();
        let button = IconButton::new(IconName::PanelLeft)
            .label(cx.labels().toggle_sidebar.clone())
            .size(ButtonSize::Small)
            .on_click(move || state.toggle());
        let node = button.build(cx);
        let open = self.0.is_open();
        cx.update_semantics(node, |s| s.expanded = Some(open));
        node
    }
}

impl View for SidebarTrigger {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let name = cx.labels().toggle_sidebar.clone();
        cx.child(&Tooltip::new(SidebarTriggerButton(self.0.clone()), name))
    }
}

/// A small caption that groups sidebar items. Reduced to icons, it is a hairline.
#[derive(Clone, Debug)]
pub struct SidebarSection(String);

impl SidebarSection {
    /// A section caption; shown in upper case.
    pub fn new(title: impl Into<String>) -> Self {
        Self(title.into())
    }
}

/// What stands for a caption when only icons are left.
fn rail_separator(cx: &mut BuildCx<'_>) -> NodeId {
    let line = cx.child(&Divider::horizontal());
    cx.container(
        LayoutStyle {
            padding: Insets::symmetric(8.0, 6.0),
            align: Align::Stretch,
            ..LayoutStyle::stack(Axis::Vertical)
        },
        &[line],
        Paint::None,
    )
}

impl View for SidebarSection {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        if let Some(scope) = cx.sidebar().filter(|s| s.icons) {
            return if scope.next.get() > 0 { rail_separator(cx) } else { cx.empty() };
        }
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

/// A titled set of sidebar items that can fold away and carry actions.
pub struct SidebarGroup<C> {
    title: String,
    children: C,
    collapsible: bool,
    open_by_default: bool,
    open: Option<Binding<bool>>,
    actions: Vec<AnyView>,
}

impl<C> fmt::Debug for SidebarGroup<C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SidebarGroup").field("title", &self.title).finish_non_exhaustive()
    }
}

impl<C: ViewSeq> SidebarGroup<C> {
    /// A group of `children` under this caption.
    pub fn new(title: impl Into<String>, children: C) -> Self {
        Self {
            title: title.into(),
            children,
            collapsible: false,
            open_by_default: true,
            open: None,
            actions: Vec::new(),
        }
    }

    /// Lets the caption fold the group. Reduced to icons, a group is always
    /// unfolded, because its caption is gone.
    pub fn collapsible(mut self) -> Self {
        self.collapsible = true;
        self
    }

    /// Whether a collapsible group starts unfolded (true by default).
    pub fn open(mut self, open: bool) -> Self {
        self.open_by_default = open;
        self
    }

    /// Reads and writes whether it is unfolded from the app; makes it collapsible.
    pub fn open_binding(mut self, open: Binding<bool>) -> Self {
        self.open = Some(open);
        self.collapsible = true;
        self
    }

    /// Adds a control at the end of the caption, usually a small [`IconButton`]
    /// with a label, or a [`DropdownMenu`](crate::DropdownMenu). Hidden when
    /// reduced to icons.
    pub fn action(mut self, action: impl View + 'static) -> Self {
        self.actions.push(Box::new(action));
        self
    }
}

struct SidebarGroupTitle<'a> {
    title: &'a str,
    open: Option<Binding<bool>>,
}

impl View for SidebarGroupTitle<'_> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let hovered = self.open.is_some() && cx.is_hovered();
        let color = theme.color(if hovered { Semantic::Ink } else { Semantic::InkMuted });
        let attrs = util::attrs(&theme, TextStyle::Caption, None);
        let text = cx.text_truncated(&self.title.to_uppercase(), &attrs, color, LayoutStyle::leaf());
        let text = cx.container(
            LayoutStyle { grow: 1.0, min_width: Some(0.0), ..LayoutStyle::stack(Axis::Horizontal) },
            &[text],
            Paint::None,
        );
        let mut children = vec![text];
        if let Some(open) = &self.open {
            let chevron = if open.get() { IconName::ChevronDown } else { IconName::ChevronRight };
            children.push(util::icon(cx, chevron, 14.0, color));
        }
        let node = cx.container(
            LayoutStyle {
                grow: 1.0,
                min_width: Some(0.0),
                padding: Insets::symmetric(6.0, 10.0),
                align: Align::Center,
                spacing: theme.spacing.s2,
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &children,
            Paint::Quad(QuadPaint {
                corner_radius: theme.radii.md,
                ..QuadPaint::new(if hovered {
                    theme.color(Semantic::Ink).with_alpha(0.05)
                } else {
                    Color::TRANSPARENT
                })
            }),
        );
        if let Some(open) = &self.open {
            motion::control(cx, node);
            cx.semantics(node, Semantics::new(Role::Button).name(self.title).expanded(open.get()));
            let toggle = open.clone();
            cx.on_click(node, Rc::new(move || toggle.update(|o| *o = !*o)));
            cx.focusable(node, theme.radii.md);
            cx.cursor(node, CursorIcon::Pointer);
            let scope = cx.sidebar();
            wire_row_keys(cx, node, scope.as_deref(), Some(open.clone()));
        } else {
            // The group around it already carries this name.
            cx.semantics(node, Semantics::hidden());
        }
        node
    }
}

struct SidebarRows<'a>(&'a dyn ViewSeq);

impl View for SidebarRows<'_> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let children = cx.children(self.0);
        cx.container(
            LayoutStyle { spacing: 2.0, align: Align::Stretch, ..LayoutStyle::stack(Axis::Vertical) },
            &children,
            Paint::None,
        )
    }
}

impl<C: ViewSeq> View for SidebarGroup<C> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let default = self.open_by_default;
        let own = cx.use_state(move || default);
        let open = self.open.clone().unwrap_or_else(|| own.binding());
        let scope = cx.sidebar();
        let icons = scope.as_ref().is_some_and(|s| s.icons);
        let mut children = Vec::new();
        if icons {
            // Nothing to separate from above the first rows.
            if scope.is_some_and(|s| s.next.get() > 0) {
                children.push(rail_separator(cx));
            }
        } else {
            let title = cx.child(&SidebarGroupTitle {
                title: &self.title,
                open: self.collapsible.then(|| open.clone()),
            });
            let mut parts = vec![title];
            parts.extend(self.actions.iter().map(|action| cx.child(action.as_ref())));
            children.push(cx.container(
                LayoutStyle {
                    padding: Insets { top: 10.0, ..Insets::ZERO },
                    align: Align::Center,
                    spacing: 2.0,
                    ..LayoutStyle::stack(Axis::Horizontal)
                },
                &parts,
                Paint::None,
            ));
        }
        let unfolded = icons || !self.collapsible || open.get();
        children.push(motion::collapse(cx, unfolded, &SidebarRows(&self.children)));
        let node = cx.container(
            LayoutStyle { spacing: 2.0, align: Align::Stretch, ..LayoutStyle::stack(Axis::Vertical) },
            &children,
            Paint::None,
        );
        cx.semantics(node, Semantics::new(Role::Group).name(self.title.clone()));
        node
    }
}

/// A sidebar row with an icon, a label, an optional count or badge, actions and
/// a submenu.
pub struct SidebarItem {
    icon: Option<IconName>,
    label: String,
    selected: bool,
    disabled: bool,
    count: Option<String>,
    badge: Option<Badge>,
    compact: bool,
    on_click: Option<Rc<dyn Fn()>>,
    actions: Vec<AnyView>,
    submenu: Vec<SidebarItem>,
    expanded_by_default: Option<bool>,
    expanded: Option<Binding<bool>>,
}

impl fmt::Debug for SidebarItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SidebarItem").field("label", &self.label).field("selected", &self.selected).finish()
    }
}

impl SidebarItem {
    /// An item with this icon and label.
    pub fn new(icon: IconName, label: impl Into<String>) -> Self {
        let mut item = Self::text(label);
        item.icon = Some(icon);
        item
    }

    /// An item with only a label, the usual entry of a submenu. Reduced to
    /// icons, its first letter stands in for the icon.
    pub fn text(label: impl Into<String>) -> Self {
        Self {
            icon: None,
            label: label.into(),
            selected: false,
            disabled: false,
            count: None,
            badge: None,
            compact: false,
            on_click: None,
            actions: Vec::new(),
            submenu: Vec::new(),
            expanded_by_default: None,
            expanded: None,
        }
    }

    /// Highlights the item as the current location.
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// Dims the item and takes it out of clicks and of keyboard focus.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// A count badge at the end of the row (e.g. unread items). Reduced to
    /// icons, a dot on the icon stands for it, and screen readers read it.
    pub fn count(mut self, count: impl Into<String>) -> Self {
        self.count = Some(count.into());
        self
    }

    /// A [`Badge`] at the end of the row, treated like [`Self::count`]: for a
    /// tone or a word such as "New".
    pub fn badge(mut self, badge: Badge) -> Self {
        self.badge = Some(badge);
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

    /// Adds a control at the end of the row, beside its button: usually a small
    /// [`IconButton`] with a label, or a [`DropdownMenu`](crate::DropdownMenu).
    /// Hidden when reduced to icons.
    pub fn action(mut self, action: impl View + 'static) -> Self {
        self.actions.push(Box::new(action));
        self
    }

    /// Adds a row to the submenu that unfolds under this one. Clicking this row
    /// folds and unfolds it; reduced to icons, clicking it opens the sidebar too.
    pub fn item(mut self, item: SidebarItem) -> Self {
        self.submenu.push(item);
        self
    }

    /// Whether the submenu starts unfolded. By default, only if it holds the
    /// selected item.
    pub fn expanded(mut self, expanded: bool) -> Self {
        self.expanded_by_default = Some(expanded);
        self
    }

    /// Reads and writes whether the submenu is unfolded from the app.
    pub fn expanded_binding(mut self, expanded: Binding<bool>) -> Self {
        self.expanded = Some(expanded);
        self
    }

    fn holds_selection(&self) -> bool {
        self.submenu.iter().any(|item| item.selected || item.holds_selection())
    }
}

/// The focusable part of a [`SidebarItem`].
struct SidebarButton<'a> {
    item: &'a SidebarItem,
    /// Whether the submenu is unfolded, for a row with one.
    expanded: Option<Binding<bool>>,
    /// Draws the row's fill itself: there is no action sharing it.
    filled: bool,
}

/// A row's fill, shared by its button and the box around its actions.
fn row_paint(cx: &BuildCx<'_>, selected: bool, hovered: bool) -> Paint {
    let theme = cx.theme();
    let fill = if selected {
        theme.color(Semantic::Control)
    } else if hovered {
        theme.color(Semantic::Ink).with_alpha(0.05)
    } else {
        Color::TRANSPARENT
    };
    Paint::Quad(QuadPaint { corner_radius: theme.radii.md, ..QuadPaint::new(fill) })
}

impl View for SidebarButton<'_> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let item = self.item;
        let scope = cx.sidebar();
        let icons = scope.as_ref().is_some_and(|s| s.icons);
        let hovered = cx.is_hovered() && !item.disabled;
        let (ink, muted) = (theme.color(Semantic::Ink), theme.color(Semantic::InkMuted));
        let dim = |color: Color| if item.disabled { color.with_alpha(0.5) } else { color };
        let icon_color =
            dim(if item.selected { theme.color(Semantic::Accent).lerp(ink, 0.35) } else { muted });
        let text_style = if item.compact { TextStyle::Callout } else { TextStyle::Body };
        let mut children = Vec::new();
        if let Some(icon) = item.icon {
            children.push(util::icon(cx, icon, 16.0, icon_color));
        } else if icons {
            let letter = item.label.chars().next().map(String::from).unwrap_or_default();
            children.push(util::label(cx, &letter, TextStyle::Callout, Some(Weight::SEMIBOLD), icon_color));
        }
        let note = item.count.as_deref().or(item.badge.as_ref().map(Badge::text));
        if icons {
            if note.is_some() {
                children.push(cx.container(
                    LayoutStyle {
                        width: Some(6.0),
                        height: Some(6.0),
                        absolute: Some((24.0, 6.0)),
                        ..LayoutStyle::leaf()
                    },
                    &[],
                    Paint::Quad(QuadPaint {
                        corner_radius: 3.0,
                        ..QuadPaint::new(theme.color(Semantic::Accent))
                    }),
                ));
            }
        } else {
            let attrs = util::attrs(
                &theme,
                text_style,
                Some(if item.selected { Weight::SEMIBOLD } else { Weight::REGULAR }),
            );
            let color = dim(if item.selected || hovered { ink } else { muted });
            let label = cx.text_truncated(&item.label, &attrs, color, LayoutStyle::leaf());
            children.push(cx.container(
                LayoutStyle { grow: 1.0, min_width: Some(0.0), ..LayoutStyle::stack(Axis::Horizontal) },
                &[label],
                Paint::None,
            ));
            if let Some(count) = &item.count {
                children.push(util::label(cx, count, TextStyle::Caption, None, dim(muted)));
            }
            if let Some(badge) = &item.badge {
                children.push(cx.child(badge));
            }
            if let Some(expanded) = &self.expanded {
                let chevron = if expanded.get() { IconName::ChevronDown } else { IconName::ChevronRight };
                children.push(util::icon(cx, chevron, 14.0, dim(muted)));
            }
        }
        let style = LayoutStyle {
            padding: if icons {
                Insets::ZERO
            } else {
                Insets::symmetric(0.0, if item.compact { theme.spacing.s2 } else { 10.0 })
            },
            width: icons.then_some(SIDEBAR_RAIL - 24.0),
            height: Some(if item.compact { 32.0 } else { 36.0 }),
            spacing: if item.compact { theme.spacing.s2 } else { 10.0 },
            align: Align::Center,
            justify: if icons { Justify::Center } else { Justify::Start },
            grow: if self.filled { 0.0 } else { 1.0 },
            min_width: Some(0.0),
            ..LayoutStyle::stack(Axis::Horizontal)
        };
        let paint = if self.filled { row_paint(cx, item.selected, hovered) } else { Paint::None };
        let node = cx.container(style, &children, paint);
        if self.filled {
            motion::control(cx, node);
        }
        let mut semantics = Semantics::new(Role::Button)
            .name(item.label.clone())
            .selected(item.selected)
            .disabled(item.disabled);
        if let Some(note) = note {
            semantics = semantics.description(note);
        }
        if let Some(expanded) = &self.expanded {
            semantics = semantics.expanded(expanded.get());
        }
        cx.semantics(node, semantics);
        cx.cursor(node, if item.disabled { CursorIcon::Default } else { CursorIcon::Pointer });
        if !item.disabled {
            let handler = item.on_click.clone();
            let expanded = self.expanded.clone();
            let state = scope.as_ref().filter(|s| icons || s.in_sheet).map(|s| s.state.clone());
            if handler.is_some() || expanded.is_some() || state.is_some() {
                cx.on_click(
                    node,
                    Rc::new(move || {
                        match (&expanded, &state) {
                            // Reduced to icons there is no room to unfold into.
                            (Some(expanded), Some(state)) if icons => {
                                expanded.set(true);
                                state.set_open(true);
                            }
                            (Some(expanded), _) => expanded.update(|e| *e = !*e),
                            // A sheet is in the way of where the item leads.
                            (None, Some(state)) if !icons => state.set_open(false),
                            _ => {}
                        }
                        if let Some(handler) = &handler {
                            handler();
                        }
                    }),
                );
            }
            cx.focusable(node, theme.radii.md);
            let keys = self.expanded.clone().filter(|_| !icons);
            wire_row_keys(cx, node, scope.as_deref(), keys);
        }
        if let Some(scope) = scope.as_ref().filter(|s| s.icons)
            && (cx.is_hovered() || cx.is_focus_visible())
        {
            let placement =
                if scope.side == Side::Left { Placement::RightCenter } else { Placement::LeftCenter };
            cx.overlay(node, placement, &Bubble(&item.label), None);
        }
        node
    }
}

/// The rows under a [`SidebarItem`], hung from a hairline below its icon.
struct SidebarSubmenu<'a>(&'a [SidebarItem]);

impl View for SidebarSubmenu<'_> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let border = cx.theme().color(Semantic::Border);
        let children: Vec<NodeId> = self.0.iter().map(|item| cx.child(item)).collect();
        let node = cx.container(
            LayoutStyle {
                padding: Insets { top: 2.0, left: 26.0, ..Insets::ZERO },
                spacing: 2.0,
                align: Align::Stretch,
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &children,
            Paint::Custom(Rc::new(move |f: Rect, scene| {
                let line = Rect::new(f.x() + 18.0, f.y() + 2.0, 1.0, (f.height() - 2.0).max(0.0));
                scene.push_quad(Quad::new(line, border));
            })),
        );
        cx.semantics(node, Semantics::new(Role::Group));
        node
    }
}

impl View for SidebarItem {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let default = self.expanded_by_default.unwrap_or_else(|| self.holds_selection());
        let own = cx.use_state(move || default);
        let expanded = self.expanded.clone().unwrap_or_else(|| own.binding());
        let icons = cx.sidebar().is_some_and(|s| s.icons);
        let has_submenu = !self.submenu.is_empty();
        let filled = icons || self.actions.is_empty();
        let button =
            cx.child(&SidebarButton { item: self, expanded: has_submenu.then(|| expanded.clone()), filled });
        let row = if filled {
            button
        } else {
            let mut parts = vec![button];
            parts.extend(self.actions.iter().map(|action| cx.child(action.as_ref())));
            let paint = row_paint(cx, self.selected, cx.is_hover_within() && !self.disabled);
            let row = cx.container(
                LayoutStyle {
                    padding: Insets { right: 2.0, ..Insets::ZERO },
                    align: Align::Center,
                    spacing: 2.0,
                    ..LayoutStyle::stack(Axis::Horizontal)
                },
                &parts,
                paint,
            );
            motion::control(cx, row);
            row
        };
        if !has_submenu || icons {
            return row;
        }
        let submenu = motion::collapse(cx, expanded.get(), &SidebarSubmenu(&self.submenu));
        cx.container(
            LayoutStyle { align: Align::Stretch, ..LayoutStyle::stack(Axis::Vertical) },
            &[row, submenu],
            Paint::None,
        )
    }
}

/// Placeholder rows while a sidebar's items load.
#[derive(Clone, Copy, Debug)]
pub struct SidebarSkeleton {
    rows: usize,
    icon: bool,
}

impl SidebarSkeleton {
    /// This many placeholder rows.
    pub fn new(rows: usize) -> Self {
        Self { rows, icon: true }
    }

    /// Whether the rows have a place for an icon (true by default).
    pub fn icon(mut self, icon: bool) -> Self {
        self.icon = icon;
        self
    }
}

impl View for SidebarSkeleton {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        // Uneven lengths read as text; the same ones on each build stay still.
        const LENGTHS: [f32; 4] = [128.0, 88.0, 152.0, 104.0];
        let icons = cx.sidebar().is_some_and(|s| s.icons);
        let mut rows = Vec::with_capacity(self.rows);
        for i in 0..self.rows {
            let mut parts = Vec::new();
            if self.icon || icons {
                parts.push(cx.child(&Skeleton::block(16.0, 16.0)));
            }
            if !icons {
                parts.push(cx.child(&Skeleton::block(LENGTHS[i % LENGTHS.len()], 12.0)));
            }
            rows.push(cx.container(
                LayoutStyle {
                    height: Some(36.0),
                    padding: Insets::symmetric(0.0, 10.0),
                    spacing: 10.0,
                    align: Align::Center,
                    overflow_hidden: true,
                    ..LayoutStyle::stack(Axis::Horizontal)
                },
                &parts,
                Paint::None,
            ));
        }
        let name = cx.labels().loading.clone();
        let node = cx.container(
            LayoutStyle { spacing: 2.0, align: Align::Stretch, ..LayoutStyle::stack(Axis::Vertical) },
            &rows,
            Paint::None,
        );
        cx.semantics(node, Semantics::new(Role::Group).name(name));
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
