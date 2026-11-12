use std::any::Any;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::rc::Rc;

use ravenrs_animation::{Animatable, Animated, Transition};
use ravenrs_core::{
    AccessTree, Border, Brush, Color, CursorIcon, Key, Modifiers, NumericValue, Point, Rect, Role, Scene,
    Shadow, Size, Toggled,
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
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct WidgetId(u64);

#[cfg_attr(feature = "live", ravenrs_live::expose)]
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
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct NodeId(pub(crate) u32);

/// A rounded rectangle painted behind an element's children.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
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

#[cfg_attr(feature = "live", ravenrs_live::expose)]
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
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "variants"))]
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
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "variants"))]
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
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
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

#[cfg_attr(feature = "live", ravenrs_live::expose)]
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

/// What just happened in the UI, for something watching it as a whole.
#[derive(Clone, Copy, Debug)]
#[non_exhaustive]
pub enum UiActivity<'a> {
    /// A frame finished painting, and this is it.
    Painted(&'a Scene),
    /// The pointer moved here.
    PointerMoved(Point),
    /// The primary button went down here.
    Clicked(Point),
    /// What is on screen, as asked for with [`BuildCx::ask_for_access_tree`].
    Described(&'a AccessTree),
}

/// Receives what happens in the UI, with the clock; see [`BuildCx::observe_activity`].
pub type ActivityObserver = Rc<dyn Fn(UiActivity<'_>, f64)>;

/// Where an overlay is placed relative to its anchor.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "variants"))]
pub enum Placement {
    /// Below the anchor, left edges aligned.
    Below,
    /// Above the anchor, left edges aligned.
    Above,
    /// To the right of the anchor, top edges aligned; to its left when only that fits.
    Right,
    /// Centered above the anchor (tooltips).
    AboveCenter,
    /// To the right of the anchor, centers aligned (tooltips beside a rail).
    RightCenter,
    /// To the left of the anchor, centers aligned (tooltips beside a rail).
    LeftCenter,
    /// To the right of the anchor, bottom edges aligned.
    RightEnd,
    /// To the left of the anchor, top edges aligned.
    LeftStart,
    /// To the left of the anchor, bottom edges aligned.
    LeftEnd,
    /// Covers the whole viewport (dialogs, sheets); the anchor is ignored.
    Fill,
    /// At a point in the viewport (context menus); the anchor is ignored.
    At(Point),
    /// The bottom-right corner of the viewport (toasts).
    BottomRight,
    /// The bottom-left corner of the viewport; the anchor is ignored.
    BottomLeft,
    /// The middle of the bottom edge of the viewport; the anchor is ignored.
    BottomCenter,
    /// The top-left corner of the viewport; the anchor is ignored.
    TopLeft,
    /// The middle of the top edge of the viewport; the anchor is ignored.
    TopCenter,
    /// The top-right corner of the viewport; the anchor is ignored.
    TopRight,
}

/// A keyboard shortcut that works wherever focus is (unless the focused
/// element handles the key first). `command` means Cmd on macOS, Ctrl elsewhere;
/// both are accepted on every platform.
#[derive(Clone)]
pub(crate) struct Shortcut {
    pub key: Key,
    pub command: bool,
    /// It also needs Shift; without this, Shift does not matter.
    pub shift: bool,
    /// It needs Alt, and goes before what the focused element would do with the key.
    pub alt: bool,
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
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
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
    pub(crate) read_only: bool,
    pub(crate) modal: bool,
    pub(crate) level: Option<u32>,
    pub(crate) hidden: bool,
    pub(crate) active: bool,
    /// The name is a guess (an icon's name) that a tooltip may replace.
    pub(crate) fallback_name: bool,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
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
            read_only: false,
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

    /// Marks the control's value as readable but not changeable.
    pub fn read_only(mut self, read_only: bool) -> Self {
        self.read_only = read_only;
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
    /// Stays below every other layer, whenever it was built.
    pub floating: bool,
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
    /// How far past its edge the scrollbar goes.
    pub scrollbar_outside: f32,
    /// Scrolling rebuilds the UI (virtualized content that depends on the offset).
    pub rebuild_on_scroll: bool,
    /// Visual offset applied to the children's positions.
    pub translate: Point,
    pub on_double_click: Option<Rc<dyn Fn()>>,
    pub on_pinch: Option<Rc<dyn Fn(f32)>>,
    /// Takes the wheel over it, before any scroll container around it.
    pub on_wheel: Option<Rc<dyn Fn(Point)>>,
    /// The wheel over it moves what this widget scrolls.
    pub scrolls_with_widget: bool,
    /// A wheel that only turns up and down moves this scroll container sideways.
    pub wheel_sideways: bool,
    pub on_context_menu: Option<Rc<dyn Fn(Point)>>,
    pub on_file_drop: Option<Rc<dyn Fn(Vec<crate::PickedFile>)>>,
    pub on_context_menu_in: Option<Rc<dyn Fn(Point, Rect)>>,
    pub on_pointer_move: Option<Rc<dyn Fn(Point, Rect)>>,
    /// How the element appears to assistive technology.
    pub semantics: Option<Box<Semantics>>,
    /// The string a text leaf shows.
    pub text: Option<Rc<str>>,
    /// Index of this element's entry among the build's motions.
    pub motion: Option<u32>,
    /// Report the size of the content (see [`BuildCx::measured_content`]).
    pub measure: bool,
    pub report_frame: bool,
    /// Stays this far below the top of its scroll container while its parent passes.
    pub pin: Option<f32>,
    /// The track of a scroll scene: what [`crate::ScrollRange::Scene`] measures.
    pub scroll_track: Option<crate::scroll_scene::ScrollTrack>,
    /// Painted through this effect of the scroll position.
    pub scroll_link: Option<crate::ScrollLink>,
    /// A place links can scroll to (`#name`), and how far below the top it lands.
    pub anchor: Option<(Rc<str>, f32)>,
    /// Called once, the first time the element shows in its scroll container.
    pub on_appear: Option<Rc<dyn Fn()>>,
    /// Painted through this effect of the clock and the element's frame.
    pub time_link: Option<crate::TimeLink>,
    /// Told the scroll progress along this range before each paint.
    pub progress_out: Option<(crate::ScrollRange, Rc<std::cell::Cell<f32>>)>,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
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
            scrollbar_outside: 0.0,
            scroll_x: false,
            rebuild_on_scroll: false,
            translate: Point::ZERO,
            on_double_click: None,
            on_pinch: None,
            on_wheel: None,
            scrolls_with_widget: false,
            wheel_sideways: false,
            on_context_menu: None,
            on_file_drop: None,
            on_context_menu_in: None,
            on_pointer_move: None,
            semantics: None,
            text: None,
            motion: None,
            measure: false,
            report_frame: false,
            pin: None,
            scroll_track: None,
            scroll_link: None,
            anchor: None,
            on_appear: None,
            time_link: None,
            progress_out: None,
        }
    }

    pub fn is_interactive(&self) -> bool {
        self.on_click.is_some()
            || self.on_double_click.is_some()
            || self.on_context_menu.is_some()
            || self.on_context_menu_in.is_some()
            || self.on_pointer_move.is_some()
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

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl LocalStore {
    pub fn begin_build(&mut self) {
        self.used.clear();
    }

    pub fn end_build(&mut self) {
        let used = &self.used;
        self.slots.retain(|k, _| used.contains(k));
    }
}

thread_local! {
    // A build happens on every change and most never replace the words, while
    // `Labels` holds hundreds of strings: built once, or each build pays for them.
    static DEFAULT_LABELS: Rc<crate::Labels> = Rc::new(crate::Labels::default());
}

/// What a [`View`] uses to describe itself during a build: theme access,
/// interaction state, child building, layout boxes, painting and behavior.
pub struct BuildCx<'a> {
    /// The views that asked about the pointer, so that it moving elsewhere rebuilds nothing.
    pub(crate) hover_readers: Rc<RefCell<HashSet<WidgetId>>>,
    pub(crate) view_profile: Option<Rc<RefCell<crate::ui::ViewProfile>>>,
    pub(crate) theme: Theme,
    pub(crate) theme_state: State<Theme>,
    pub(crate) viewport: Size,
    pub(crate) overlays: Vec<Overlay>,
    floating_overlays: u64,
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
    ui_waker: crate::UiWaker,
    sidebar: Option<Rc<crate::nav::SidebarScope>>,
    pub(crate) shortcuts: Vec<Shortcut>,
    pub(crate) activity: Option<ActivityObserver>,
    pub(crate) access_tree_asked: bool,
    pub(crate) wake: Option<f64>,
    pub(crate) modifiers: Rc<std::cell::Cell<Modifiers>>,
    pub(crate) local: &'a mut LocalStore,
    pub(crate) placeholder: LayoutId,
    pub(crate) error: Option<LayoutError>,
    pub(crate) clipboard: crate::Clipboard,
    pub(crate) sound: crate::Sound,
    pub(crate) links: crate::Links,
    pub(crate) full_screen: crate::FullScreen,
    pub(crate) file_picker: crate::FilePicker,
    pub(crate) files_over: State<bool>,
    pub(crate) appearance: crate::SystemAppearance,
    pub(crate) motions: Vec<MotionEntry>,
    pub(crate) motion_clock: f64,
    pub(crate) motion_enabled: bool,
    pub(crate) reduced_motion: bool,
    /// A value from [`Self::animated`] is still moving.
    pub(crate) value_moving: bool,
    /// The text color of the view being built, for content that takes it from what it is in.
    foreground: Option<Color>,
    scopes: Vec<Scope>,
}

impl fmt::Debug for BuildCx<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BuildCx").field("widget", &self.widget_id()).finish_non_exhaustive()
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
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
        ui_waker: crate::UiWaker,
    ) -> Self {
        Self {
            hover_readers: Rc::default(),
            view_profile: None,
            // Read through the state so a theme change rebuilds the UI.
            theme: theme_state.get(),
            theme_state,
            viewport,
            overlays: Vec::new(),
            floating_overlays: 0,
            text,
            layout,
            elements,
            interaction,
            hover_chain,
            scroll,
            scroll_requests: Vec::new(),
            focus_request: None,
            now: 0.0,
            labels: DEFAULT_LABELS.with(Rc::clone),
            ui_waker,
            sidebar: None,
            shortcuts: Vec::new(),
            activity: None,
            access_tree_asked: false,
            wake: None,
            modifiers,
            local,
            placeholder,
            error: None,
            clipboard,
            sound: crate::Sound::default(),
            links: crate::Links::default(),
            full_screen: crate::FullScreen::default(),
            file_picker: crate::FilePicker::default(),
            files_over: State::new(false),
            appearance: crate::SystemAppearance::default(),
            motions: Vec::new(),
            motion_clock: 0.0,
            motion_enabled: false,
            reduced_motion: false,
            value_moving: false,
            foreground: None,
            scopes: vec![Scope { id: WidgetId::ROOT, next_child: 0, next_state: 0, next_motion: 0 }],
        }
    }

    /// The text color of the view being built, if a view around it set one with
    /// [`Self::with_foreground`]. Colors do not cascade by themselves, so this is how
    /// a [`Kbd`](crate::Kbd) inside a filled button reads as part of the button.
    pub fn foreground(&self) -> Option<Color> {
        self.foreground
    }

    /// Builds what `build` builds with `color` as its [`Self::foreground`]. Layers opened
    /// inside it start without one, since they are not painted over this view.
    pub fn with_foreground<R>(&mut self, color: Color, build: impl FnOnce(&mut Self) -> R) -> R {
        let outer = self.foreground.replace(color);
        let result = build(self);
        self.foreground = outer;
        result
    }

    /// The theme in effect for this build.
    pub fn theme(&self) -> &Theme {
        &self.theme
    }

    /// A handle that asks for a rebuild from another thread; see [`UiWaker`](crate::UiWaker).
    pub fn ui_waker(&self) -> crate::UiWaker {
        self.ui_waker.clone()
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

    /// Whether the system is light or dark, and whether the theme follows it.
    pub fn system_appearance(&self) -> crate::SystemAppearance {
        self.appearance.clone()
    }

    /// Opens addresses: in the browser it navigates, on the desktop the system's
    /// browser opens them. `#name` scrolls to the anchor of that name instead.
    pub fn links(&self) -> crate::Links {
        self.links.clone()
    }

    /// Puts the app in full screen and takes it out: the page's full screen in the
    /// browser, a borderless full screen window on the desktop.
    pub fn full_screen(&self) -> crate::FullScreen {
        self.full_screen.clone()
    }

    /// Marks the node as the place `#name` links scroll to, landing `offset` pixels
    /// below the top of its scroll container (the height of a bar pinned over it).
    pub fn anchor(&mut self, node: NodeId, name: &str, offset: f32) {
        if let Some(e) = self.element_mut(node) {
            e.anchor = Some((Rc::from(name), offset));
        }
    }

    /// Calls `handler` once, after the first frame in which the node shows inside its
    /// scroll container: counters that start when they are seen, content loaded when
    /// it is reached. The handler runs outside the build, so it may set state.
    pub fn on_appear(&mut self, node: NodeId, handler: Rc<dyn Fn()>) {
        if let Some(e) = self.element_mut(node) {
            e.on_appear = Some(handler);
        }
    }

    /// Paints the node and its subtree with the effect `link` gives for the clock and
    /// the node's frame, on every frame while it shows: a strip that keeps moving, a
    /// gentle float. It neither rebuilds nor lays out. With reduced motion the clock
    /// stands at zero.
    pub fn time_linked(&mut self, node: NodeId, link: crate::TimeLink) {
        if let Some(e) = self.element_mut(node) {
            e.time_link = Some(link);
        }
    }

    /// Writes into `progress`, before each paint of the node, how far the scroll is along
    /// `range` from 0 to 1, for drawing that follows the scroll itself, such as a
    /// [`crate::QuadField`]. Effects that only move, fade or scale use
    /// [`Self::scroll_linked`] instead.
    pub fn scroll_progress(
        &mut self,
        node: NodeId,
        range: crate::ScrollRange,
        progress: Rc<std::cell::Cell<f32>>,
    ) {
        if let Some(e) = self.element_mut(node) {
            e.progress_out = Some((range, progress));
        }
    }

    /// The system clipboard, for copy and paste handlers.
    pub fn clipboard(&self) -> crate::Clipboard {
        self.clipboard.clone()
    }

    /// The system's sound and voice.
    pub fn sound(&self) -> crate::Sound {
        self.sound.clone()
    }

    /// The system's chooser of files.
    pub fn file_picker(&self) -> crate::FilePicker {
        self.file_picker.clone()
    }

    /// Whether files are being dragged over the window, for places that take them to show it.
    pub fn files_over(&self) -> bool {
        self.files_over.get()
    }

    /// Makes the node take files dropped on it.
    pub fn on_file_drop(&mut self, node: NodeId, handler: Rc<dyn Fn(Vec<crate::PickedFile>)>) {
        if let Some(e) = self.element_mut(node) {
            e.on_file_drop = Some(handler);
        }
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
        self.hover_readers.borrow_mut().insert(self.widget_id());
        self.interaction.hovered == Some(self.widget_id())
    }

    /// The pointer is over this view or over any interactive element inside it.
    pub fn is_hover_within(&self) -> bool {
        self.hover_readers.borrow_mut().insert(self.widget_id());
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

    /// Nothing on the screen has keyboard focus.
    pub fn nothing_focused(&self) -> bool {
        self.interaction.focused.is_none()
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
        let profile = self.view_profile.clone();
        if let Some(profile) = &profile {
            profile.borrow_mut().enter(view.type_name(), self.elements.len(), self.text.stats().shapes);
        }
        let node = view.build(self);
        if let Some(profile) = &profile {
            profile.borrow_mut().leave(self.elements.len(), self.text.stats().shapes);
        }
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

    /// Gives the control at or under `node` this name and this description, replacing what it
    /// had: for a name that says more than the label on the screen.
    pub(crate) fn rename_control(&mut self, node: NodeId, name: Option<&str>, description: Option<&str>) {
        let Some(i) = self.control_in(node) else { return };
        let e = &mut self.elements[i];
        let s = e.semantics.get_or_insert_with(|| Box::new(Semantics::new(Role::Button)));
        if let Some(name) = name {
            s.name = Some(name.to_owned());
            s.fallback_name = false;
        }
        if let Some(description) = description {
            s.description = Some(description.to_owned());
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

    /// The size that the keyed child built with `key` ended up with in the last layout, if
    /// it asked with [`Self::report_frame`]: for a view that places its children by how much
    /// room each one took, before building them again.
    pub fn keyed_child_frame(&self, key: u64) -> Option<Size> {
        self.scroll.viewports.get(&self.widget_id().child(key ^ (1 << 63))).copied()
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

    /// Watches the whole UI while this view is built: each painted frame, the
    /// pointer and clicks. For recording what happens, as a flight recorder does.
    /// It runs on every frame, so it has to be quick. The last call of a build wins.
    pub fn observe_activity(&mut self, observer: ActivityObserver) {
        self.activity = Some(observer);
    }

    /// Asks that [`Self::observe_activity`] be told what is on screen once this frame
    /// is painted, as [`UiActivity::Described`]. Building that description costs more
    /// than a frame should, so it comes only when asked for.
    pub fn ask_for_access_tree(&mut self) {
        self.access_tree_asked = true;
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

    /// Numbers that go from where they are to `target` along a tween, in full `f64`
    /// precision: a counter, or a chart's data changing. A new target while they move
    /// starts from where they are, with no jump. When the count of numbers changes, or
    /// under reduced motion, or for a spring, they take the target at once. Call it in
    /// the same order on every build, as with [`Self::animated`].
    pub fn animated_numbers(&mut self, target: &[f64], transition: Transition) -> Vec<f64> {
        let cell = self.use_state(|| Rc::new(RefCell::new(Numbers::at(target)))).peek().clone();
        let mut run = cell.borrow_mut();
        let now = self.motion_clock;
        let (duration, easing) = match transition {
            Transition::Tween { duration, easing } if self.motion_enabled && !self.reduced_motion => {
                (f64::from(duration), easing)
            }
            _ => (0.0, ravenrs_animation::Easing::Linear),
        };
        // By bits, so a gap (NaN) in the data counts as the same gap.
        if !run.to.iter().map(|v| v.to_bits()).eq(target.iter().map(|v| v.to_bits())) {
            let from =
                if run.to.len() == target.len() && duration > 0.0 { run.value(now) } else { target.to_vec() };
            *run = Numbers { from, to: target.to_vec(), start: now, duration, easing };
        }
        if now < run.start + run.duration {
            self.value_moving = true;
            self.wake_at(self.now);
        }
        run.value(now)
    }

    /// How far a view has appeared, from 0 to 1, since it was first built: for
    /// content that draws itself in, such as a chart's data. A new `key` starts it
    /// over. Under reduced motion, or with motion off, it is 1 from the start. Call
    /// it in the same order on every build, as with [`Self::animated`].
    pub fn appear(&mut self, key: u64, transition: Transition) -> f32 {
        let cell = self.use_state(|| Rc::new(RefCell::new((key, Animated::new(0.0_f32))))).peek().clone();
        let mut cell = cell.borrow_mut();
        if cell.0 != key {
            *cell = (key, Animated::new(0.0));
        }
        let now = self.motion_clock;
        let transition =
            if !self.motion_enabled || self.reduced_motion { Transition::Instant } else { transition };
        let value = &mut cell.1;
        value.set_target(1.0, transition, now);
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

    /// Makes the size this node ended up with readable on the next build, with
    /// [`Self::scroll_viewport`], and builds again when it changes: for a view that
    /// arranges itself by the room it got.
    pub fn report_frame(&mut self, node: NodeId) {
        if let Some(e) = self.element_mut(node) {
            e.report_frame = true;
        }
    }

    /// The size of the content of this view's measured node in the last layout.
    pub fn measured_content(&self) -> Option<Size> {
        self.scroll.contents.get(&self.widget_id()).copied()
    }

    /// Registers an app-wide keyboard shortcut for as long as this view is built.
    /// With `command`, it needs Cmd (macOS) or Ctrl held, e.g. Cmd/Ctrl+K.
    pub fn shortcut(&mut self, key: Key, command: bool, handler: Rc<dyn Fn()>) {
        self.shortcuts.push(Shortcut { key, command, shift: false, alt: false, handler });
    }

    /// Makes the wheel over `node` move what this widget scrolls, for a part that
    /// follows the scroll without being inside it, such as frozen row names.
    pub fn scroll_with_widget(&mut self, node: NodeId) {
        if let Some(e) = self.element_mut(node) {
            e.scrolls_with_widget = true;
        }
    }

    /// Like [`Self::shortcut`], for a key that also needs Shift. Register it before the
    /// same key without Shift, which would otherwise answer first.
    pub fn shortcut_with_shift(&mut self, key: Key, command: bool, handler: Rc<dyn Fn()>) {
        self.shortcuts.push(Shortcut { key, command, shift: true, alt: false, handler });
    }

    /// Like [`Self::shortcut`], for a key held with Alt. It wins over a text field that has the
    /// focus, since Alt and a letter would otherwise type an accented character.
    pub fn shortcut_with_alt(&mut self, key: Key, handler: Rc<dyn Fn()>) {
        self.shortcuts.push(Shortcut { key, command: false, shift: false, alt: true, handler });
    }

    /// Calls `handler` with the pointer position on a secondary (right) click.
    pub fn on_context_menu(&mut self, node: NodeId, handler: Rc<dyn Fn(Point)>) {
        if let Some(e) = self.element_mut(node) {
            e.on_context_menu = Some(handler);
        }
    }

    /// The same, also saying the element's absolute frame, for a menu that depends
    /// on where inside the element it was opened.
    pub fn on_context_menu_in(&mut self, node: NodeId, handler: Rc<dyn Fn(Point, Rect)>) {
        if let Some(e) = self.element_mut(node) {
            e.on_context_menu_in = Some(handler);
        }
    }

    /// Calls `handler` with the pointer position and the element's absolute frame
    /// whenever the pointer moves over it, for something that depends on where inside
    /// it the pointer is. [`is_hover_within`](Self::is_hover_within) says when it left.
    pub fn on_pointer_move(&mut self, node: NodeId, handler: Rc<dyn Fn(Point, Rect)>) {
        if let Some(e) = self.element_mut(node) {
            e.on_pointer_move = Some(handler);
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

    /// Makes a wheel that only turns up and down move this scroll container sideways,
    /// for a strip with nothing to scroll in the other direction.
    pub fn wheel_scrolls_sideways(&mut self, node: NodeId) {
        if let Some(e) = self.element_mut(node) {
            e.wheel_sideways = true;
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

    /// Keeps the node on screen while its parent scrolls past, `top` pixels below the
    /// top of the scroll container around it, and lets it leave with the bottom of the
    /// parent: CSS `position: sticky`. It is moved when frames are placed, so clicks,
    /// focus and assistive technology find it where it shows. See [`crate::Sticky`].
    pub fn pin(&mut self, node: NodeId, top: f32) {
        if let Some(e) = self.element_mut(node) {
            e.pin = Some(top);
        }
    }

    /// Marks the node as the track of a scroll scene: content pinned `top` pixels
    /// below the top of the scroll container for `length` pixels of scrolling. Links
    /// inside it that measure [`crate::ScrollRange::Scene`] run from 0 to 1 over that
    /// length. See [`crate::ScrollScene`].
    pub fn scroll_track(&mut self, node: NodeId, top: f32, length: f32) {
        if let Some(e) = self.element_mut(node) {
            e.scroll_track = Some(crate::scroll_scene::ScrollTrack { top, length });
        }
    }

    /// Paints the node and its subtree with the effect `link` gives for the current
    /// scroll position, worked out on every paint without rebuilding. Layout and hit
    /// testing are unaffected. See [`crate::ScrollLinked`].
    pub fn scroll_linked(&mut self, node: NodeId, link: crate::ScrollLink) {
        if let Some(e) = self.element_mut(node) {
            e.scroll_link = Some(link);
        }
    }

    /// Calls `handler` on a zoom gesture over the node, with how much larger was
    /// asked for relative to now: 0.02 is 2% larger, and a negative value is smaller.
    pub fn on_pinch(&mut self, node: NodeId, handler: Rc<dyn Fn(f32)>) {
        if let Some(e) = self.element_mut(node) {
            e.on_pinch = Some(handler);
        }
    }

    /// Calls `handler` with the wheel or two-finger scroll over the node, in logical pixels
    /// (positive `y` when the content should move down), instead of scrolling a container
    /// around it: for a surface that pans on its own, such as a diagram.
    pub fn on_wheel(&mut self, node: NodeId, handler: Rc<dyn Fn(Point)>) {
        if let Some(e) = self.element_mut(node) {
            e.on_wheel = Some(handler);
        }
    }

    /// Calls `handler` on a double click on the node.
    pub fn on_double_click(&mut self, node: NodeId, handler: Rc<dyn Fn()>) {
        if let Some(e) = self.element_mut(node) {
            e.on_double_click = Some(handler);
        }
    }

    /// Makes the node a vertical scroll container: its children scroll with the
    /// wheel or trackpad, are clipped, and a scrollbar shows while they move.
    /// Give the node `LayoutStyle::overflow_hidden` so it does not grow to fit.
    pub fn scrollable(&mut self, node: NodeId) {
        if let Some(e) = self.element_mut(node) {
            e.scroll = true;
            e.clip = true;
        }
    }

    /// Puts this scroll container's scrollbar `by` past its right edge, in the room around it.
    pub fn scrollbar_outside(&mut self, node: NodeId, by: f32) {
        if let Some(e) = self.element_mut(node) {
            e.scrollbar_outside = by;
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
        // The slot comes before the content: overlays are placed and painted in
        // order, so one opened from inside this one must come after it, or it
        // would be placed with no anchor yet and painted underneath.
        let key = 0x6f76_6572_6c61_7900 + self.overlays.len() as u64;
        self.keyed_overlay(key, anchor, placement, view, on_dismiss);
    }

    /// Like [`Self::overlay`], for a layer that must stay the same one when other
    /// layers before it come and go. A plain overlay is told apart by its place among
    /// them, so a new one before it makes it start over, and animate in again.
    pub fn keyed_overlay(
        &mut self,
        key: u64,
        anchor: NodeId,
        placement: Placement,
        view: &dyn View,
        on_dismiss: Option<Rc<dyn Fn()>>,
    ) {
        let slot = self.overlays.len();
        let gap = self.theme.spacing.s2;
        self.overlays.push(Overlay { root: anchor, anchor, placement, gap, on_dismiss, floating: false });
        let outer = self.foreground.take();
        let root = self.keyed_child(key, view);
        self.foreground = outer;
        self.overlays[slot].root = root;
    }

    /// Builds `view` as a layer that stays open beside the UI, like a chat in a
    /// corner: it is under every menu and dialog, wherever it is built, keeps its state
    /// when they open, and is not closed by a click outside it.
    pub fn floating_overlay(&mut self, anchor: NodeId, placement: Placement, view: &dyn View) {
        // Its key does not depend on the other layers, which come and go.
        let key = 0x666c_6f61_7400_0000 + self.floating_overlays;
        self.floating_overlays += 1;
        let slot = self.overlays.len();
        let gap = self.theme.spacing.s2;
        self.overlays.push(Overlay {
            root: anchor,
            anchor,
            placement,
            gap,
            on_dismiss: None,
            floating: true,
        });
        let outer = self.foreground.take();
        let root = self.keyed_child(key, view);
        self.foreground = outer;
        self.overlays[slot].root = root;
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

/// What [`BuildCx::animated_numbers`] keeps between builds.
struct Numbers {
    from: Vec<f64>,
    to: Vec<f64>,
    start: f64,
    duration: f64,
    easing: ravenrs_animation::Easing,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl Numbers {
    fn at(values: &[f64]) -> Self {
        let easing = ravenrs_animation::Easing::Linear;
        Self { from: values.to_vec(), to: values.to_vec(), start: 0.0, duration: 0.0, easing }
    }

    /// Exactly the target once the time is up.
    fn value(&self, now: f64) -> Vec<f64> {
        let t = if self.duration > 0.0 { (now - self.start) / self.duration } else { 1.0 };
        if t >= 1.0 {
            return self.to.clone();
        }
        let eased = f64::from(self.easing.sample(t.max(0.0) as f32));
        self.from.iter().zip(&self.to).map(|(a, b)| a + (b - a) * eased).collect()
    }
}
