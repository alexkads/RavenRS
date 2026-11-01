use std::any::Any;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::rc::Rc;

use ravenrs_animation::{Animatable, Animated, Transition};
use ravenrs_core::{
    Border, Brush, Color, CursorIcon, Key, Modifiers, NumericValue, Point, Rect, Role, Scene, Shadow, Size,
    Toggled,
};
use ravenrs_layout::{LayoutError, LayoutId, LayoutStyle, LayoutTree};
use ravenrs_reactive::State;
use ravenrs_style::Theme;
use ravenrs_text::{TextAttrs, TextLayout, TextSystem};

use crate::View;
use crate::motion::{Motion, MotionEntry, MotionKey};
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

    /// The raw identity, for derived ids such as accessibility node ids.
    pub(crate) fn raw(self) -> u64 {
        self.0
    }
}

/// A node of the element tree produced by one build. Only valid during the
/// build that created it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NodeId(pub(crate) u32);

/// A rounded rectangle painted behind an element's children.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct QuadPaint {
    /// Fill color or gradient.
    pub fill: Brush,
    /// Corner radius in logical pixels.
    pub corner_radius: f32,
    /// Optional border stroke.
    pub border: Option<Border>,
    /// Optional drop shadow.
    pub shadow: Option<Shadow>,
}

impl QuadPaint {
    /// A square-cornered quad with this fill, no border and no shadow.
    pub fn new(fill: impl Into<Brush>) -> Self {
        Self { fill: fill.into(), corner_radius: 0.0, border: None, shadow: None }
    }
}

/// Custom drawing callback: receives the element's absolute frame in logical pixels.
pub type DrawFn = Rc<dyn Fn(Rect, &mut Scene)>;

/// Drawing that changes over time: receives the element's frame and the clock in
/// seconds. It runs on paint-only frames, at most 60 times a second, for as long
/// as the view is built. With reduced motion it runs once, at time zero.
pub type LiveFn = Rc<dyn Fn(Rect, f64, &mut Scene)>;

/// What [`Paint::Rich`] draws with.
#[derive(Debug)]
#[non_exhaustive]
pub struct PaintCx<'a> {
    /// The display list.
    pub scene: &'a mut Scene,
    /// Shapes text that depends on the frame, which a build does not know yet.
    pub text: &'a mut TextSystem,
    /// Physical pixels per logical pixel, to make pictures that stay sharp.
    pub scale: f32,
}

/// Draws a node with [`PaintCx`], given its absolute frame.
pub type RichFn = Rc<dyn Fn(Rect, &mut PaintCx<'_>)>;

/// What an element draws before its children.
#[derive(Clone, Default)]
#[non_exhaustive]
pub enum Paint {
    /// Draws nothing.
    #[default]
    None,
    /// A rounded rectangle.
    Quad(QuadPaint),
    /// Arbitrary drawing into the scene, given the element's absolute frame.
    Custom(DrawFn),
    /// Arbitrary drawing that keeps changing (spinners, pulses), without rebuilding.
    Live(LiveFn),
    /// Arbitrary drawing that also shapes text or needs the device scale.
    Rich(RichFn),
}

impl fmt::Debug for Paint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::None => f.write_str("None"),
            Self::Quad(q) => f.debug_tuple("Quad").field(q).finish(),
            Self::Custom(_) => f.write_str("Custom"),
            Self::Live(_) => f.write_str("Live"),
            Self::Rich(_) => f.write_str("Rich"),
        }
    }
}

/// Called with the focused element's key presses; returns true when it handled the key.
pub type KeyHandler = Rc<dyn Fn(&Key, Modifiers) -> bool>;

/// Phase of a pointer drag that started on an element.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum DragPhase {
    /// The pointer went down and started moving.
    Start,
    /// The pointer moved while dragging.
    Move,
    /// The pointer was released.
    End,
}

/// A pointer drag on an element, in absolute logical pixels, with the element's frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Drag {
    /// Which part of the drag this event is.
    pub phase: DragPhase,
    /// Pointer position in absolute logical pixels.
    pub position: Point,
    /// The dragged element's absolute frame.
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

/// Called for each phase of a pointer drag on an element.
pub type DragHandler = Rc<dyn Fn(Drag)>;

/// Where an overlay is placed relative to its anchor.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub enum Placement {
    /// Below the anchor, left edges aligned.
    Below,
    /// Above the anchor, left edges aligned.
    Above,
    /// To the right of the anchor, top edges aligned.
    Right,
    /// Centered above the anchor (tooltips).
    AboveCenter,
    /// To the right of the anchor, centers aligned (tooltips beside a rail).
    RightCenter,
    /// To the left of the anchor, centers aligned (tooltips beside a rail).
    LeftCenter,
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

