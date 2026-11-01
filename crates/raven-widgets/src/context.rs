use std::any::Any;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::rc::Rc;

use raven_core::{Border, Brush, Color, CursorIcon, Key, Modifiers, Point, Rect, Scene, Shadow, Size};
use raven_layout::{LayoutError, LayoutId, LayoutStyle, LayoutTree};
use raven_reactive::State;
use raven_style::Theme;
use raven_text::{TextAttrs, TextLayout, TextSystem};

use crate::View;
use crate::view::ViewSeq;

/// Stable identity of a widget across rebuilds, derived from its position in the
/// view tree (or from an explicit key, see [`BuildCx::keyed_child`]). Hover,
/// press and focus are remembered per `WidgetId`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct WidgetId(u64);

impl WidgetId {
    pub(crate) const ROOT: Self = Self(0x5241_5645_4e52_5321);

    fn child(self, key: u64) -> Self {
        let mut h = self.0 ^ key.wrapping_mul(0x9e37_79b9_7f4a_7c15);
        h = (h ^ (h >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        h = (h ^ (h >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        Self(h ^ (h >> 31))
    }
}

/// A node of the element tree produced by one build. Only valid during the
/// build that created it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NodeId(pub(crate) u32);

/// A rounded rectangle painted behind an element's children.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct QuadPaint {
    pub fill: Brush,
    pub corner_radius: f32,
    pub border: Option<Border>,
    pub shadow: Option<Shadow>,
}

impl QuadPaint {
    pub fn new(fill: impl Into<Brush>) -> Self {
        Self { fill: fill.into(), corner_radius: 0.0, border: None, shadow: None }
    }
}

/// Custom drawing callback: receives the element's absolute frame in logical pixels.
pub type DrawFn = Rc<dyn Fn(Rect, &mut Scene)>;

/// What an element draws before its children.
#[derive(Clone, Default)]
pub enum Paint {
    #[default]
    None,
    Quad(QuadPaint),
    /// Arbitrary drawing into the scene, given the element's absolute frame.
    Custom(DrawFn),
}

impl fmt::Debug for Paint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::None => f.write_str("None"),
            Self::Quad(q) => f.debug_tuple("Quad").field(q).finish(),
            Self::Custom(_) => f.write_str("Custom"),
        }
    }
}

/// Called with the focused element's key presses; returns true when it handled the key.
pub type KeyHandler = Rc<dyn Fn(&Key, Modifiers) -> bool>;

/// Phase of a pointer drag that started on an element.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DragPhase {
    Start,
    Move,
    End,
}

/// A pointer drag on an element, in absolute logical pixels, with the element's frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Drag {
    pub phase: DragPhase,
    pub position: Point,
    pub frame: Rect,
    /// The frame of the dragged element's parent (e.g. a splitter's container).
    pub container: Rect,
}

impl Drag {
    /// Horizontal position inside the frame, from 0.0 (left) to 1.0 (right).
    pub fn fraction_x(&self) -> f32 {
        if self.frame.width() <= 0.0 {
            return 0.0;
        }
        ((self.position.x - self.frame.x()) / self.frame.width()).clamp(0.0, 1.0)
    }
}

pub type DragHandler = Rc<dyn Fn(Drag)>;

/// Where an overlay is placed relative to its anchor.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Placement {
    /// Below the anchor, left edges aligned.
    Below,
    /// Above the anchor, left edges aligned.
    Above,
    /// To the right of the anchor, top edges aligned.
    Right,
    /// Centered above the anchor (tooltips).
    AboveCenter,
    /// Covers the whole viewport (dialogs, sheets); the anchor is ignored.
    Fill,
    /// At a point in the viewport (context menus); the anchor is ignored.
    At(Point),
    /// The bottom-right corner of the viewport (toasts).
    BottomRight,
}

/// A keyboard shortcut that works wherever focus is (unless the focused
/// element handles the key first). `command` means Cmd on macOS, Ctrl elsewhere;
/// both are accepted on every platform.
#[derive(Clone)]
pub(crate) struct Shortcut {
    pub key: Key,
    pub command: bool,
    pub handler: Rc<dyn Fn()>,
}

pub(crate) struct Overlay {
    pub root: NodeId,
    pub anchor: NodeId,
    pub placement: Placement,
    pub gap: f32,
    pub on_dismiss: Option<Rc<dyn Fn()>>,
}

pub(crate) struct Element {
    pub widget: WidgetId,
    pub layout: LayoutId,
    pub children: Vec<NodeId>,
    pub paint: Paint,
    /// Color of the text this element shows, if it is a text leaf.
    pub text_color: Option<Color>,
    pub on_click: Option<Rc<dyn Fn()>>,
    /// Corner radius of the focus ring; `Some` makes the element focusable.
    pub focus_radius: Option<f32>,
    pub cursor: Option<CursorIcon>,
    pub on_key: Option<KeyHandler>,
    pub on_drag: Option<DragHandler>,
    /// Children are clipped to the frame.
    pub clip: bool,
    /// Children scroll vertically inside the frame (implies `clip`).
    pub scroll: bool,
    /// Children also scroll horizontally.
    pub scroll_x: bool,
    /// Scrolling rebuilds the UI (virtualized content that depends on the offset).
    pub rebuild_on_scroll: bool,
    /// Visual offset applied to the children's positions.
    pub translate: Point,
    pub on_double_click: Option<Rc<dyn Fn()>>,
    pub on_context_menu: Option<Rc<dyn Fn(Point)>>,
}

impl Element {
    pub(crate) fn new(widget: WidgetId, layout: LayoutId) -> Self {
        Self {
            widget,
            layout,
            children: Vec::new(),
            paint: Paint::None,
            text_color: None,
            on_click: None,
            focus_radius: None,
            cursor: None,
            on_key: None,
            on_drag: None,
            clip: false,
            scroll: false,
            scroll_x: false,
            rebuild_on_scroll: false,
            translate: Point::ZERO,
            on_double_click: None,
            on_context_menu: None,
        }
    }

    pub fn is_interactive(&self) -> bool {
        self.on_click.is_some()
            || self.on_double_click.is_some()
            || self.on_context_menu.is_some()
            || self.cursor.is_some()
            || self.focus_radius.is_some()
            || self.on_drag.is_some()
    }
}

/// Layout context of measured leaves.
pub(crate) struct TextLeaf {
    pub layout: TextLayout,
}

/// Which widgets the pointer and keyboard are interacting with.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Interaction {
    pub hovered: Option<WidgetId>,
    pub pressed: Option<WidgetId>,
    pub focused: Option<WidgetId>,
    /// Focus came from the keyboard, so the ring is shown.
    pub focus_visible: bool,
}

struct Scope {
    id: WidgetId,
    next_child: u64,
    next_state: u32,
}

/// Scroll positions and visible sizes of scroll containers, by widget.
#[derive(Debug, Default)]
pub(crate) struct ScrollState {
    pub offsets: HashMap<WidgetId, Point>,
    /// Frame size of each scroll container in the last layout.
    pub viewports: HashMap<WidgetId, Size>,
}

/// Widget-local state that survives rebuilds, keyed by widget identity and
/// call order (see [`BuildCx::use_state`]). Slots of widgets that were not
/// built in the latest build are dropped.
#[derive(Default)]
pub(crate) struct LocalStore {
    slots: HashMap<(WidgetId, u32), Box<dyn Any>>,
    used: HashSet<(WidgetId, u32)>,
}

impl LocalStore {
    pub fn begin_build(&mut self) {
        self.used.clear();
    }

    pub fn end_build(&mut self) {
        let used = &self.used;
        self.slots.retain(|k, _| used.contains(k));
    }
}

/// What a [`View`] uses to describe itself during a build: theme access,
/// interaction state, child building, layout boxes, painting and behavior.
pub struct BuildCx<'a> {
    pub(crate) theme: Theme,
    pub(crate) theme_state: State<Theme>,
    pub(crate) viewport: Size,
    pub(crate) overlays: Vec<Overlay>,
    pub(crate) text: &'a mut TextSystem,
    pub(crate) layout: &'a mut LayoutTree<TextLeaf>,
    pub(crate) elements: &'a mut Vec<Element>,
    pub(crate) interaction: Interaction,
    pub(crate) hover_chain: &'a [WidgetId],
    pub(crate) scroll: &'a ScrollState,
    pub(crate) scroll_requests: Vec<(WidgetId, Point)>,
    pub(crate) focus_request: Option<WidgetId>,
    pub(crate) now: f64,
    pub(crate) shortcuts: Vec<Shortcut>,
    pub(crate) wake: Option<f64>,
    pub(crate) modifiers: Rc<std::cell::Cell<Modifiers>>,
    pub(crate) local: &'a mut LocalStore,
    pub(crate) placeholder: LayoutId,
    pub(crate) error: Option<LayoutError>,
    scopes: Vec<Scope>,
}