/// How a node appears to screen readers: its [`Role`], name and state.
///
/// Built-in components set this themselves. Custom components call
/// [`BuildCx::semantics`] on the node they return:
///
/// ```
/// use ravenrs_core::Role;
/// use ravenrs_widgets::Semantics;
///
/// let s = Semantics::new(Role::Switch).name("Wi-Fi").toggled(true);
/// assert_eq!(s.role(), Role::Switch);
/// ```
///
/// Without semantics, a focusable node is announced as a button named by the
/// text inside it, text is announced as a label, and other boxes are
/// transparent: their children belong to the nearest described ancestor.
#[derive(Clone, Debug, PartialEq)]
pub struct Semantics {
    pub(crate) role: Role,
    pub(crate) name: Option<String>,
    pub(crate) description: Option<String>,
    pub(crate) value: Option<String>,
    pub(crate) numeric: Option<NumericValue>,
    pub(crate) toggled: Option<Toggled>,
    pub(crate) selected: Option<bool>,
    pub(crate) expanded: Option<bool>,
    pub(crate) disabled: bool,
    pub(crate) modal: bool,
    pub(crate) level: Option<u32>,
    pub(crate) hidden: bool,
    pub(crate) active: bool,
    /// The name is a guess (an icon's name) that a tooltip may replace.
    pub(crate) fallback_name: bool,
}

impl Semantics {
    /// A node with this role, named by its content when the role allows it.
    pub fn new(role: Role) -> Self {
        Self {
            role,
            name: None,
            description: None,
            value: None,
            numeric: None,
            toggled: None,
            selected: None,
            expanded: None,
            disabled: false,
            modal: false,
            level: None,
            hidden: false,
            active: false,
            fallback_name: false,
        }
    }

    /// Hides the node and everything inside it from assistive technology
    /// (decorations, duplicated content).
    pub fn hidden() -> Self {
        Self { hidden: true, ..Self::new(Role::Group) }
    }

    /// The role.
    pub fn role(&self) -> Role {
        self.role
    }

    /// Sets the accessible name, replacing the name from content.
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Sets a description, read after the name.
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Sets a text value (text fields, combo boxes).
    pub fn value(mut self, value: impl Into<String>) -> Self {
        self.value = Some(value.into());
        self
    }

    /// Sets a numeric value and its range.
    pub fn numeric(mut self, value: f64, min: f64, max: f64, step: Option<f64>) -> Self {
        self.numeric = Some(NumericValue { value, min, max, step });
        self
    }

    /// Sets the checked state.
    pub fn toggled(mut self, toggled: impl Into<Toggled>) -> Self {
        self.toggled = Some(toggled.into());
        self
    }

    /// Sets the selected state.
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = Some(selected);
        self
    }

    /// Sets the expanded state.
    pub fn expanded(mut self, expanded: bool) -> Self {
        self.expanded = Some(expanded);
        self
    }

    /// Marks the control as not responding to input.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Marks a dialog as modal.
    pub fn modal(mut self) -> Self {
        self.modal = true;
        self
    }

    /// Marks the option highlighted in a list that a focused text field
    /// drives (combobox pattern), so screen readers announce it.
    pub fn active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }

    /// Sets the heading level (1 to 6).
    pub fn level(mut self, level: u32) -> Self {
        self.level = Some(level.clamp(1, 6));
        self
    }
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
    /// How the element appears to assistive technology.
    pub semantics: Option<Box<Semantics>>,
    /// The string a text leaf shows.
    pub text: Option<Rc<str>>,
    /// Index of this element's entry among the build's motions.
    pub motion: Option<u32>,
    /// Report the size of the content (see [`BuildCx::measured_content`]).
    pub measure: bool,
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
            semantics: None,
            text: None,
            motion: None,
            measure: false,
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
    /// One line ended with `…` when it does not fit; holds what to shape again then.
    pub truncate: Option<(Rc<str>, TextAttrs)>,
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
    next_motion: u64,
}

/// Scroll positions and visible sizes of scroll containers, by widget.
#[derive(Debug, Default)]
pub(crate) struct ScrollState {
    pub offsets: HashMap<WidgetId, Point>,
    /// Frame size of each scroll container in the last layout.
    pub viewports: HashMap<WidgetId, Size>,
    /// Content size of each measured element in the last layout.
    pub contents: HashMap<WidgetId, Size>,
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
    labels: Rc<crate::Labels>,
    sidebar: Option<Rc<crate::nav::SidebarScope>>,
    pub(crate) shortcuts: Vec<Shortcut>,
    pub(crate) wake: Option<f64>,
    pub(crate) modifiers: Rc<std::cell::Cell<Modifiers>>,
    pub(crate) local: &'a mut LocalStore,
    pub(crate) placeholder: LayoutId,
    pub(crate) error: Option<LayoutError>,
    pub(crate) clipboard: crate::Clipboard,
    pub(crate) motions: Vec<MotionEntry>,
    pub(crate) motion_clock: f64,
    pub(crate) motion_enabled: bool,
    pub(crate) reduced_motion: bool,
    /// A value from [`Self::animated`] is still moving.
    pub(crate) value_moving: bool,
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
        clipboard: crate::Clipboard,
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
            labels: Rc::new(crate::Labels::default()),
            sidebar: None,
            shortcuts: Vec::new(),
            wake: None,
            modifiers,
            local,
            placeholder,
            error: None,
            clipboard,
            motions: Vec::new(),
            motion_clock: 0.0,
            motion_enabled: false,
            reduced_motion: false,
            value_moving: false,
            scopes: vec![Scope { id: WidgetId::ROOT, next_child: 0, next_state: 0, next_motion: 0 }],
        }
    }

    /// The theme in effect for this build.
    pub fn theme(&self) -> &Theme {
        &self.theme
    }

    /// The words the components use here; see [`Localized`](crate::Localized).
    pub fn labels(&self) -> Rc<crate::Labels> {
        self.labels.clone()
    }

    pub(crate) fn replace_labels(&mut self, labels: Rc<crate::Labels>) -> Rc<crate::Labels> {
        std::mem::replace(&mut self.labels, labels)
    }

    /// The sidebar this view is being built inside, if any.
    pub(crate) fn sidebar(&self) -> Option<Rc<crate::nav::SidebarScope>> {
        self.sidebar.clone()
    }

    pub(crate) fn replace_sidebar(
        &mut self,
        sidebar: Option<Rc<crate::nav::SidebarScope>>,
    ) -> Option<Rc<crate::nav::SidebarScope>> {
        std::mem::replace(&mut self.sidebar, sidebar)
    }

    /// The app-wide theme as state: set it to switch themes at runtime.
    pub fn theme_state(&self) -> State<Theme> {
        self.theme_state.clone()
    }

    /// The system clipboard, for copy and paste handlers.
    pub fn clipboard(&self) -> crate::Clipboard {
        self.clipboard.clone()
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

    /// Keyboard focus is on this view.
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
        self.scopes.push(Scope { id, next_child: 0, next_state: 0, next_motion: 0 });
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
        let id = match self.layout.measured_leaf(style, TextLeaf { layout, truncate: None }) {
            Ok(id) => id,
            Err(e) => self.fail(e),
        };
        let mut element = Element::new(self.widget_id(), id);
        element.text_color = Some(color);
        element.text = Some(text.into());
        self.push(element)
    }

    /// Like [`Self::text`], on one line that ends with `…` when it does not fit.
    /// Screen readers still get the whole text.
    pub fn text_truncated(
        &mut self,
        text: &str,
        attrs: &TextAttrs,
        color: Color,
        style: LayoutStyle,
    ) -> NodeId {
        let node = self.text(text, attrs, color, style);
        let layout = self.elements[node.0 as usize].layout;
        if let Some(leaf) = self.layout.context_mut(layout) {
            leaf.truncate = Some((text.into(), attrs.clone()));
        }
        node
    }

    /// Measures `text` without creating a node, e.g. to place a caret.
    pub fn measure_text(&mut self, text: &str, attrs: &TextAttrs) -> Size {
        self.text.layout(text, attrs).measure(None)
    }

    /// An empty box that takes no space.
    pub fn empty(&mut self) -> NodeId {
        // Out of the flow, so a `None` or a closed dialog takes no gap in its stack.
        self.container(LayoutStyle { absolute: Some((0.0, 0.0)), ..LayoutStyle::leaf() }, &[], Paint::None)
    }

    /// Calls `handler` when the node is clicked, or activated with Enter/Space while focused.
    pub fn on_click(&mut self, node: NodeId, handler: Rc<dyn Fn()>) {
        if let Some(e) = self.element_mut(node) {
            e.on_click = Some(handler);
        }
    }

    /// Describes the node to screen readers; see [`Semantics`].
    pub fn semantics(&mut self, node: NodeId, semantics: Semantics) {
        if let Some(e) = self.element_mut(node) {
            e.semantics = Some(Box::new(semantics));
        }
    }

    /// Describes a node built by someone else (content passed in by the app)
    /// unless it already describes itself.
    pub(crate) fn semantics_if_none(&mut self, node: NodeId, semantics: Semantics) {
        if let Some(e) = self.element_mut(node)
            && e.semantics.is_none()
            && e.text.is_none()
        {
            e.semantics = Some(Box::new(semantics));
        }
    }

    /// Changes the semantics a child already set (marking a button selected, say).
    pub(crate) fn update_semantics(&mut self, node: NodeId, f: impl FnOnce(&mut Semantics)) {
        if let Some(s) = self.element_mut(node).and_then(|e| e.semantics.as_deref_mut()) {
            f(s);
        }
    }

    /// The first described or focusable node at or under `node`, in tree
    /// order: the control a label or tooltip around it is about.
    fn control_in(&self, node: NodeId) -> Option<usize> {
        let mut stack = vec![node];
        while let Some(n) = stack.pop() {
            let e = self.elements.get(n.0 as usize)?;
            if e.semantics.as_deref().is_some_and(|s| !s.hidden) || e.focus_radius.is_some() {
                return Some(n.0 as usize);
            }
            stack.extend(e.children.iter().rev());
        }
        None
    }

    /// Names the control at or under `node` with `name` unless it already has
    /// a name of its own, and sets `description` (form fields, tooltips).
    /// When it has a name, `name` becomes its description instead.
    pub(crate) fn label_control(&mut self, node: NodeId, name: &str, description: Option<&str>) {
        let Some(i) = self.control_in(node) else { return };
        let e = &mut self.elements[i];
        let s = e.semantics.get_or_insert_with(|| Box::new(Semantics::new(Role::Button)));
        if s.name.is_none() || s.fallback_name {
            s.name = Some(name.to_owned());
            s.fallback_name = false;
            if let Some(d) = description {
                s.description = Some(d.to_owned());
            }
        } else if s.name.as_deref() != Some(name) {
            s.description = Some(description.map_or_else(|| name.to_owned(), |d| format!("{name}. {d}")));
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

    /// Declares how `node` animates; see [`Motion`]. Call it in the same order on
    /// every build, or give the motion a [`Motion::key`].
    pub fn motion(&mut self, node: NodeId, motion: Motion) {
        let id = self.widget_id();
        let key = match motion.shared {
            Some((scope, tag)) => MotionKey::Shared(scope, tag),
            None => match (motion.key, self.scopes.last_mut()) {
                (Some(k), _) => MotionKey::Node(id, k | (1 << 63)),
                (None, Some(scope)) => {
                    scope.next_motion += 1;
                    MotionKey::Node(id, scope.next_motion)
                }
                (None, None) => MotionKey::Node(id, 0),
            },
        };
        let index = self.motions.len() as u32;
        if let Some(e) = self.element_mut(node) {
            e.motion = Some(index);
            self.motions.push(MotionEntry { node, key, motion });
        }
    }

    /// A value that eases toward `target`, for what [`Motion`] does not cover:
    /// numbers that feed layout or custom drawing. Call it in the same order on
    /// every build. It rebuilds the UI on each frame while it moves, so prefer
    /// [`Motion`] when you can.
    pub fn animated<T: Animatable + 'static>(&mut self, target: T, transition: Transition) -> T {
        let cell = self.use_state(|| Rc::new(RefCell::new(Animated::new(target))));
        let cell = cell.peek().clone();
        let mut value = cell.borrow_mut();
        let now = self.motion_clock;
        let transition =
            if !self.motion_enabled || self.reduced_motion { Transition::Instant } else { transition };
        value.set_target(target, transition, now);
        if value.step(now) {
            self.value_moving = true;
            self.wake_at(self.now);
        }
        value.value(now)
    }

    /// The user asked for less motion, in the system settings or in the theme.
    /// [`Motion`] already respects this; check it for motion you draw yourself.
    pub fn reduced_motion(&self) -> bool {
        self.reduced_motion
    }

    /// Asks the runtime to report the size of the content of `node`, read back
    /// with [`Self::measured_content`] on the next build.
    pub fn measure_content(&mut self, node: NodeId) {
        if let Some(e) = self.element_mut(node) {
            e.measure = true;
        }
    }

    /// The size of the content of this view's measured node in the last layout.
    pub fn measured_content(&self) -> Option<Size> {
        self.scroll.contents.get(&self.widget_id()).copied()
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

    /// Moves keyboard focus to this widget's focusable element after the build,
    /// scrolling the containers around it until it shows.
    pub fn request_focus(&mut self) {
        self.focus_request = Some(self.widget_id());
    }

    /// Moves keyboard focus to the widget that built `node` (a child's trigger
    /// button, say) after the build.
    pub(crate) fn request_focus_node(&mut self, node: NodeId) {
        if let Some(e) = self.elements.get(node.0 as usize) {
            self.focus_request = Some(e.widget);
        }
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

    /// Lets a scroll container (see [`Self::scrollable`]) scroll sideways too.
    pub fn scroll_horizontally(&mut self, node: NodeId) {
        if let Some(e) = self.element_mut(node) {
            e.scroll_x = true;
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