impl fmt::Debug for BuildCx<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BuildCx").field("widget", &self.widget_id()).finish_non_exhaustive()
    }
}

impl<'a> BuildCx<'a> {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        theme_state: State<Theme>,
        viewport: Size,
        text: &'a mut TextSystem,
        layout: &'a mut LayoutTree<TextLeaf>,
        elements: &'a mut Vec<Element>,
        interaction: Interaction,
        hover_chain: &'a [WidgetId],
        scroll: &'a ScrollState,
        modifiers: Rc<std::cell::Cell<Modifiers>>,
        local: &'a mut LocalStore,
        placeholder: LayoutId,
    ) -> Self {
        Self {
            // Read through the state so a theme change rebuilds the UI.
            theme: theme_state.get(),
            theme_state,
            viewport,
            overlays: Vec::new(),
            text,
            layout,
            elements,
            interaction,
            hover_chain,
            scroll,
            scroll_requests: Vec::new(),
            focus_request: None,
            now: 0.0,
            shortcuts: Vec::new(),
            wake: None,
            modifiers,
            local,
            placeholder,
            error: None,
            scopes: vec![Scope { id: WidgetId::ROOT, next_child: 0, next_state: 0 }],
        }
    }

    pub fn theme(&self) -> &Theme {
        &self.theme
    }

    /// The app-wide theme as state: set it to switch themes at runtime.
    pub fn theme_state(&self) -> State<Theme> {
        self.theme_state.clone()
    }

    /// The viewport size in logical pixels.
    pub fn viewport(&self) -> Size {
        self.viewport
    }

    /// The identity of the view being built.
    pub fn widget_id(&self) -> WidgetId {
        self.scopes.last().map_or(WidgetId::ROOT, |s| s.id)
    }

    /// The pointer is over this view's interactive element.
    pub fn is_hovered(&self) -> bool {
        self.interaction.hovered == Some(self.widget_id())
    }

    /// The pointer is over this view or over any interactive element inside it.
    pub fn is_hover_within(&self) -> bool {
        self.hover_chain.contains(&self.widget_id())
    }

    /// State owned by this view instance that survives rebuilds, like SwiftUI's
    /// `@State`. Call it unconditionally and in the same order on every build;
    /// `init` runs only the first time. The state is dropped when the view stops
    /// being built.
    pub fn use_state<T: 'static>(&mut self, init: impl FnOnce() -> T) -> State<T> {
        let id = self.widget_id();
        let slot = match self.scopes.last_mut() {
            Some(scope) => {
                scope.next_state += 1;
                scope.next_state
            }
            None => 0,
        };
        let key = (id, slot);
        self.local.used.insert(key);
        if let Some(state) = self.local.slots.get(&key).and_then(|b| b.downcast_ref::<State<T>>()) {
            return state.clone();
        }
        let state = State::new(init());
        self.local.slots.insert(key, Box::new(state.clone()));
        state
    }

    /// The primary button went down on this view and has not been released.
    pub fn is_pressed(&self) -> bool {
        self.interaction.pressed == Some(self.widget_id())
    }

    pub fn is_focused(&self) -> bool {
        self.interaction.focused == Some(self.widget_id())
    }

    /// Focus is on this view and came from the keyboard.
    pub fn is_focus_visible(&self) -> bool {
        self.is_focused() && self.interaction.focus_visible
    }

    /// Builds a child view; its identity comes from its position among this
    /// view's children.
    pub fn child(&mut self, view: &dyn View) -> NodeId {
        let key = match self.scopes.last_mut() {
            Some(scope) => {
                scope.next_child += 1;
                scope.next_child
            }
            None => 0,
        };
        self.build_scoped(key, view)
    }

    /// Builds a child with an explicit key, so it keeps its hover/focus state
    /// when siblings are inserted or removed (use it for dynamic lists).
    pub fn keyed_child(&mut self, key: u64, view: &dyn View) -> NodeId {
        // Keep keyed and positional identities apart.
        self.build_scoped(key ^ (1 << 63), view)
    }

    fn build_scoped(&mut self, key: u64, view: &dyn View) -> NodeId {
        let id = self.widget_id().child(key);
        self.scopes.push(Scope { id, next_child: 0, next_state: 0 });
        let node = view.build(self);
        self.scopes.pop();
        node
    }

    /// Builds every view of a sequence as children.
    pub fn children(&mut self, seq: &dyn ViewSeq) -> Vec<NodeId> {
        let mut out = Vec::new();
        seq.build_each(self, &mut out);
        out
    }

    /// Creates a box that lays out `children` and paints `paint` behind them.
    pub fn container(&mut self, style: LayoutStyle, children: &[NodeId], paint: Paint) -> NodeId {
        let child_layouts: Vec<LayoutId> =
            children.iter().filter_map(|c| self.elements.get(c.0 as usize).map(|e| e.layout)).collect();
        let layout = match self.layout.node(style, &child_layouts) {
            Ok(id) => id,
            Err(e) => self.fail(e),
        };
        let mut element = Element::new(self.widget_id(), layout);
        element.children = children.to_vec();
        element.paint = paint;
        self.push(element)
    }

    /// Creates a text leaf that wraps to the width its parent offers.
    pub fn text(&mut self, text: &str, attrs: &TextAttrs, color: Color, style: LayoutStyle) -> NodeId {
        let layout = self.text.layout(text, attrs);
        let id = match self.layout.measured_leaf(style, TextLeaf { layout }) {
            Ok(id) => id,
            Err(e) => self.fail(e),
        };
        let mut element = Element::new(self.widget_id(), id);
        element.text_color = Some(color);
        self.push(element)
    }

    /// Measures `text` without creating a node, e.g. to place a caret.
    pub fn measure_text(&mut self, text: &str, attrs: &TextAttrs) -> Size {
        self.text.layout(text, attrs).measure(None)
    }

    /// An empty box that takes no space.
    pub fn empty(&mut self) -> NodeId {
        self.container(LayoutStyle::leaf(), &[], Paint::None)
    }

    /// Calls `handler` when the node is clicked, or activated with Enter/Space while focused.
    pub fn on_click(&mut self, node: NodeId, handler: Rc<dyn Fn()>) {
        if let Some(e) = self.element_mut(node) {
            e.on_click = Some(handler);
        }
    }

    /// Makes the node reachable with Tab; the focus ring uses `corner_radius`.
    pub fn focusable(&mut self, node: NodeId, corner_radius: f32) {
        if let Some(e) = self.element_mut(node) {
            e.focus_radius = Some(corner_radius);
        }
    }

    /// Shows `icon` while the pointer is over the node, and makes it hoverable.
    pub fn cursor(&mut self, node: NodeId, icon: CursorIcon) {
        if let Some(e) = self.element_mut(node) {
            e.cursor = Some(icon);
        }
    }

    /// Receives key presses while the node is focused (make it [`Self::focusable`]).
    /// Return true from the handler to stop the default handling (Tab, Enter...).
    pub fn on_key(&mut self, node: NodeId, handler: KeyHandler) {
        if let Some(e) = self.element_mut(node) {
            e.on_key = Some(handler);
        }
    }

    /// Receives pointer drags that start on the node (sliders, splitters...).
    pub fn on_drag(&mut self, node: NodeId, handler: DragHandler) {
        if let Some(e) = self.element_mut(node) {
            e.on_drag = Some(handler);
        }
    }

    /// Clips the node's children to its frame.
    pub fn clip(&mut self, node: NodeId) {
        if let Some(e) = self.element_mut(node) {
            e.clip = true;
        }
    }

    /// This widget's scroll offset (for its scrollable element), as of this build.
    pub fn scroll_offset(&self) -> Point {
        self.scroll.offsets.get(&self.widget_id()).copied().unwrap_or(Point::ZERO)
    }

    /// The visible size of this widget's scroll container in the last layout,
    /// or `None` before its first layout. Virtualized views use it with
    /// [`Self::scroll_offset`] to build only the visible rows.
    pub fn scroll_viewport(&self) -> Option<Size> {
        self.scroll.viewports.get(&self.widget_id()).copied()
    }

    /// Scrolls this widget's scroll container to `offset` after the build
    /// (clamped to its content), e.g. to keep a selected row visible.
    pub fn set_scroll_offset(&mut self, offset: Point) {
        let id = self.widget_id();
        self.scroll_requests.push((id, offset));
    }

    /// Seconds since the app started, as of this build. Reading it does not
    /// schedule anything; use [`Self::wake_at`] to be rebuilt later.
    pub fn now(&self) -> f64 {
        self.now
    }

    /// Asks for a rebuild at `time` (seconds, the same clock as [`Self::now`]):
    /// toasts that expire, carousels that advance, spinners that turn.
    pub fn wake_at(&mut self, time: f64) {
        self.wake = Some(self.wake.map_or(time, |w| w.min(time)));
    }

    /// Registers an app-wide keyboard shortcut for as long as this view is built.
    /// With `command`, it needs Cmd (macOS) or Ctrl held, e.g. Cmd/Ctrl+K.
    pub fn shortcut(&mut self, key: Key, command: bool, handler: Rc<dyn Fn()>) {
        self.shortcuts.push(Shortcut { key, command, handler });
    }

    /// Calls `handler` with the pointer position on a secondary (right) click.
    pub fn on_context_menu(&mut self, node: NodeId, handler: Rc<dyn Fn(Point)>) {
        if let Some(e) = self.element_mut(node) {
            e.on_context_menu = Some(handler);
        }
    }

    /// Moves keyboard focus to this widget's focusable element after the build.
    pub fn request_focus(&mut self) {
        self.focus_request = Some(self.widget_id());
    }

    /// Keyboard modifiers as of the latest input event, readable from click
    /// handlers (e.g. Shift-click to extend a selection).
    pub fn modifiers(&self) -> Rc<std::cell::Cell<Modifiers>> {
        self.modifiers.clone()
    }

    /// A scroll container in both directions whose content is rebuilt on every
    /// scroll (see [`Self::scroll_viewport`]).
    pub fn scrollable_virtual(&mut self, node: NodeId, horizontal: bool) {
        if let Some(e) = self.element_mut(node) {
            e.scroll = true;
            e.scroll_x = horizontal;
            e.clip = true;
            e.rebuild_on_scroll = true;
        }
    }

    /// Moves the node's children by `offset` when painting (e.g. a header that
    /// follows a body's horizontal scroll).
    pub fn translate(&mut self, node: NodeId, offset: Point) {
        if let Some(e) = self.element_mut(node) {
            e.translate = offset;
        }
    }

    /// Calls `handler` on a double click.
    pub fn on_double_click(&mut self, node: NodeId, handler: Rc<dyn Fn()>) {
        if let Some(e) = self.element_mut(node) {
            e.on_double_click = Some(handler);
        }
    }

    /// Makes the node a vertical scroll container: its children scroll with the
    /// wheel or trackpad, are clipped, and a scrollbar is drawn when they overflow.
    /// Give the node `LayoutStyle::overflow_hidden` so it does not grow to fit.
    pub fn scrollable(&mut self, node: NodeId) {
        if let Some(e) = self.element_mut(node) {
            e.scroll = true;
            e.clip = true;
        }
    }

    /// Builds `view` as a floating layer above the rest of the UI, placed relative
    /// to `anchor`. `on_dismiss` is called on a click outside the layer and its
    /// anchor, or on Escape.
    pub fn overlay(
        &mut self,
        anchor: NodeId,
        placement: Placement,
        view: &dyn View,
        on_dismiss: Option<Rc<dyn Fn()>>,
    ) {
        let key = 0x6f76_6572_6c61_7900 + self.overlays.len() as u64;
        let root = self.keyed_child(key, view);
        let gap = self.theme.spacing.s2;
        self.overlays.push(Overlay { root, anchor, placement, gap, on_dismiss });
    }

    fn element_mut(&mut self, node: NodeId) -> Option<&mut Element> {
        self.elements.get_mut(node.0 as usize)
    }

    fn push(&mut self, element: Element) -> NodeId {
        self.elements.push(element);
        NodeId((self.elements.len() - 1) as u32)
    }

    fn fail(&mut self, e: LayoutError) -> LayoutId {
        tracing::error!(error = %e, "layout node could not be created");
        if self.error.is_none() {
            self.error = Some(e);
        }
        self.placeholder
    }
}
