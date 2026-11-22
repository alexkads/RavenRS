//! The drawing of a [`DockArea`]: everything sits where [`geometry`](super::geometry) says, under
//! one node, so a panel keeps its place in the widget tree as it moves between groups.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;

use ravenrs_core::{Border, Color, CursorIcon, Insets, Key, Modifiers, Point, Quad, Rect, Role, Size};
use ravenrs_layout::{Align, Axis, Justify, LayoutStyle};
use ravenrs_reactive::{Binding, State};
use ravenrs_style::{Semantic, TextStyle};

use super::geometry::{self, Arranged, BarKind, BarPlace, DropAim, GroupPlace, Metrics};
use super::layout::{DockChange, DockDrop, DockEdge, DockLayout, DockSide};
use crate::commands::menu_trigger;
use crate::context::Placement;
use crate::context::{BuildCx, DragPhase, NodeId, Paint, QuadPaint, Semantics};
use crate::overlay::Entry;
use crate::reorder;
use crate::stack::ScrollEdgeArrow;
use crate::util::{self, FocusTarget, Roving};
use crate::view::{AnyView, Keyed, View};
use crate::{
    Button, ButtonSize, ButtonStyle, ContextMenu, IconButton, IconName, Labels, MenuItem, QuarterTurn,
    SplitHandle, Text, Turn,
};

const TAB_HEIGHT: f32 = 32.0;
const TITLE_MAX_WIDTH: f32 = 180.0;
/// How thick the strip that collapses a dock is.
const TOGGLE: f32 = 12.0;
/// What keeps the keys of the different kinds of things under the area apart.
const KEY_PANEL: u8 = 1;
const KEY_STRIP: u8 = 2;
const KEY_BAR: u8 = 3;
const KEY_TOGGLE: u8 = 4;
const KEY_RAIL: u8 = 5;
const KEY_RAIL_TAB: u8 = 6;
const KEY_FLOATING: u8 = 7;
/// How long the pointer rests on a tab of a rail before its panel opens, in seconds.
const HOVER_OPEN: f64 = 0.4;
/// How long after the pointer leaves a panel it opened that the panel goes back, in seconds.
const HOVER_CLOSE: f64 = 0.6;
/// The least a hidden panel opens to, and what it leaves of the area at the most.
const OPEN_MIN: f32 = 160.0;
const OPEN_MARGIN: f32 = 48.0;
/// How large the dot of unsaved changes is.
const UNSAVED_DOT: f32 = 7.0;

/// The dot of a panel with unsaved changes; screen readers hear it from the tab, not from the dot.
fn unsaved_dot(cx: &mut BuildCx<'_>) -> NodeId {
    let dot = cx.child(&crate::Dot::new().tone(crate::Tone::Accent).size(UNSAVED_DOT));
    cx.semantics(dot, Semantics::hidden());
    dot
}

/// How long Ctrl+Tab is held before the list shows; a shorter press switches at once.
const QUICK_SWITCH: f64 = 0.15;
/// How wide the list of Ctrl+Tab is.
const SWITCHER_WIDTH: f32 = 560.0;

/// Ctrl+Tab while Ctrl is held: the panels by how lately they were used, and the one chosen.
#[derive(Clone, Debug, PartialEq)]
struct Switch {
    order: Vec<String>,
    index: usize,
    /// When it started, which its first build learns.
    since: Option<f64>,
    shown: bool,
}

/// The panels that show somewhere, the most lately used first.
fn switch_order(known: &[String], layout: &DockLayout, recent: &[String]) -> Vec<String> {
    let mut present: Vec<String> = layout.panels().into_iter().map(str::to_owned).collect();
    for side in DockSide::DOCKS {
        present.extend(layout.auto_hidden_panels(side).into_iter().map(str::to_owned));
    }
    present.extend(layout.floating_panels().into_iter().map(str::to_owned));
    present.retain(|id| known.contains(id));
    let mut order: Vec<String> = recent.iter().filter(|id| present.contains(id)).cloned().collect();
    for id in present {
        if !order.contains(&id) {
            order.push(id);
        }
    }
    order
}

/// The panel a change is about.
fn change_panel(change: &DockChange) -> Option<&String> {
    match change {
        DockChange::Activate { panel }
        | DockChange::Move { panel, .. }
        | DockChange::Close { panel }
        | DockChange::Reopen { panel }
        | DockChange::AutoHide { panel }
        | DockChange::Pin { panel }
        | DockChange::Float { panel }
        | DockChange::Dock { panel, .. }
        | DockChange::Place { panel } => Some(panel),
        _ => None,
    }
}

/// The least a floating panel gets.
const FLOAT_MIN: Size = Size::new(200.0, 120.0);
/// How close an edge draws a floating panel to it.
const MAGNET: f32 = 8.0;
/// How far each new floating panel opens from the one before.
const CASCADE: f32 = 24.0;
/// How thick the edges are that resize a floating panel.
const GRAB: f32 = 6.0;

/// How long the reason of a refusal shows, in seconds, unless another gesture comes first.
const NOTICE_SECONDS: f64 = 4.0;

/// What runs when the arrangement settles.
type OnEnd = Rc<dyn Fn(&DockLayout)>;

/// Decides whether a person may make a change, given the arrangement before and after it. When
/// they may not, the arrangement stays as it was, nothing is saved, and the reason shows.
pub type DockGuard = Rc<dyn Fn(&DockLayout, &DockLayout, &DockChange) -> Result<(), String>>;

/// What a panel of a [`DockArea`] is: a document goes in the center, a tool in the docks around it,
/// once the area has documents (see [`DockPanel::kind`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "variants"))]
pub enum DockPanelKind {
    /// Explorers, properties, output: around the work. What a panel is unless told.
    #[default]
    Tool,
    /// The work itself: what is edited or read, in the center.
    Document,
}

/// What closing a panel of a [`DockArea`] does.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "variants"))]
pub enum DockCloseAction {
    /// It closes and the menus offer it back. What closing does unless told.
    #[default]
    Hide,
    /// It closes for good: the menus do not offer it back, as a document closed is gone. The app
    /// can still bring it back through the arrangement.
    Remove,
}

/// A panel of a [`DockArea`]: what it is called and whether it can close. What it shows comes from
/// the function given to the area, by its id.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct DockPanel {
    /// What identifies it, in the arrangement and in what is saved. It should never change.
    pub id: String,
    /// What its tab says.
    pub title: String,
    /// An icon before its title.
    pub icon: Option<IconName>,
    /// It has a way to close (on by default).
    pub closable: bool,
    /// A document or a tool ([`DockPanelKind::Tool`] by default).
    pub kind: DockPanelKind,
    /// What closing it does ([`DockCloseAction::Hide`] by default).
    pub close_action: DockCloseAction,
    /// It has changes that are not saved (off by default).
    pub unsaved: bool,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl DockPanel {
    /// A panel with this id, whose tab says `title`.
    pub fn new(id: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            icon: None,
            closable: true,
            kind: DockPanelKind::Tool,
            close_action: DockCloseAction::Hide,
            unsaved: false,
        }
    }

    /// An icon before its title.
    pub fn icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
        self
    }

    /// Whether it has a way to close. The app can still close it through the arrangement.
    pub fn closable(mut self, closable: bool) -> Self {
        self.closable = closable;
        self
    }

    /// Whether it is a document or a tool. As soon as one panel of an area is a document, documents
    /// go only in the center and tools only in the docks: a drag marks nothing elsewhere, the menu
    /// does not offer it, and the keys say why. [`DockArea::free_placement`] lifts that.
    pub fn kind(mut self, kind: DockPanelKind) -> Self {
        self.kind = kind;
        self
    }

    /// Whether it has changes that are not saved: a dot on its tab, on its rail and on its header
    /// when it floats, and "unsaved" for screen readers. The app says it on every build.
    pub fn unsaved(mut self, unsaved: bool) -> Self {
        self.unsaved = unsaved;
        self
    }

    /// What closing it does: hidden, to be offered back, or gone from the menus.
    pub fn close_action(mut self, action: DockCloseAction) -> Self {
        self.close_action = action;
        self
    }
}

/// When a group of a [`DockArea`] shows its tabs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "variants"))]
pub enum DockTabBar {
    /// Always, even for one panel.
    #[default]
    Always,
    /// Only for more than one panel, leaving a panel alone with just its content.
    WhenSeveral,
    /// Never: the panels are columns with no title, which only change size and collapse.
    Never,
}

/// Panels in a tree of splits and groups of tabs, at the left, the right, the top, the bottom and
/// the center of a space. Bars between them resize with the pointer or the keyboard, docks collapse from the
/// strips on their edges, and panels close and come back.
///
/// A tab goes anywhere with a drag, with the target marked: to another group, to one side of a
/// group, or to a border that has no dock showing. Without a pointer, the arrows walk between tabs,
/// Alt with an arrow moves one within its group, Alt and Shift with an arrow send it to the dock on
/// that side (and with Enter, to the center), the menu of a tab or of the button at the end of the
/// strip has "Move to…", and Delete closes.
///
/// Before each change a person makes, [`on_change`](Self::on_change) asks the app, which can refuse
/// it and say why; a panel with unsaved work stays open that way.
///
/// The arrangement is a [`DockLayout`] in a [`Binding`], which holds ids alone and can be saved as
/// text. What a panel shows is asked from the app, by id, for the panel at the front of each group:
/// that is all a change of tab builds, and what a panel must remember while behind others belongs in
/// the app. An id in the arrangement with no [`DockPanel`] is left out.
///
/// ```
/// use ravenrs_reactive::State;
/// use ravenrs_widgets::{AnyView, DockArea, DockLayout, DockNode, DockPanel, Text, Ui};
///
/// let saved = None::<String>; // What the app kept the last time.
/// let start = DockLayout::new()
///     .center(DockNode::tabs(["photo"]))
///     .left(DockNode::tabs(["folders", "search"]), 240.0);
/// let layout = State::new(saved.and_then(|text| DockLayout::parse(&text).ok()).unwrap_or(start));
///
/// let area = DockArea::new(
///     layout.binding(),
///     vec![
///         DockPanel::new("photo", "Photo").closable(false),
///         DockPanel::new("folders", "Folders"),
///         DockPanel::new("search", "Search"),
///     ],
///     |id| -> AnyView { Box::new(Text::new(format!("Panel {id}"))) },
/// )
/// .on_end(|now| println!("{}", now.to_text()));
/// let mut ui = Ui::new(area, ravenrs_style::Theme::raven_dark());
/// ui.update();
/// ```
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct DockArea {
    layout: Binding<DockLayout>,
    panels: Vec<DockPanel>,
    content: Rc<dyn Fn(&str) -> AnyView>,
    height: Option<f32>,
    metrics: Metrics,
    movable: bool,
    auto_hide: bool,
    floatable: bool,
    free_placement: bool,
    on_documents_empty: Option<Rc<dyn Fn() -> AnyView>>,
    tab_bar: DockTabBar,
    on_end: Option<OnEnd>,
    guard: Option<DockGuard>,
    label: Option<String>,
}

impl fmt::Debug for DockArea {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DockArea").field("panels", &self.panels).finish_non_exhaustive()
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl DockArea {
    /// These panels in the arrangement of `layout`, each showing what `content` makes for its id.
    pub fn new(
        layout: Binding<DockLayout>,
        panels: Vec<DockPanel>,
        content: impl Fn(&str) -> AnyView + 'static,
    ) -> Self {
        Self {
            layout,
            panels,
            content: Rc::new(content),
            height: None,
            metrics: Metrics {
                bar: crate::extras::SPLIT_BAR,
                toggle: TOGGLE,
                left: (140.0, 640.0),
                right: (140.0, 640.0),
                bottom: (90.0, 560.0),
                top: (60.0, 400.0),
                center_min: (200.0, 120.0),
                pane_min: 100.0,
            },
            movable: true,
            auto_hide: true,
            floatable: true,
            free_placement: false,
            on_documents_empty: None,
            tab_bar: DockTabBar::Always,
            on_end: None,
            guard: None,
            label: None,
        }
    }

    /// Its height, in logical pixels. Without it, it takes the room its parent has.
    pub fn height(mut self, height: f32) -> Self {
        self.height = Some(height);
        self
    }

    /// The smallest and the largest a dock gets, in logical pixels.
    pub fn dock_limits(mut self, side: DockSide, min: f32, max: f32) -> Self {
        let limits = (min, max.max(min));
        match side {
            DockSide::Left => self.metrics.left = limits,
            DockSide::Right => self.metrics.right = limits,
            DockSide::Bottom => self.metrics.bottom = limits,
            DockSide::Top => self.metrics.top = limits,
            _ => {}
        }
        self
    }

    /// The width and the height the center keeps, which the docks do not take from it.
    pub fn center_min(mut self, width: f32, height: f32) -> Self {
        self.metrics.center_min = (width, height);
        self
    }

    /// The smallest a group gets when a bar inside a split moves.
    pub fn pane_min(mut self, min: f32) -> Self {
        self.metrics.pane_min = min;
        self
    }

    /// Whether panels change places (they do by default). Without it the tabs stay where the app
    /// put them, and only the sizes, the docks and closing are left.
    pub fn movable(mut self, movable: bool) -> Self {
        self.movable = movable;
        self
    }

    /// Whether the panels of the docks can hide at the edge of their dock (they can by default):
    /// a pin at the end of each strip of a dock, "Auto-hide" in the menu of its panels, and
    /// Ctrl+Alt+P (Cmd+Alt+P on a Mac). A hidden panel is a tab on a rail at that edge, which opens
    /// it over the rest, and its pin puts it back.
    pub fn auto_hide(mut self, on: bool) -> Self {
        self.auto_hide = on;
        self
    }

    /// Whether panels can float over the area in boxes of their own (they can by default): "Float"
    /// in the menu of a panel, Ctrl+Alt+F (Cmd+Alt+F on a Mac), or a tab dragged with Ctrl held or
    /// let go outside the area. A floating panel moves by its header, resizes by its edges, and
    /// goes back by its button, a double click on its header, Ctrl+Alt+F, or its header dragged
    /// onto the tabs of a group or the border of a dock.
    pub fn floatable(mut self, on: bool) -> Self {
        self.floatable = on;
        self
    }

    /// Lets documents and tools go anywhere, as when the area has no documents.
    pub fn free_placement(mut self, free: bool) -> Self {
        self.free_placement = free;
        self
    }

    /// What the center shows when nothing is in it, instead of a few words: a way to open or start
    /// a document. The panels that can come back are listed under it, as before.
    pub fn on_documents_empty(mut self, view: impl Fn() -> AnyView + 'static) -> Self {
        self.on_documents_empty = Some(Rc::new(view));
        self
    }

    /// When a group shows its tabs ([`DockTabBar::Always`] by default).
    pub fn tab_bar(mut self, tab_bar: DockTabBar) -> Self {
        self.tab_bar = tab_bar;
        self
    }

    /// Whether the strips that collapse the docks show (they do by default). Without them, the
    /// app does it with [`DockLayout::set_open`].
    pub fn edge_toggles(mut self, on: bool) -> Self {
        self.metrics.toggle = if on { TOGGLE } else { 0.0 };
        self
    }

    /// Runs when the arrangement settles: after a drop, a bar let go, a key, a tab, a dock or a
    /// panel. It is the moment to save.
    pub fn on_end(mut self, handler: impl Fn(&DockLayout) + 'static) -> Self {
        self.on_end = Some(Rc::new(handler));
        self
    }

    /// Asked before every change a person makes to the arrangement, with the arrangement before
    /// and after and what the change is; an `Err` refuses it and says why, beside the tab or at
    /// the top of the area, for a few seconds. A drag is asked once, when it is let go, and a
    /// refused bar goes back. What the app writes to the [`Binding`] itself is not asked.
    ///
    /// ```
    /// use ravenrs_reactive::State;
    /// use ravenrs_widgets::{AnyView, DockArea, DockChange, DockLayout, DockNode, DockPanel, Text};
    ///
    /// let layout = State::new(DockLayout::new().center(DockNode::tabs(["sales.rs"])));
    /// let unsaved = State::new(true);
    /// let area = DockArea::new(layout.binding(), vec![DockPanel::new("sales.rs", "sales.rs")], |id| -> AnyView {
    ///     Box::new(Text::new(id.to_owned()))
    /// })
    /// .on_change(move |_, _, change| match change {
    ///     DockChange::Close { panel } if unsaved.get() => Err(format!("{panel} has unsaved changes")),
    ///     _ => Ok(()),
    /// });
    /// ```
    pub fn on_change(
        mut self,
        guard: impl Fn(&DockLayout, &DockLayout, &DockChange) -> Result<(), String> + 'static,
    ) -> Self {
        self.guard = Some(Rc::new(guard));
        self
    }

    /// What screen readers call the area, instead of the name in [`Labels`].
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    fn panel(&self, id: &str) -> Option<&DockPanel> {
        self.panels.iter().find(|panel| panel.id == id)
    }

    fn title_of<'a>(&'a self, id: &'a str) -> &'a str {
        self.panel(id).map_or(id, |panel| panel.title.as_str())
    }

    fn strip_height(&self, panels: usize) -> f32 {
        strip_height(self.tab_bar, panels)
    }
}

fn strip_height(tab_bar: DockTabBar, panels: usize) -> f32 {
    match tab_bar {
        DockTabBar::Always => TAB_HEIGHT,
        DockTabBar::WhenSeveral if panels > 1 => TAB_HEIGHT,
        _ => 0.0,
    }
}

/// A key for something under the area that stays the same wherever it goes.
fn key_of(kind: u8, text: &str) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64 ^ u64::from(kind);
    for byte in text.bytes() {
        hash = (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3);
    }
    hash
}

/// Places a node exactly where the geometry says.
fn placed(rect: Rect) -> LayoutStyle {
    LayoutStyle {
        absolute: Some((rect.x(), rect.y())),
        width: Some(rect.width().max(0.0)),
        height: Some(rect.height().max(0.0)),
        min_width: Some(0.0),
        min_height: Some(0.0),
        align: Align::Stretch,
        ..LayoutStyle::stack(Axis::Vertical)
    }
}

/// Where things were last painted. What a drag aims at has to be compared with the pointer, and
/// a tab inside a strip that scrolls is known only then.
#[derive(Debug, Default)]
struct Frames {
    root: Cell<Rect>,
    tabs: RefCell<HashMap<String, Rect>>,
}

/// A tab being dragged.
#[derive(Clone, Debug, PartialEq)]
struct TabDrag {
    panel: String,
    pressed_at: Point,
    moving: bool,
    aim: Option<DropAim>,
    /// Where it would float if let go now, relative to the area.
    floating: Option<Rect>,
}

/// What the pointer does to a floating panel: moves it by its header, or resizes it by an edge.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Grab {
    left: bool,
    right: bool,
    top: bool,
    bottom: bool,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl Grab {
    const MOVE: Self = Self { left: true, right: true, top: true, bottom: true };

    fn moves(self) -> bool {
        self == Self::MOVE
    }
}

/// A floating panel being moved or resized.
#[derive(Clone, Debug, PartialEq)]
struct FloatDrag {
    panel: String,
    grab: Grab,
    pressed_at: Point,
    from: Rect,
    rect: Rect,
    aim: Option<DropAim>,
    /// Where it touches an edge it was drawn to.
    lines: Vec<Rect>,
}

/// Why the app refused the last change, and where it shows.
#[derive(Clone, Debug, PartialEq)]
struct Notice {
    reason: String,
    /// Its corner, relative to the area.
    at: Point,
    /// When it first showed, which its first build learns.
    since: Rc<Cell<Option<f64>>>,
}

/// Where each panel may go: documents to the center and tools to the docks, once the area has
/// documents and the app has not freed them.
#[derive(Clone, Default)]
struct Places {
    kinds: Rc<HashMap<String, DockPanelKind>>,
    enforced: bool,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl Places {
    fn of(area: &DockArea) -> Self {
        let kinds: HashMap<String, DockPanelKind> =
            area.panels.iter().map(|panel| (panel.id.clone(), panel.kind)).collect();
        let documents = kinds.values().any(|kind| *kind == DockPanelKind::Document);
        Self { kinds: Rc::new(kinds), enforced: documents && !area.free_placement }
    }

    /// Why this panel cannot go to that place, if it cannot.
    fn refusal(&self, labels: &Labels, id: &str, side: DockSide) -> Option<String> {
        if !self.enforced {
            return None;
        }
        let document = self.kinds.get(id) == Some(&DockPanelKind::Document);
        match (document, side == DockSide::Center) {
            (true, false) => Some(labels.dock_documents_center.clone()),
            (false, true) => Some(labels.dock_tools_docks.clone()),
            _ => None,
        }
    }
}

/// The place a drop lands in.
fn target_side(layout: &DockLayout, target: &DockDrop) -> Option<DockSide> {
    match target {
        DockDrop::Side(side) => Some(*side),
        DockDrop::Tab { beside, .. } | DockDrop::Split { beside, .. } => layout.side_of(beside),
    }
}

/// The hidden panel open over the rest, and whether the pointer opened it by resting on its tab.
#[derive(Clone, Debug, PartialEq)]
struct Opened {
    panel: String,
    by_hover: bool,
}

/// What the parts of the area change the arrangement through.
#[derive(Clone)]
struct Actions {
    layout: Binding<DockLayout>,
    on_end: Option<OnEnd>,
    guard: Option<DockGuard>,
    notice: State<Option<Notice>>,
    frames: Rc<Frames>,
    /// The arrangement as a bar's drag found it, until the drag is let go.
    resizing: Rc<RefCell<Option<DockLayout>>>,
    spoken: State<String>,
    /// The panel whose tab takes focus when it is next built.
    focus: Rc<RefCell<Option<String>>>,
    /// The hidden panel open over the rest.
    opened: State<Option<Opened>>,
    /// Whether focus goes into the panel that opens next.
    focus_open: Rc<Cell<bool>>,
    /// The tab of a rail the pointer is on.
    hover_tab: Rc<RefCell<Option<String>>>,
    places: Places,
    /// The panels, the most lately used first, for Ctrl+Tab. Not saved.
    recent: Rc<RefCell<Vec<String>>>,
    labels: Rc<Labels>,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl Actions {
    /// Makes a change when it changes something and the app lets it, saying whether it was made.
    fn apply(&self, change: DockChange, edit: impl FnOnce(&mut DockLayout) -> bool) -> bool {
        self.forget_notice();
        let before = self.layout.get();
        let mut next = before.clone();
        if !edit(&mut next) || !self.allowed(&before, &next, &change) {
            return false;
        }
        self.layout.set(next.clone());
        if let Some(on_end) = &self.on_end {
            on_end(&next);
        }
        if !matches!(change, DockChange::Close { .. })
            && let Some(panel) = change_panel(&change)
        {
            self.remember(panel);
        }
        true
    }

    /// Puts a panel first in the order of use, for Ctrl+Tab.
    fn remember(&self, id: &str) {
        let mut recent = self.recent.borrow_mut();
        recent.retain(|other| other != id);
        recent.insert(0, id.to_owned());
    }

    /// Brings a panel up wherever it is, for Ctrl+Tab: the front of its group, open over the
    /// rest if it is hidden, in front if it floats.
    fn go_to(&self, id: &str) {
        let layout = self.layout.get();
        if layout.is_auto_hidden(id) {
            self.focus_open.set(true);
            self.opened.set(Some(Opened { panel: id.to_owned(), by_hover: false }));
            self.remember(id);
        } else if layout.is_floating(id) {
            self.raise(id);
            *self.focus.borrow_mut() = Some(id.to_owned());
            self.remember(id);
        } else {
            *self.focus.borrow_mut() = Some(id.to_owned());
            let shown =
                self.apply(DockChange::Activate { panel: id.to_owned() }, |layout| layout.activate(id));
            if shown {
                self.remember(id);
            }
        }
    }

    /// Whether the app lets this change happen. A refusal shows its reason.
    fn allowed(&self, before: &DockLayout, after: &DockLayout, change: &DockChange) -> bool {
        let Some(guard) = &self.guard else { return true };
        match guard(before, after, change) {
            Ok(()) => true,
            Err(reason) => {
                let at = self.notice_at(change);
                self.notice.set(Some(Notice { reason, at, since: Rc::default() }));
                false
            }
        }
    }

    /// Where the reason of a refusal shows: under the tab of the panel it is about, or else at the
    /// top of the area, and never past its right edge.
    fn notice_at(&self, change: &DockChange) -> Point {
        let root = self.frames.root.get();
        let margin = 8.0;
        let panel = change_panel(change);
        let tab = panel.and_then(|panel| self.frames.tabs.borrow().get(panel).copied());
        let (x, y) = match tab {
            Some(tab) => (tab.x() - root.x(), tab.y() - root.y() + tab.height() + 4.0),
            None => (margin, margin),
        };
        // About as wide as a bubble gets, so it stays inside the area.
        let widest = 320.0;
        Point::new(x.min(root.width() - widest).max(margin), y)
    }

    /// Lets go of the reason of the last refusal, as a new gesture starts.
    fn forget_notice(&self) {
        self.notice.set_if_changed(None);
    }

    /// Writes a size as a bar moves, remembering the arrangement from before the first move.
    fn resize(&self, edit: impl FnOnce(&mut DockLayout)) {
        let mut layout = self.layout.get();
        {
            let mut resizing = self.resizing.borrow_mut();
            if resizing.is_none() {
                *resizing = Some(layout.clone());
                self.forget_notice();
            }
        }
        edit(&mut layout);
        self.layout.set(layout);
    }

    /// Ends what a bar changed: asks the app once, and either saves it or puts the sizes back.
    fn settle_resize(&self, side: DockSide) {
        let Some(before) = self.resizing.borrow_mut().take() else { return };
        let after = self.layout.get();
        if before == after {
            return;
        }
        if self.allowed(&before, &after, &DockChange::Resize { side }) {
            if let Some(on_end) = &self.on_end {
                on_end(&after);
            }
        } else {
            self.layout.set(before);
        }
    }

    /// Whether this panel may not go there; if not, the reason shows by its tab.
    fn refuses(&self, id: &str, target: &DockDrop) -> bool {
        let layout = self.layout.get();
        let reason =
            target_side(&layout, target).and_then(|side| self.places.refusal(&self.labels, id, side));
        let Some(reason) = reason else { return false };
        let at = self.notice_at(&DockChange::Move { panel: id.to_owned(), to: target.clone() });
        self.notice.set(Some(Notice { reason, at, since: Rc::default() }));
        true
    }

    /// Whether this panel may go there, without a word.
    fn may_go(&self, layout: &DockLayout, id: &str, target: &DockDrop) -> bool {
        target_side(layout, target).is_none_or(|side| self.places.refusal(&self.labels, id, side).is_none())
    }

    /// Sends a panel somewhere, keeping focus on its tab and saying where it went.
    fn send(&self, id: &str, target: DockDrop) -> bool {
        if self.refuses(id, &target) {
            return false;
        }
        let change = DockChange::Move { panel: id.to_owned(), to: target.clone() };
        let moved = self.apply(change, |layout| layout.move_panel(id, target));
        if moved {
            *self.focus.borrow_mut() = Some(id.to_owned());
            if let Some(side) = self.layout.get().side_of(id) {
                self.spoken.set(self.labels.dock_moved_to(side));
            }
        }
        moved
    }

    fn close(&self, id: &str, next: Option<String>, title: &str) -> bool {
        let closed = self.apply(DockChange::Close { panel: id.to_owned() }, |layout| layout.close(id));
        if closed {
            *self.focus.borrow_mut() = next;
            self.spoken.set(self.labels.dock_closed_name(title));
        }
        closed
    }

    fn reopen(&self, id: &str) {
        if self.apply(DockChange::Reopen { panel: id.to_owned() }, |layout| layout.reopen(id)) {
            *self.focus.borrow_mut() = Some(id.to_owned());
        }
    }

    /// Hides a panel at the edge of its dock, with focus going to its tab on the rail.
    fn hide(&self, id: &str, title: &str) -> bool {
        let side = self.layout.get().side_of(id);
        let hidden = self.apply(DockChange::AutoHide { panel: id.to_owned() }, |layout| layout.auto_hide(id));
        if hidden {
            *self.focus.borrow_mut() = Some(id.to_owned());
            if let Some(side) = side {
                self.spoken.set(self.labels.dock_hidden_name(title, side));
            }
        }
        hidden
    }

    /// Puts a hidden panel back in its dock, with focus on its tab there.
    fn pin(&self, id: &str, title: &str) -> bool {
        let pinned = self.apply(DockChange::Pin { panel: id.to_owned() }, |layout| layout.pin(id));
        if pinned {
            self.opened.set(None);
            *self.focus.borrow_mut() = Some(id.to_owned());
            self.spoken.set(self.labels.dock_pinned_name(title));
        }
        pinned
    }

    /// Lifts a panel out to float at `rect`, with focus on its header.
    fn float(&self, id: &str, title: &str, rect: Rect) -> bool {
        let floated = self.apply(DockChange::Float { panel: id.to_owned() }, |layout| layout.float(id, rect));
        if floated {
            self.opened.set(None);
            *self.focus.borrow_mut() = Some(id.to_owned());
            self.spoken.set(self.labels.dock_floating_name(title));
        }
        floated
    }

    /// Puts a floating panel back into the tree at `to`, with focus on its tab.
    fn dock(&self, id: &str, to: DockDrop) -> bool {
        if self.refuses(id, &to) {
            return false;
        }
        let change = DockChange::Dock { panel: id.to_owned(), to: to.clone() };
        let docked = self.apply(change, |layout| layout.move_panel(id, to));
        if docked {
            *self.focus.borrow_mut() = Some(id.to_owned());
            if let Some(side) = self.layout.get().side_of(id) {
                self.spoken.set(self.labels.dock_moved_to(side));
            }
        }
        docked
    }

    /// Puts a floating panel back where it came from.
    fn dock_home(&self, id: &str) -> bool {
        let home = self.layout.get().floating_home(id);
        home.is_some_and(|home| self.dock(id, home))
    }

    /// Moves or resizes a floating panel.
    fn place(&self, id: &str, rect: Rect) -> bool {
        self.apply(DockChange::Place { panel: id.to_owned() }, |layout| {
            let before = layout.floating_rect(id);
            layout.set_floating_rect(id, rect);
            before != layout.floating_rect(id)
        })
    }

    /// Brings a floating panel in front of the others. Not a change to ask about or to save at
    /// once: the order goes with the next change that is saved.
    fn raise(&self, id: &str) {
        let mut layout = self.layout.get();
        if layout.bring_to_front(id) {
            self.layout.set(layout);
        }
    }

    /// Opens a hidden panel over the rest, or lets it go back when it already shows.
    fn toggle_open(&self, id: &str) {
        if self.opened.peek().as_ref().is_some_and(|open| open.panel == id) {
            self.opened.set(None);
        } else {
            self.forget_notice();
            self.focus_open.set(true);
            self.opened.set(Some(Opened { panel: id.to_owned(), by_hover: false }));
        }
    }
}

/// Whether the key is the one that hides a panel or pins it back: Ctrl+Alt+P, or Cmd+Alt+P on a
/// Mac. With Alt, the P may come as another character, so the key without it counts too.
fn pin_key(key: &Key, modifiers: Modifiers) -> bool {
    let command = if crate::apple_keys() { modifiers.meta } else { modifiers.ctrl };
    let is_p = matches!(key, Key::Character(c) if c.eq_ignore_ascii_case("p"))
        || modifiers.base_key.is_some_and(|c| c.eq_ignore_ascii_case(&'p'));
    command && modifiers.alt && !modifiers.shift && is_p
}

/// Whether the key is the one that floats a panel or docks it back: Ctrl+Alt+F, or Cmd+Alt+F on a
/// Mac.
fn float_key(key: &Key, modifiers: Modifiers) -> bool {
    let command = if crate::apple_keys() { modifiers.meta } else { modifiers.ctrl };
    let is_f = matches!(key, Key::Character(c) if c.eq_ignore_ascii_case("f"))
        || modifiers.base_key.is_some_and(|c| c.eq_ignore_ascii_case(&'f'));
    command && modifiers.alt && !modifiers.shift && is_f
}

/// Where a panel floats when it is lifted out without a pointer: in the middle of the area, as
/// large as it was, at most half the area, and a step down and right of each that floats already.
fn float_box(size: Size, was: Size, floating: usize) -> Rect {
    let width = was.width.max(FLOAT_MIN.width).min((size.width / 2.0).max(FLOAT_MIN.width));
    let height = was.height.max(FLOAT_MIN.height).min((size.height / 2.0).max(FLOAT_MIN.height));
    let step = CASCADE * floating as f32;
    let rect =
        Rect::new((size.width - width) / 2.0 + step, (size.height - height) / 2.0 + step, width, height);
    geometry::clamp_floating(rect, size, FLOAT_MIN)
}

/// Whether a panel can hide at the edge of where it is: only in a dock.
fn can_hide(area: &DockArea, layout: &DockLayout, id: &str) -> bool {
    area.auto_hide && layout.side_of(id).is_some_and(|side| DockSide::DOCKS.contains(&side))
}

/// The items of a panel's menu.
fn panel_items(
    area: &DockArea,
    layout: &DockLayout,
    arranged: &Arranged,
    actions: &Actions,
    id: &str,
) -> Vec<MenuItem> {
    let labels = &actions.labels;
    let mut items = Vec::new();
    if area.movable {
        // These menus are made on every build, so what each place allows is read off the tree
        // rather than tried on a copy of it.
        let own_group = arranged.groups.iter().find(|group| group.panels.iter().any(|panel| panel == id));
        let mut places = Vec::new();
        // A place it may not go shows greyed, with why.
        let mut barred = Vec::new();
        for side in DockSide::ALL_IN_MENU {
            if layout.side_of(id) == Some(side) {
                continue;
            }
            match actions.places.refusal(labels, id, side) {
                None => places.push((labels.dock_place(side).to_owned(), DockDrop::Side(side))),
                Some(reason) => barred.push((labels.dock_place(side).to_owned(), reason)),
            }
        }
        for group in &arranged.groups {
            if own_group.is_some_and(|own| std::ptr::eq(own, group)) {
                continue;
            }
            let Some(first) = group.panels.first() else { continue };
            if !actions.may_go(layout, id, &DockDrop::Side(layout.side_of(first).unwrap_or(DockSide::Center)))
            {
                continue;
            }
            let shown = group.panels.get(group.active).unwrap_or(first);
            let target = DockDrop::Tab { beside: first.clone(), at: group.panels.len() };
            places.push((labels.dock_with_name(area.title_of(shown)), target));
        }
        if own_group.is_some_and(|own| own.panels.len() > 1) {
            let split = |edge| DockDrop::Split { beside: id.to_owned(), edge };
            places.push((labels.dock_split_right.clone(), split(DockEdge::Right)));
            places.push((labels.dock_split_below.clone(), split(DockEdge::Bottom)));
        }
        if !places.is_empty() || !barred.is_empty() {
            let mut item = MenuItem::new(labels.dock_move_to.clone());
            for (name, target) in places {
                let (actions, id) = (actions.clone(), id.to_owned());
                item = item.item(MenuItem::new(name).on_select(move || {
                    actions.send(&id, target.clone());
                }));
            }
            for (name, reason) in barred {
                item = item.item(MenuItem::new(name).disabled(reason));
            }
            items.push(item);
        }
    }
    if area.floatable && !layout.is_floating(id) {
        let (actions, owned, title) = (actions.clone(), id.to_owned(), area.title_of(id).to_owned());
        let was = arranged.groups.iter().find(|group| group.panels.iter().any(|panel| panel == id));
        let was = was.map_or(FLOAT_MIN, |group| group.rect.size);
        let rect = float_box(arranged.size, was, layout.floating_panels().len());
        items.push(MenuItem::new(labels.dock_float_item.clone()).on_select(move || {
            actions.float(&owned, &title, rect);
        }));
    }
    if can_hide(area, layout, id) {
        let (actions, id, title) = (actions.clone(), id.to_owned(), area.title_of(id).to_owned());
        items.push(MenuItem::new(labels.dock_auto_hide_item.clone()).on_select(move || {
            actions.hide(&id, &title);
        }));
    }
    if area.panel(id).is_some_and(|panel| panel.closable) {
        let (closer, owned, title) = (actions.clone(), id.to_owned(), area.title_of(id).to_owned());
        items.push(MenuItem::new(labels.dock_close.clone()).on_select(move || {
            closer.close(&owned, None, &title);
        }));
    }
    let document = area.panel(id).is_some_and(|panel| panel.kind == DockPanelKind::Document);
    let own_group = arranged.groups.iter().find(|group| group.panels.iter().any(|panel| panel == id));
    let others: Vec<(String, String)> = own_group
        .map(|group| {
            group
                .panels
                .iter()
                .filter(|other| *other != id && area.panel(other).is_some_and(|panel| panel.closable))
                .map(|other| (other.clone(), area.title_of(other).to_owned()))
                .collect()
        })
        .unwrap_or_default();
    if document && !others.is_empty() {
        let closer = actions.clone();
        items.push(MenuItem::new(labels.dock_close_others.clone()).on_select(move || {
            // One by one, each asked; the first the app keeps stops the rest.
            for (other, title) in &others {
                if !closer.close(other, None, title) {
                    break;
                }
            }
        }));
    }
    if let Some(reopen) = reopen_item(area, layout, actions) {
        items.push(reopen);
    }
    items
}

/// The submenu of what can come back, if anything can.
fn reopen_item(area: &DockArea, layout: &DockLayout, actions: &Actions) -> Option<MenuItem> {
    let closed = offered_back(area, layout);
    if closed.is_empty() {
        return None;
    }
    let mut item = MenuItem::new(actions.labels.dock_reopen.clone());
    for id in closed {
        let (actions, owned) = (actions.clone(), id.to_owned());
        item =
            item.item(MenuItem::new(area.title_of(id).to_owned()).on_select(move || actions.reopen(&owned)));
    }
    Some(item)
}

/// The closed panels the menus offer back: not those that close for good.
fn offered_back<'a>(area: &DockArea, layout: &'a DockLayout) -> Vec<&'a str> {
    let gone = |id: &str| area.panel(id).is_some_and(|panel| panel.close_action == DockCloseAction::Remove);
    layout.closed_panels().into_iter().filter(|id| !gone(id)).collect()
}

/// What a tab's drag does: it aims as it goes, and moves on release.
struct DragContext {
    actions: Actions,
    state: State<Option<TabDrag>>,
    frames: Rc<Frames>,
    arranged: Rc<Arranged>,
    tab_bar: DockTabBar,
    /// The keys held as it goes: with Ctrl it floats instead of landing.
    modifiers: Rc<Cell<Modifiers>>,
    /// Its title and its size, when it floats; none when the area does not float panels.
    floats: Option<(String, Size)>,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl DragContext {
    fn aim_at(&self, id: &str, position: Point) -> Option<DropAim> {
        let root = self.frames.root.get();
        let at = Point::new(position.x - root.x(), position.y - root.y());
        let tabs = self.frames.tabs.borrow();
        let tab_bar = self.tab_bar;
        let strip = move |group: &GroupPlace| strip_height(tab_bar, group.panels.len());
        let tab = |panel: &str| {
            tabs.get(panel).map(|f| Rect::new(f.x() - root.x(), f.y() - root.y(), f.width(), f.height()))
        };
        let found = geometry::aim(&self.arranged, at, &strip, &tab)?;
        let mut layout = self.actions.layout.get();
        // Where it may not go, and a drop that would leave things as they are, mark nothing.
        if !self.actions.may_go(&layout, id, &found.target) {
            return None;
        }
        layout.move_panel(id, found.target.clone()).then_some(found)
    }

    fn handler(self, id: String) -> crate::context::DragHandler {
        Rc::new(move |drag| match drag.phase {
            DragPhase::Start => self.state.set(Some(TabDrag {
                panel: id.clone(),
                pressed_at: drag.position,
                moving: false,
                aim: None,
                floating: None,
            })),
            DragPhase::Move => {
                // Nothing here when it was called off with Escape.
                let Some(mut now) = self.state.get() else { return };
                let (dx, dy) = (drag.position.x - now.pressed_at.x, drag.position.y - now.pressed_at.y);
                now.moving |= dx.hypot(dy) > reorder::DRAG_THRESHOLD;
                if now.moving {
                    let root = self.frames.root.get();
                    let held = self.modifiers.get();
                    let outside = !root.contains(drag.position);
                    match &self.floats {
                        Some((_, was)) if held.ctrl || held.meta || outside => {
                            let size = self.arranged.size;
                            let (width, height) = (was.width, was.height);
                            let at = Point::new(drag.position.x - root.x(), drag.position.y - root.y());
                            let rect = Rect::new(at.x - width / 2.0, at.y - 12.0, width, height);
                            now.aim = None;
                            now.floating = Some(geometry::clamp_floating(rect, size, FLOAT_MIN));
                        }
                        _ => {
                            now.aim = self.aim_at(&id, drag.position);
                            now.floating = None;
                        }
                    }
                }
                self.state.set_if_changed(Some(now));
            }
            DragPhase::End => {
                if let Some(done) = self.state.get() {
                    if let Some(aim) = done.aim {
                        self.actions.send(&id, aim.target);
                    } else if let (Some(rect), Some((title, _))) = (done.floating, &self.floats) {
                        self.actions.float(&id, title, rect);
                    }
                }
                self.state.set(None);
            }
            // The base may add more; they act as nothing here.
            _ => {}
        })
    }
}

/// A tab.
struct TabView<'a> {
    area: &'a DockArea,
    panel: &'a DockPanel,
    group: &'a GroupPlace,
    index: usize,
    on: bool,
    actions: &'a Actions,
    frames: &'a Rc<Frames>,
    drag: &'a State<Option<TabDrag>>,
    arranged: &'a Rc<Arranged>,
    roving: Roving,
}

impl View for TabView<'_> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let labels = cx.labels();
        let (panel, on) = (self.panel, self.on);
        let hovered = cx.is_hover_within();
        let dragged = self.drag.get().is_some_and(|d| d.moving && d.panel == panel.id);
        let ink = theme.color(if on { Semantic::Ink } else { Semantic::InkMuted });
        let attrs = util::attrs(&theme, TextStyle::Callout, None);
        let title_width = (cx.measure_text(&panel.title, &attrs).width.ceil() + 1.0).min(TITLE_MAX_WIDTH);
        let gap = theme.spacing.s2;
        let right_pad = if panel.closable { theme.spacing.s1 } else { theme.spacing.s3 };
        let mut width = theme.spacing.s3 + right_pad + title_width;
        let mut parts = Vec::with_capacity(3);
        if let Some(icon) = panel.icon {
            parts.push(util::icon(cx, icon, 14.0, ink));
            width += 14.0 + gap;
        }
        let title = cx.text_truncated(
            &panel.title,
            &attrs,
            ink,
            LayoutStyle { width: Some(title_width), min_width: Some(0.0), ..LayoutStyle::leaf() },
        );
        cx.semantics(title, Semantics::hidden());
        parts.push(title);
        if panel.unsaved {
            parts.push(unsaved_dot(cx));
            width += UNSAVED_DOT + gap;
        }
        let (id, group_panels) = (panel.id.clone(), &self.group.panels);
        // Whoever is beside it takes focus when it closes from the keyboard.
        let next = group_panels
            .get(self.index + 1)
            .or_else(|| self.index.checked_sub(1).and_then(|i| group_panels.get(i)))
            .cloned();
        if panel.closable {
            let (actions, id, title) = (self.actions.clone(), id.clone(), panel.title.clone());
            width += ButtonSize::Mini.height() + gap;
            let button = IconButton::new(IconName::X)
                .label(labels.dock_close_name(&panel.title))
                .size(ButtonSize::Mini)
                .on_click(move || {
                    actions.close(&id, None, &title);
                });
            parts.push(cx.child(&button));
        }
        let fill = if dragged {
            theme.color(Semantic::Accent).with_alpha(0.16)
        } else if on {
            theme.color(Semantic::Surface)
        } else if hovered {
            theme.color(Semantic::Ink).with_alpha(0.05)
        } else {
            Color::TRANSPARENT
        };
        let radius = theme.radii.sm;
        let (frames, recorded) = (self.frames.clone(), panel.id.clone());
        let node = cx.container(
            LayoutStyle {
                height: Some(TAB_HEIGHT),
                width: Some(width),
                min_width: Some(width),
                spacing: gap,
                padding: Insets { top: 0.0, bottom: 0.0, left: theme.spacing.s3, right: right_pad },
                align: Align::Center,
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &parts,
            Paint::Custom(Rc::new(move |frame, scene| {
                frames.tabs.borrow_mut().insert(recorded.clone(), frame);
                if fill.a > 0.0 {
                    scene.push_quad(Quad::new(frame, fill).radius(radius));
                }
            })),
        );
        let (actions, to_show) = (self.actions.clone(), id.clone());
        cx.on_click(
            node,
            Rc::new(move || {
                let change = DockChange::Activate { panel: to_show.clone() };
                actions.apply(change, |layout| !layout.is_active(&to_show) && layout.activate(&to_show));
            }),
        );
        cx.cursor(node, CursorIcon::Pointer);
        cx.focusable(node, radius);
        let semantics = Semantics::new(Role::Tab).name(panel.title.clone()).selected(on);
        cx.semantics(
            node,
            if panel.unsaved { semantics.description(labels.unsaved.clone()) } else { semantics },
        );
        cx.test_id(node, &format!("panel:{id}"));
        if self.actions.focus.borrow().as_deref() == Some(id.as_str()) {
            self.actions.focus.borrow_mut().take();
            cx.request_focus_node(node);
        }
        let movable = self.area.movable;
        if movable {
            let was = float_box(self.arranged.size, self.group.rect.size, 0).size;
            let context = DragContext {
                actions: self.actions.clone(),
                state: self.drag.clone(),
                frames: self.frames.clone(),
                arranged: self.arranged.clone(),
                tab_bar: self.area.tab_bar,
                modifiers: cx.modifiers(),
                floats: self.area.floatable.then(|| (panel.title.clone(), was)),
            };
            cx.on_drag(node, context.handler(id.clone()));
        }
        let (index, count) = (self.index, group_panels.len());
        let first = group_panels.first().cloned().unwrap_or_default();
        let (actions, called_off, closable) = (self.actions.clone(), self.drag.clone(), panel.closable);
        let title = panel.title.clone();
        let key_id = id.clone();
        let hides = can_hide(self.area, &self.actions.layout.get(), &id);
        let floats = self.area.floatable.then(|| {
            let floating = self.actions.layout.get().floating_panels().len();
            float_box(self.arranged.size, self.group.rect.size, floating)
        });
        let before: crate::KeyHandler = Rc::new(move |key, modifiers| {
            if *key == Key::Escape && called_off.peek().is_some() {
                called_off.set(None);
                return true;
            }
            if hides && pin_key(key, modifiers) {
                actions.hide(&key_id, &title);
                return true;
            }
            if let Some(rect) = floats
                && float_key(key, modifiers)
            {
                actions.float(&key_id, &title, rect);
                return true;
            }
            if *key == Key::Delete && closable {
                actions.close(&key_id, next.clone(), &title);
                return true;
            }
            if !movable {
                return false;
            }
            if let Some(side) = dock_key(key, modifiers) {
                actions.send(&key_id, DockDrop::Side(side));
                return true;
            }
            if let Some(to) = reorder::key_target(key, modifiers, index, count, false) {
                // A gap, as the layout counts them: past the tab itself it is one more.
                let at = if to > index { to + 1 } else { to };
                if actions.send(&key_id, DockDrop::Tab { beside: first.clone(), at }) {
                    actions.spoken.set(actions.labels.moved_to_position(to + 1, count));
                }
            }
            reorder::is_move_key(key, modifiers)
        });
        let (actions, panels) = (self.actions.clone(), group_panels.clone());
        let opens: Rc<dyn Fn(usize)> = Rc::new(move |to| {
            if let Some(id) = panels.get(to) {
                actions.apply(DockChange::Activate { panel: id.clone() }, |layout| layout.activate(id));
            }
        });
        self.roving.wire_with(cx, node, Some(opens), Some(before));
        node
    }
}

/// Which place Alt and Shift with an arrow send a panel to, or with Enter, the center.
fn dock_key(key: &Key, modifiers: Modifiers) -> Option<DockSide> {
    if !(modifiers.alt && modifiers.shift) {
        return None;
    }
    match key {
        Key::ArrowLeft => Some(DockSide::Left),
        Key::ArrowRight => Some(DockSide::Right),
        Key::ArrowDown => Some(DockSide::Bottom),
        Key::ArrowUp => Some(DockSide::Top),
        Key::Enter => Some(DockSide::Center),
        _ => None,
    }
}

/// A tab with its menu.
struct TabWithMenu<'a> {
    strip: &'a Strip<'a>,
    index: usize,
    focus_to: FocusTarget,
}

impl View for TabWithMenu<'_> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let strip = self.strip;
        let Some(panel) = strip.group.panels.get(self.index).and_then(|id| strip.area.panel(id)) else {
            return cx.empty();
        };
        let view = TabView {
            area: strip.area,
            panel,
            group: strip.group,
            index: self.index,
            on: self.index == strip.group.active,
            actions: strip.actions,
            frames: strip.frames,
            drag: strip.drag,
            arranged: strip.arranged,
            roving: Roving {
                index: self.index,
                count: strip.group.panels.len(),
                focus_to: self.focus_to.clone(),
            },
        };
        let mut menu = ContextMenu::new(view);
        for item in panel_items(strip.area, strip.layout, strip.arranged, strip.actions, &panel.id) {
            menu = menu.item(item);
        }
        menu.build(cx)
    }
}

/// The tabs of a group, and the button with the menu of the one that shows.
struct Strip<'a> {
    area: &'a DockArea,
    group: &'a GroupPlace,
    layout: &'a DockLayout,
    actions: &'a Actions,
    frames: &'a Rc<Frames>,
    drag: &'a State<Option<TabDrag>>,
    arranged: &'a Rc<Arranged>,
}

impl View for Strip<'_> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let labels = cx.labels();
        let focus_to = util::roving_cell(cx, None);
        let tabs: Vec<Keyed<TabWithMenu<'_>>> = self
            .group
            .panels
            .iter()
            .enumerate()
            .map(|(index, id)| {
                Keyed::new(
                    key_of(KEY_PANEL, id),
                    TabWithMenu { strip: self, index, focus_to: focus_to.clone() },
                )
            })
            .collect();
        let tabs =
            cx.child(&ScrollEdgeArrow::new(tabs).spacing(2.0).fade(Semantic::Canvas).height(TAB_HEIGHT));
        let tabs = cx.container(
            LayoutStyle {
                grow: 1.0,
                min_width: Some(0.0),
                align: Align::Stretch,
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &[tabs],
            Paint::None,
        );
        cx.semantics(tabs, Semantics::new(Role::TabList).name(labels.dock_tabs_name.clone()));
        let mut nodes = vec![tabs];
        let shown = self.group.panels.get(self.group.active).map(String::as_str).unwrap_or_default();
        if can_hide(self.area, self.layout, shown) {
            let title = self.area.title_of(shown).to_owned();
            let (actions, id, named) = (self.actions.clone(), shown.to_owned(), title.clone());
            let pin = IconButton::new(IconName::Pin)
                .label(labels.dock_auto_hide_name(&title))
                .style(ButtonStyle::Ghost)
                .size(ButtonSize::Mini)
                .on_click(move || {
                    actions.hide(&id, &named);
                });
            nodes.push(cx.child(&pin));
        }
        let items = panel_items(self.area, self.layout, self.arranged, self.actions, shown);
        if !items.is_empty() {
            let entries: Vec<Entry> = items.into_iter().map(Entry::Item).collect();
            nodes.push(
                cx.child(&Options { entries, name: labels.dock_options_name(self.area.title_of(shown)) }),
            );
        }
        cx.container(
            LayoutStyle {
                height: Some(TAB_HEIGHT),
                min_width: Some(0.0),
                align: Align::Center,
                spacing: theme.spacing.s1,
                padding: Insets { top: 0.0, bottom: 0.0, left: 0.0, right: theme.spacing.s1 },
                align_self: Some(Align::Stretch),
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &nodes,
            Paint::Quad(QuadPaint::new(theme.color(Semantic::Canvas))),
        )
    }
}

/// The button at the end of a strip, so that all a tab's menu has is reached from the keyboard.
struct Options {
    entries: Vec<Entry>,
    name: String,
}

impl View for Options {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        menu_trigger(
            cx,
            &self.entries,
            IconName::Ellipsis,
            ButtonStyle::Ghost,
            ButtonSize::Mini,
            self.name.clone(),
        )
    }
}

/// The strip that collapses a dock and brings it back.
struct EdgeToggle {
    rect: Rect,
    side: DockSide,
    open: bool,
    actions: Actions,
}

impl View for EdgeToggle {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let labels = cx.labels();
        let lit = cx.is_hovered() || cx.is_focus_visible();
        // It points the way the dock will go: out to hide, in to show.
        let icon = match (self.side, self.open) {
            (DockSide::Left, true) | (DockSide::Right, false) => IconName::ChevronLeft,
            (DockSide::Left, false) | (DockSide::Right, true) => IconName::ChevronRight,
            (DockSide::Top, true) => IconName::ChevronUp,
            (DockSide::Top, false) => IconName::ChevronDown,
            (_, true) => IconName::ChevronDown,
            (_, false) => IconName::ChevronUp,
        };
        let ink = theme.color(if lit { Semantic::Ink } else { Semantic::InkMuted });
        let glyph = util::icon(cx, icon, 10.0, ink);
        cx.semantics(glyph, Semantics::hidden());
        let fill = if lit { theme.color(Semantic::Ink).with_alpha(0.06) } else { Color::TRANSPARENT };
        let node = cx.container(
            LayoutStyle { align: Align::Center, justify: Justify::Center, ..placed(self.rect) },
            &[glyph],
            Paint::Quad(QuadPaint::new(fill)),
        );
        let (actions, side, open) = (self.actions.clone(), self.side, self.open);
        cx.on_click(
            node,
            Rc::new(move || {
                actions.apply(DockChange::Collapse { side, open: !open }, |layout| {
                    layout.set_open(side, !open);
                    true
                });
            }),
        );
        cx.cursor(node, CursorIcon::Pointer);
        cx.focusable(node, 2.0);
        let name = if open { labels.dock_hide_name(self.side) } else { labels.dock_show_name(self.side) };
        cx.semantics(node, Semantics::new(Role::Button).name(name).expanded(open));
        node
    }
}

/// What shows when the center holds nothing: the words, and a way back for each closed panel.
struct EmptyCenter<'a> {
    area: &'a DockArea,
    layout: &'a DockLayout,
    actions: &'a Actions,
    rect: Rect,
}

impl View for EmptyCenter<'_> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let labels = cx.labels();
        let attrs = util::attrs(&theme, TextStyle::Callout, None);
        let words = match &self.area.on_documents_empty {
            Some(view) => {
                let view = view();
                cx.child(view.as_ref())
            }
            None => cx.text(&labels.dock_empty, &attrs, theme.color(Semantic::InkMuted), LayoutStyle::leaf()),
        };
        let mut buttons = Vec::new();
        for id in offered_back(self.area, self.layout) {
            let (actions, owned) = (self.actions.clone(), id.to_owned());
            let button = Button::new(self.area.title_of(id).to_owned())
                .style(ButtonStyle::Secondary)
                .size(ButtonSize::Small)
                .on_click(move || actions.reopen(&owned));
            buttons.push(cx.keyed_child(key_of(KEY_PANEL, id), &button));
        }
        let row = cx.container(
            LayoutStyle {
                wrap: true,
                spacing: theme.spacing.s2,
                justify: Justify::Center,
                align_self: Some(Align::Stretch),
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &buttons,
            Paint::None,
        );
        cx.container(
            LayoutStyle {
                align: Align::Center,
                justify: Justify::Center,
                spacing: theme.spacing.s3,
                padding: Insets::uniform(theme.spacing.s5),
                ..placed(self.rect)
            },
            &[words, row],
            Paint::None,
        )
    }
}

impl View for DockArea {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let labels = cx.labels();
        let drag: State<Option<TabDrag>> = cx.use_state(|| None);
        let float_drag: State<Option<FloatDrag>> = cx.use_state(|| None);
        let switch: State<Option<Switch>> = cx.use_state(|| None);
        let recent: Rc<RefCell<Vec<String>>> =
            cx.use_state(|| Rc::new(RefCell::new(Vec::new()))).peek().clone();
        let spoken: State<String> = cx.use_state(String::new);
        let frames: Rc<Frames> = cx.use_state(|| Rc::new(Frames::default())).peek().clone();
        let focus: Rc<RefCell<Option<String>>> = cx.use_state(|| Rc::new(RefCell::new(None))).peek().clone();
        let known: Vec<&str> = self.panels.iter().map(|panel| panel.id.as_str()).collect();
        let mut layout = self.layout.get();
        // From here on what was saved fits what exists, and it is saved that way next time.
        if layout.reconcile(&known) {
            self.layout.set(layout.clone());
        }
        let notice: State<Option<Notice>> = cx.use_state(|| None);
        let opened: State<Option<Opened>> = cx.use_state(|| None);
        let focus_open: Rc<Cell<bool>> = cx.use_state(|| Rc::new(Cell::new(false))).peek().clone();
        let hover_tab: Rc<RefCell<Option<String>>> =
            cx.use_state(|| Rc::new(RefCell::new(None))).peek().clone();
        let resizing: Rc<RefCell<Option<DockLayout>>> =
            cx.use_state(|| Rc::new(RefCell::new(None))).peek().clone();
        let actions = Actions {
            layout: self.layout.clone(),
            on_end: self.on_end.clone(),
            guard: self.guard.clone(),
            notice: notice.clone(),
            frames: frames.clone(),
            resizing,
            spoken: spoken.clone(),
            focus,
            opened: opened.clone(),
            focus_open,
            hover_tab,
            places: Places::of(self),
            recent: recent.clone(),
            labels: labels.clone(),
        };
        // Known from the second build on; until then the window is the best guess there is.
        let size = cx.scroll_viewport().unwrap_or_else(|| {
            let window = cx.viewport();
            Size::new(window.width, self.height.unwrap_or(window.height))
        });
        let arranged = Rc::new(geometry::arrange(&layout, size, &self.metrics));
        let mut nodes = Vec::with_capacity(arranged.groups.len() * 2 + arranged.bars.len() + 5);
        for group in &arranged.groups {
            nodes.push(self.build_group(cx, group, &layout, &actions, &frames, &drag, &arranged));
        }
        for bar in &arranged.bars {
            nodes.push(self.build_bar(cx, bar, &actions));
        }
        for (side, rect) in &arranged.toggles {
            let toggle = EdgeToggle {
                rect: *rect,
                side: *side,
                open: layout.is_open(*side),
                actions: actions.clone(),
            };
            nodes.push(cx.keyed_child(key_of(KEY_TOGGLE, labels.dock_place(*side)), &toggle));
        }
        let mut rails = Vec::with_capacity(arranged.rails.len());
        for (side, rect) in &arranged.rails {
            let rail = Rail {
                area: self,
                side: *side,
                rect: *rect,
                layout: &layout,
                actions: &actions,
                frames: &frames,
                drag: &drag,
                arranged: &arranged,
            };
            let node = cx.keyed_child(key_of(KEY_RAIL, labels.dock_place(*side)), &rail);
            nodes.push(node);
            rails.push((*side, node, *rect));
        }
        self.open_hidden(cx, &layout, &actions, &rails, size);
        if layout.node(DockSide::Center).is_none() {
            let empty = EmptyCenter { area: self, layout: &layout, actions: &actions, rect: arranged.center };
            nodes.push(cx.child(&empty));
        }
        let floating = layout.floating_panels();
        let shown: Vec<(String, Rect)> = floating
            .iter()
            .filter_map(|id| {
                let rect = layout.floating_rect(id)?;
                Some(((*id).to_owned(), geometry::clamp_floating(rect, size, FLOAT_MIN)))
            })
            .collect();
        for (id, rect) in &shown {
            let others: Vec<Rect> = shown.iter().filter(|(other, _)| other != id).map(|(_, r)| *r).collect();
            let view = FloatingView {
                area: self,
                id: id.clone(),
                rect: *rect,
                others,
                actions: &actions,
                arranged: &arranged,
                layout: &layout,
                frames: &frames,
                float_drag: &float_drag,
            };
            nodes.push(cx.keyed_child(key_of(KEY_FLOATING, id), &view));
        }
        if let Some(aim) = drag.get().and_then(|d| d.aim) {
            nodes.push(mark(cx, aim.mark));
        }
        if let Some(drag) = float_drag.get() {
            if let Some(aim) = &drag.aim {
                nodes.push(mark(cx, aim.mark));
            }
            // The edges it was drawn to, in the color of a warning, while it moves.
            let warning = theme.color(Semantic::Warning);
            for line in &drag.lines {
                let node = cx.container(placed(*line), &[], Paint::Quad(QuadPaint::new(warning)));
                cx.semantics(node, Semantics::hidden());
                nodes.push(node);
            }
        }
        if let Some(rect) = drag.get().and_then(|d| d.floating) {
            nodes.push(float_outline(cx, rect));
        }
        self.switch_panels(cx, &layout, &actions, &switch, &mut nodes);
        // Why the app refused the last change, until the next gesture or a few seconds on.
        if let Some(shown) = notice.get() {
            let now = cx.now();
            let since = shown.since.get().unwrap_or_else(|| {
                shown.since.set(Some(now));
                now
            });
            if now >= since + NOTICE_SECONDS {
                notice.set(None);
            } else {
                cx.wake_at(since + NOTICE_SECONDS);
                let (tone, ink) = (theme.color(Semantic::Warning), theme.color(Semantic::Ink));
                let fill = theme.color(Semantic::SurfaceElevated);
                nodes.push(util::bubble(
                    cx,
                    IconName::TriangleAlert,
                    tone,
                    &shown.reason,
                    shown.at,
                    ink,
                    fill,
                ));
            }
        }
        let announcement = spoken.get();
        nodes.push(reorder::announcer(cx, announcement));
        let recorded = frames.clone();
        let node = cx.container(
            LayoutStyle {
                height: self.height,
                grow: if self.height.is_none() { 1.0 } else { 0.0 },
                min_width: Some(0.0),
                min_height: Some(0.0),
                align_self: Some(Align::Stretch),
                overflow_hidden: true,
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &nodes,
            Paint::Custom(Rc::new(move |frame, scene| {
                recorded.root.set(frame);
                scene.push_quad(Quad::new(frame, theme.color(Semantic::Canvas)));
            })),
        );
        cx.report_frame(node);
        cx.clip(node);
        let name = self.label.clone().unwrap_or_else(|| labels.dock_area_name.clone());
        cx.semantics(node, Semantics::new(Role::Group).name(name));
        node
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose(part = "part1"))]
impl DockArea {
    /// Ctrl+Tab and Ctrl+Shift+Tab: a quick press goes back to the panel used before, a longer one
    /// shows the list, Tab walks it while Ctrl is held, and letting Ctrl go opens what is chosen.
    fn switch_panels(
        &self,
        cx: &mut BuildCx<'_>,
        layout: &DockLayout,
        actions: &Actions,
        switch: &State<Option<Switch>>,
        nodes: &mut Vec<NodeId>,
    ) {
        let known: Vec<String> = self.panels.iter().map(|panel| panel.id.clone()).collect();
        let step = {
            let (switch, binding, recent) = (switch.clone(), self.layout.clone(), actions.recent.clone());
            move |by: isize| {
                let now = switch.get();
                match now {
                    None => {
                        let order = switch_order(&known, &binding.get(), &recent.borrow());
                        if order.len() < 2 {
                            return;
                        }
                        let index = if by > 0 { 1 } else { order.len() - 1 };
                        switch.set(Some(Switch { order, index, since: None, shown: false }));
                    }
                    Some(mut going) => {
                        let count = going.order.len() as isize;
                        going.index = (going.index as isize + by).rem_euclid(count) as usize;
                        going.shown = true;
                        switch.set(Some(going));
                    }
                }
            }
        };
        let step = Rc::new(step);
        let back = step.clone();
        // With Shift first: the plain one would take Shift too.
        cx.shortcut_with_shift(Key::Tab, true, Rc::new(move || back(-1)));
        cx.shortcut(Key::Tab, true, Rc::new(move || step(1)));
        let Some(mut going) = switch.get() else { return };
        for command in [true, false] {
            let stop = switch.clone();
            cx.shortcut(Key::Escape, command, Rc::new(move || stop.set(None)));
        }
        let held = cx.held();
        let now = cx.now();
        let since = *going.since.get_or_insert(now);
        if !(held.ctrl || held.meta) {
            switch.set(None);
            if let Some(id) = going.order.get(going.index) {
                actions.go_to(id);
            }
            return;
        }
        if !going.shown && now >= since + QUICK_SWITCH {
            going.shown = true;
        }
        if !going.shown {
            cx.wake_at(since + QUICK_SWITCH);
        }
        if switch.peek().as_ref() != Some(&going) {
            switch.set(Some(going.clone()));
        }
        if going.shown {
            let window = cx.viewport();
            let rows = going.order.len() as f32 + 1.0;
            let height = rows * 38.0 + 32.0;
            let at =
                Point::new((window.width - SWITCHER_WIDTH) / 2.0, ((window.height - height) / 2.0).max(16.0));
            let anchor = cx.empty();
            nodes.push(anchor);
            let list = Switcher { area: self, layout, switch: going, state: switch.clone(), actions };
            let stop = switch.clone();
            cx.overlay(anchor, Placement::At(at), &list, Some(Rc::new(move || stop.set(None))));
        }
    }

    /// The hidden panel that is open, over the rest beside its rail, if it is still hidden.
    fn open_hidden(
        &self,
        cx: &mut BuildCx<'_>,
        layout: &DockLayout,
        actions: &Actions,
        rails: &[(DockSide, NodeId, Rect)],
        size: Size,
    ) {
        let Some(open) = actions.opened.get() else { return };
        let side = layout.auto_hidden_side(&open.panel);
        let rail = side.and_then(|side| rails.iter().find(|(s, ..)| *s == side));
        let (Some(side), Some((_, anchor, rect)), Some(width)) =
            (side, rail, layout.auto_hidden_size(&open.panel))
        else {
            actions.opened.set(None);
            return;
        };
        let (length, room) = match side {
            DockSide::Top | DockSide::Bottom => (rect.width(), size.height),
            _ => (rect.height(), size.width),
        };
        let most = (room - OPEN_MARGIN).max(OPEN_MIN);
        let placement = match side {
            DockSide::Right => Placement::LeftStart,
            DockSide::Top => Placement::Below,
            DockSide::Bottom => Placement::Above,
            _ => Placement::Right,
        };
        let open_panel = OpenPanel {
            area: self,
            id: open.panel.clone(),
            side,
            size: width.clamp(OPEN_MIN, most),
            length,
            most,
            by_hover: open.by_hover,
            actions: actions.clone(),
        };
        let opened = actions.opened.clone();
        cx.flush_overlay(*anchor, placement, &open_panel, Some(Rc::new(move || opened.set(None))));
    }

    #[allow(clippy::too_many_arguments, reason = "the parts of one build, private to it")]
    fn build_group(
        &self,
        cx: &mut BuildCx<'_>,
        group: &GroupPlace,
        layout: &DockLayout,
        actions: &Actions,
        frames: &Rc<Frames>,
        drag: &State<Option<TabDrag>>,
        arranged: &Rc<Arranged>,
    ) -> NodeId {
        let theme = *cx.theme();
        let mut parts = Vec::with_capacity(2);
        let first = group.panels.first().map(String::as_str).unwrap_or_default();
        if self.strip_height(group.panels.len()) > 0.0 {
            let strip = Strip { area: self, group, layout, actions, frames, drag, arranged };
            parts.push(cx.keyed_child(key_of(KEY_STRIP, first), &strip));
        }
        let shown = group.panels.get(group.active).map(String::as_str).unwrap_or_default();
        // Under the area itself and by id: what a panel keeps goes with it to any group.
        let content = (self.content)(shown);
        let content = cx.keyed_child(key_of(KEY_PANEL, shown), content.as_ref());
        let body = cx.container(
            LayoutStyle {
                grow: 1.0,
                height: Some(0.0),
                min_height: Some(0.0),
                min_width: Some(0.0),
                align: Align::Stretch,
                overflow_hidden: true,
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &[content],
            Paint::Quad(QuadPaint::new(theme.color(Semantic::Surface))),
        );
        cx.clip(body);
        cx.semantics(body, Semantics::new(Role::TabPanel).name(self.title_of(shown).to_owned()));
        parts.push(body);
        let node = cx.container(placed(group.rect), &parts, Paint::None);
        cx.clip(node);
        node
    }

    fn build_bar(&self, cx: &mut BuildCx<'_>, bar: &BarPlace, actions: &Actions) -> NodeId {
        let labels = cx.labels();
        let value = bar.value;
        let (name, key, inverted) = match &bar.kind {
            BarKind::Dock(side) => (
                labels.dock_resize_name(*side),
                format!("{side:?}"),
                matches!(side, DockSide::Right | DockSide::Bottom),
            ),
            BarKind::Split { side, path, index, names, .. } => (
                labels.dock_resize_between(self.title_of(&names.0), self.title_of(&names.1)),
                format!("{side:?}{path:?}{index}"),
                false,
            ),
        };
        let (kind, writer) = (bar.kind.clone(), actions.clone());
        let side = match &bar.kind {
            BarKind::Dock(side) | BarKind::Split { side, .. } => *side,
        };
        let binding = Binding::new(
            move || value,
            move |next: f32| {
                writer.resize(|layout| match &kind {
                    BarKind::Dock(side) => layout.set_size(*side, next),
                    BarKind::Split { side, path, index, both, .. } => {
                        layout.resize_split(*side, path, *index, next, *both);
                    }
                });
            },
        );
        let mut handle = SplitHandle::new(bar.axis, binding).range(bar.min, bar.max).label(name);
        if inverted {
            handle = handle.inverted();
        }
        let actions = actions.clone();
        let handle = handle.on_end(move || actions.settle_resize(side));
        let handle = cx.keyed_child(key_of(KEY_BAR, &key), &handle);
        // The bar stretches across its stack, so the stack goes the way the bar's own axis does.
        let style = LayoutStyle { axis: Some(bar.axis), ..placed(bar.rect) };
        cx.container(style, &[handle], Paint::None)
    }
}

/// The rail at the edge of a dock, with a tab for each panel hidden there.
struct Rail<'a> {
    area: &'a DockArea,
    side: DockSide,
    rect: Rect,
    layout: &'a DockLayout,
    actions: &'a Actions,
    frames: &'a Rc<Frames>,
    drag: &'a State<Option<TabDrag>>,
    arranged: &'a Rc<Arranged>,
}

impl View for Rail<'_> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let labels = cx.labels();
        let panels = self.layout.auto_hidden_panels(self.side);
        let focus_to = util::roving_cell(cx, None);
        let count = panels.len();
        let tabs: Vec<Keyed<RailTab<'_>>> = panels
            .iter()
            .enumerate()
            .map(|(index, id)| {
                let roving = Roving { index, count, focus_to: focus_to.clone() };
                Keyed::new(key_of(KEY_RAIL_TAB, id), RailTab { rail: self, id: (*id).to_owned(), roving })
            })
            .collect();
        let tabs: Vec<NodeId> = tabs.iter().map(|tab| cx.child(tab)).collect();
        let across = matches!(self.side, DockSide::Top | DockSide::Bottom);
        let style = LayoutStyle {
            spacing: theme.spacing.s1,
            padding: if across {
                Insets { top: 0.0, bottom: 0.0, left: theme.spacing.s2, right: theme.spacing.s2 }
            } else {
                Insets { top: theme.spacing.s2, bottom: theme.spacing.s2, left: 0.0, right: 0.0 }
            },
            align: Align::Stretch,
            ..LayoutStyle {
                axis: Some(if across { Axis::Horizontal } else { Axis::Vertical }),
                ..placed(self.rect)
            }
        };
        let node = cx.container(style, &tabs, Paint::Quad(QuadPaint::new(theme.color(Semantic::Canvas))));
        cx.semantics(node, Semantics::new(Role::Toolbar).name(labels.dock_place(self.side).to_owned()));
        node
    }
}

/// The tab of a hidden panel on its rail: a click opens the panel over the rest, and so does the
/// pointer resting on it.
struct RailTab<'a> {
    rail: &'a Rail<'a>,
    id: String,
    roving: Roving,
}

impl View for RailTab<'_> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let labels = cx.labels();
        let (rail, id) = (self.rail, self.id.clone());
        let actions = rail.actions;
        let title = rail.area.title_of(&id).to_owned();
        let on = actions.opened.get().is_some_and(|open| open.panel == id);
        let hovered = cx.is_hovered();
        // Resting on it opens it; leaving forgets when it started.
        let since: Rc<Cell<Option<f64>>> = cx.use_state(|| Rc::new(Cell::new(None))).peek().clone();
        if hovered {
            *actions.hover_tab.borrow_mut() = Some(id.clone());
            let now = cx.now();
            let start = since.get().unwrap_or_else(|| {
                since.set(Some(now));
                now
            });
            if !on && actions.opened.peek().is_none() {
                if now >= start + HOVER_OPEN {
                    actions.opened.set(Some(Opened { panel: id.clone(), by_hover: true }));
                } else {
                    cx.wake_at(start + HOVER_OPEN);
                }
            }
        } else {
            since.set(None);
            let mut tab = actions.hover_tab.borrow_mut();
            if tab.as_deref() == Some(id.as_str()) {
                *tab = None;
            }
        }
        let across = matches!(rail.side, DockSide::Top | DockSide::Bottom);
        let words = Text::new(title.clone()).style(TextStyle::Callout);
        let words =
            if across { cx.child(&words) } else { cx.child(&QuarterTurn::new(words, Turn::Clockwise)) };
        cx.semantics(words, Semantics::hidden());
        let unsaved = rail.area.panel(&id).is_some_and(|panel| panel.unsaved);
        let mut inside = vec![words];
        if unsaved {
            inside.push(unsaved_dot(cx));
        }
        let accent = theme.color(Semantic::Accent);
        let fill = if on || hovered { theme.color(Semantic::Surface) } else { Color::TRANSPARENT };
        let side = rail.side;
        let (frames, recorded) = (rail.frames.clone(), id.clone());
        let radius = theme.radii.sm;
        let node = cx.container(
            LayoutStyle {
                padding: if across {
                    Insets::symmetric(0.0, theme.spacing.s3)
                } else {
                    Insets::symmetric(theme.spacing.s3, 0.0)
                },
                align: Align::Center,
                justify: Justify::Center,
                spacing: theme.spacing.s1,
                ..LayoutStyle::stack(if across { Axis::Horizontal } else { Axis::Vertical })
            },
            &inside,
            Paint::Custom(Rc::new(move |frame, scene| {
                frames.tabs.borrow_mut().insert(recorded.clone(), frame);
                if fill.a > 0.0 {
                    scene.push_quad(Quad::new(frame, fill).radius(radius));
                }
                if on {
                    // A line on the side that faces the panel it opens.
                    let line = match side {
                        DockSide::Right => Rect::new(frame.x(), frame.y(), 2.0, frame.height()),
                        DockSide::Top => Rect::new(frame.x(), frame.max_y() - 2.0, frame.width(), 2.0),
                        DockSide::Bottom => Rect::new(frame.x(), frame.y(), frame.width(), 2.0),
                        _ => Rect::new(frame.max_x() - 2.0, frame.y(), 2.0, frame.height()),
                    };
                    scene.push_quad(Quad::new(line, accent));
                }
            })),
        );
        let (toggler, opened) = (actions.clone(), id.clone());
        cx.on_click(node, Rc::new(move || toggler.toggle_open(&opened)));
        cx.cursor(node, CursorIcon::Pointer);
        cx.focusable(node, radius);
        let semantics = Semantics::new(Role::Button).name(labels.dock_hidden_name(&title, side)).expanded(on);
        cx.semantics(node, if unsaved { semantics.description(labels.unsaved.clone()) } else { semantics });
        if actions.focus.borrow().as_deref() == Some(id.as_str()) {
            actions.focus.borrow_mut().take();
            cx.request_focus_node(node);
        }
        if rail.area.movable {
            let deep = rail.layout.auto_hidden_size(&id).unwrap_or(FLOAT_MIN.width);
            let was = float_box(rail.arranged.size, Size::new(deep, FLOAT_MIN.height * 2.0), 0).size;
            let context = DragContext {
                actions: actions.clone(),
                state: rail.drag.clone(),
                frames: rail.frames.clone(),
                arranged: rail.arranged.clone(),
                tab_bar: rail.area.tab_bar,
                modifiers: cx.modifiers(),
                floats: rail.area.floatable.then(|| (title.clone(), was)),
            };
            cx.on_drag(node, context.handler(id.clone()));
        }
        let (pinner, pinned) = (actions.clone(), id);
        let before: crate::KeyHandler = Rc::new(move |key, modifiers| {
            if pin_key(key, modifiers) {
                pinner.pin(&pinned, &title);
                return true;
            }
            false
        });
        self.roving.wire_with(cx, node, None, Some(before));
        node
    }
}

/// A hidden panel open over the rest, beside its rail: its name, the pin that puts it back, what
/// it shows, and the edge that resizes it.
struct OpenPanel<'a> {
    area: &'a DockArea,
    id: String,
    side: DockSide,
    /// How deep it opens, away from its rail.
    size: f32,
    /// How long it is, along its rail.
    length: f32,
    most: f32,
    by_hover: bool,
    actions: Actions,
}

impl View for OpenPanel<'_> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let labels = cx.labels();
        let (id, side, actions) = (self.id.clone(), self.side, &self.actions);
        let title = self.area.title_of(&id).to_owned();
        // Opened by the pointer resting on its tab, it goes back a moment after the pointer leaves
        // both.
        if self.by_hover {
            let left_at: Rc<Cell<Option<f64>>> = cx.use_state(|| Rc::new(Cell::new(None))).peek().clone();
            let inside = cx.is_hover_within() || actions.hover_tab.borrow().as_deref() == Some(id.as_str());
            if inside {
                left_at.set(None);
            } else {
                let now = cx.now();
                let start = left_at.get().unwrap_or_else(|| {
                    left_at.set(Some(now));
                    now
                });
                if now >= start + HOVER_CLOSE {
                    actions.opened.set(None);
                } else {
                    cx.wake_at(start + HOVER_CLOSE);
                }
            }
        }
        // Focus went in and then out, by Tab or Shift+Tab: it goes back to its rail.
        let had_focus: Rc<Cell<bool>> = cx.use_state(|| Rc::new(Cell::new(false))).peek().clone();
        if cx.is_focus_within() {
            had_focus.set(true);
        } else if had_focus.get() {
            had_focus.set(false);
            actions.opened.set(None);
        }
        let attrs = util::attrs(&theme, TextStyle::Callout, Some(ravenrs_style::Weight::SEMIBOLD));
        let name = cx.text(&title, &attrs, theme.color(Semantic::Ink), LayoutStyle::leaf());
        let room = cx.container(LayoutStyle { grow: 1.0, ..LayoutStyle::leaf() }, &[], Paint::None);
        let (pinner, pinned, named) = (actions.clone(), id.clone(), title.clone());
        let pin = IconButton::new(IconName::PinOff)
            .label(labels.dock_pin_name(&title))
            .style(ButtonStyle::Ghost)
            .size(ButtonSize::Mini)
            .on_click(move || {
                pinner.pin(&pinned, &named);
            });
        let pin = cx.child(&pin);
        if actions.focus_open.replace(false) && !self.by_hover {
            cx.request_focus_node(pin);
        }
        let unsaved = self.area.panel(&id).is_some_and(|panel| panel.unsaved);
        let mut header_parts = vec![name];
        if unsaved {
            header_parts.push(unsaved_dot(cx));
        }
        header_parts.extend([room, pin]);
        let header = cx.container(
            LayoutStyle {
                height: Some(TAB_HEIGHT),
                align: Align::Center,
                spacing: theme.spacing.s1,
                padding: Insets { top: 0.0, bottom: 0.0, left: theme.spacing.s3, right: theme.spacing.s1 },
                align_self: Some(Align::Stretch),
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &header_parts,
            Paint::Quad(QuadPaint::new(theme.color(Semantic::Surface))),
        );
        let content = (self.area.content)(&id);
        let content = cx.keyed_child(key_of(KEY_PANEL, &id), content.as_ref());
        let body = cx.container(
            LayoutStyle {
                grow: 1.0,
                height: Some(0.0),
                min_height: Some(0.0),
                min_width: Some(0.0),
                align: Align::Stretch,
                overflow_hidden: true,
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &[content],
            Paint::None,
        );
        cx.clip(body);
        let column = cx.container(
            LayoutStyle {
                grow: 1.0,
                min_width: Some(0.0),
                min_height: Some(0.0),
                align: Align::Stretch,
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &[header, body],
            Paint::None,
        );
        let across = matches!(side, DockSide::Top | DockSide::Bottom);
        let (writer, sized) = (actions.clone(), id.clone());
        let value = self.size;
        let binding = Binding::new(
            move || value,
            move |next: f32| writer.resize(|layout| layout.set_auto_hidden_size(&sized, next)),
        );
        let mut handle = SplitHandle::new(if across { Axis::Vertical } else { Axis::Horizontal }, binding)
            .range(OPEN_MIN, self.most)
            .label(labels.dock_resize_name(side));
        if matches!(side, DockSide::Right | DockSide::Bottom) {
            handle = handle.inverted();
        }
        let settler = actions.clone();
        let handle = cx.child(&handle.on_end(move || settler.settle_resize(side)));
        let parts = match side {
            DockSide::Right | DockSide::Bottom => [handle, column],
            _ => [column, handle],
        };
        let deep = self.size + crate::extras::SPLIT_BAR;
        let (width, height) = if across { (self.length, deep) } else { (deep, self.length) };
        let node = cx.container(
            LayoutStyle {
                width: Some(width),
                height: Some(height),
                align: Align::Stretch,
                ..LayoutStyle::stack(if across { Axis::Vertical } else { Axis::Horizontal })
            },
            &parts,
            Paint::Quad(QuadPaint {
                border: Some(Border { width: 1.0, color: theme.color(Semantic::Border) }),
                shadow: Some(theme.elevation.raised),
                ..QuadPaint::new(theme.color(Semantic::Surface))
            }),
        );
        cx.clip(node);
        cx.semantics(node, Semantics::new(Role::Group).name(title.clone()));
        let (pinner, pinned) = (actions.clone(), id);
        cx.on_key(
            node,
            Rc::new(move |key, modifiers| {
                if pin_key(key, modifiers) {
                    pinner.pin(&pinned, &title);
                    return true;
                }
                false
            }),
        );
        node
    }
}

/// Where a tab would float if let go now: an outline of the box it would take.
fn float_outline(cx: &mut BuildCx<'_>, rect: Rect) -> NodeId {
    let theme = *cx.theme();
    let ring = theme.color(Semantic::FocusRing);
    let paint = QuadPaint {
        corner_radius: theme.radii.md,
        border: Some(Border { width: 2.0, color: ring }),
        ..QuadPaint::new(ring.with_alpha(0.08))
    };
    let node = cx.container(placed(rect), &[], Paint::Quad(paint));
    cx.semantics(node, Semantics::hidden());
    node
}

/// A panel floating over the area, in a box of its own: a header that moves it, with its name and
/// its buttons, what it shows, and edges that resize it.
struct FloatingView<'a> {
    area: &'a DockArea,
    id: String,
    /// Where it shows, relative to the area, already kept inside it.
    rect: Rect,
    /// Where the other floating panels show, for its edges to draw to.
    others: Vec<Rect>,
    actions: &'a Actions,
    arranged: &'a Rc<Arranged>,
    layout: &'a DockLayout,
    frames: &'a Rc<Frames>,
    float_drag: &'a State<Option<FloatDrag>>,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl FloatingView<'_> {
    /// What a drag of the header (`Grab::MOVE`) or of an edge does: it follows the pointer, and
    /// is asked about once, when let go.
    fn grab_handler(&self, grab: Grab, modifiers: Rc<Cell<Modifiers>>) -> crate::context::DragHandler {
        let (state, actions, id) = (self.float_drag.clone(), self.actions.clone(), self.id.clone());
        let (from, others, size) = (self.rect, self.others.clone(), self.arranged.size);
        let (frames, arranged, tab_bar) = (self.frames.clone(), self.arranged.clone(), self.area.tab_bar);
        Rc::new(move |drag| match drag.phase {
            DragPhase::Start => {
                actions.raise(&id);
                actions.forget_notice();
                let pressed_at = drag.position;
                state.set(Some(FloatDrag {
                    panel: id.clone(),
                    grab,
                    pressed_at,
                    from,
                    rect: from,
                    aim: None,
                    lines: Vec::new(),
                }));
            }
            DragPhase::Move => {
                // Nothing here when it was called off with Escape.
                let Some(mut now) = state.get() else { return };
                let (dx, dy) = (drag.position.x - now.pressed_at.x, drag.position.y - now.pressed_at.y);
                if grab.moves() {
                    let moved = Rect::new(from.x() + dx, from.y() + dy, from.width(), from.height());
                    let (snapped, lines) = geometry::snap_floating(moved, size, &others, MAGNET);
                    now.rect = geometry::clamp_floating(snapped, size, FLOAT_MIN);
                    now.lines = lines;
                    let root = frames.root.get();
                    let at = Point::new(drag.position.x - root.x(), drag.position.y - root.y());
                    let tabs = frames.tabs.borrow();
                    let strip = move |group: &GroupPlace| strip_height(tab_bar, group.panels.len());
                    let tab = |panel: &str| {
                        tabs.get(panel)
                            .map(|f| Rect::new(f.x() - root.x(), f.y() - root.y(), f.width(), f.height()))
                    };
                    let layout = actions.layout.get();
                    now.aim = geometry::float_aim(&arranged, at, &strip, &tab, modifiers.get().shift)
                        .filter(|aim| actions.may_go(&layout, &id, &aim.target));
                } else {
                    let (mut x0, mut y0, mut x1, mut y1) = (from.x(), from.y(), from.max_x(), from.max_y());
                    if grab.left {
                        x0 = (x0 + dx).min(x1 - FLOAT_MIN.width).max(0.0);
                    }
                    if grab.right {
                        x1 = (x1 + dx).max(x0 + FLOAT_MIN.width).min(size.width);
                    }
                    if grab.top {
                        y0 = (y0 + dy).min(y1 - FLOAT_MIN.height).max(0.0);
                    }
                    if grab.bottom {
                        y1 = (y1 + dy).max(y0 + FLOAT_MIN.height).min(size.height);
                    }
                    now.rect = Rect::new(x0, y0, (x1 - x0).max(0.0), (y1 - y0).max(0.0));
                }
                state.set_if_changed(Some(now));
            }
            DragPhase::End => {
                if let Some(done) = state.get() {
                    state.set(None);
                    match done.aim {
                        Some(aim) => {
                            actions.dock(&id, aim.target);
                        }
                        None if done.rect != done.from => {
                            actions.place(&id, done.rect);
                        }
                        None => {}
                    }
                }
            }
            // The base may add more; they act as nothing here.
            _ => {}
        })
    }
}

/// An edge or a corner of a floating panel, which resizes it.
struct Grabber<'a> {
    view: &'a FloatingView<'a>,
    which: Grab,
    /// Where it is, relative to the panel.
    at: Rect,
    cursor: CursorIcon,
    lit: bool,
    modifiers: Rc<Cell<Modifiers>>,
}

impl View for Grabber<'_> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let (which, at, lit) = (self.which, self.at, self.lit);
        let corner = (which.left || which.right) && (which.top || which.bottom);
        let (dot, ring) = (theme.color(Semantic::Ink), theme.color(Semantic::Accent));
        let node = cx.container(
            LayoutStyle {
                absolute: Some((at.x(), at.y())),
                width: Some(at.width().max(0.0)),
                height: Some(at.height().max(0.0)),
                ..LayoutStyle::leaf()
            },
            &[],
            Paint::Custom(Rc::new(move |frame, scene| {
                // The corners show a small square while the panel is under the pointer.
                if lit && corner {
                    let square = Rect::new(frame.center().x - 4.0, frame.center().y - 4.0, 8.0, 8.0);
                    scene.push_quad(Quad::new(square, dot).radius(2.0).border(1.0, ring));
                }
            })),
        );
        cx.cursor(node, self.cursor);
        cx.on_drag(node, self.view.grab_handler(which, self.modifiers.clone()));
        cx.semantics(node, Semantics::hidden());
        node
    }
}

impl View for FloatingView<'_> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let labels = cx.labels();
        let (id, actions) = (self.id.clone(), self.actions);
        let title = self.area.title_of(&id).to_owned();
        let dragging = self.float_drag.get().filter(|drag| drag.panel == id);
        let rect = dragging.as_ref().map_or(self.rect, |drag| drag.rect);
        let lit = cx.is_hover_within() || dragging.is_some();
        let muted = theme.color(Semantic::InkMuted);

        // The header: a grip, the name, and the buttons.
        let grip = cx.container(
            LayoutStyle { width: Some(10.0), height: Some(14.0), ..LayoutStyle::leaf() },
            &[],
            Paint::Custom(Rc::new(move |frame, scene| {
                for row in 0..3 {
                    for column in 0..2 {
                        let at = Point::new(
                            frame.x() + 2.0 + column as f32 * 4.0,
                            frame.y() + 2.0 + row as f32 * 4.5,
                        );
                        scene.push_quad(Quad::new(Rect::new(at.x, at.y, 2.0, 2.0), muted).radius(1.0));
                    }
                }
            })),
        );
        cx.semantics(grip, Semantics::hidden());
        let attrs = util::attrs(&theme, TextStyle::Callout, Some(ravenrs_style::Weight::SEMIBOLD));
        let name = cx.text_truncated(
            &title,
            &attrs,
            theme.color(Semantic::Ink),
            LayoutStyle { grow: 1.0, min_width: Some(0.0), ..LayoutStyle::leaf() },
        );
        cx.semantics(name, Semantics::hidden());
        let mut parts = vec![grip, name];
        let unsaved = self.area.panel(&id).is_some_and(|panel| panel.unsaved);
        if unsaved {
            parts.insert(2, unsaved_dot(cx));
        }
        let (back, home) = (actions.clone(), id.clone());
        let button = IconButton::new(IconName::Minimize2)
            .label(labels.dock_dock_back_name(&title))
            .style(ButtonStyle::Ghost)
            .size(ButtonSize::Mini)
            .on_click(move || {
                back.dock_home(&home);
            });
        parts.push(cx.child(&button));
        let items = panel_items(self.area, self.layout, self.arranged, actions, &id);
        if !items.is_empty() {
            let entries: Vec<Entry> = items.into_iter().map(Entry::Item).collect();
            parts.push(cx.child(&Options { entries, name: labels.dock_options_name(&title) }));
        }
        let closable = self.area.panel(&id).is_some_and(|panel| panel.closable);
        if closable {
            let (closer, gone, named) = (actions.clone(), id.clone(), title.clone());
            let button = IconButton::new(IconName::X)
                .label(labels.dock_close_name(&title))
                .style(ButtonStyle::Ghost)
                .size(ButtonSize::Mini)
                .on_click(move || {
                    closer.close(&gone, None, &named);
                });
            parts.push(cx.child(&button));
        }
        let radius = theme.radii.md;
        let header = cx.container(
            LayoutStyle {
                height: Some(TAB_HEIGHT),
                align: Align::Center,
                spacing: theme.spacing.s1,
                padding: Insets { top: 0.0, bottom: 0.0, left: theme.spacing.s2, right: theme.spacing.s1 },
                align_self: Some(Align::Stretch),
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &parts,
            Paint::Quad(QuadPaint {
                corner_radius: radius,
                ..QuadPaint::new(theme.color(Semantic::SurfaceElevated))
            }),
        );
        cx.focusable(header, radius);
        cx.cursor(header, CursorIcon::Move);
        cx.semantics(header, Semantics::new(Role::Button).name(labels.dock_move_floating_name(&title)));
        cx.test_id(header, &format!("floating-move:{id}"));
        let modifiers = cx.modifiers();
        cx.on_drag(header, self.grab_handler(Grab::MOVE, modifiers.clone()));
        let (back, home) = (actions.clone(), id.clone());
        cx.on_double_click(
            header,
            Rc::new(move || {
                back.dock_home(&home);
            }),
        );
        if actions.focus.borrow().as_deref() == Some(id.as_str()) {
            actions.focus.borrow_mut().take();
            cx.request_focus_node(header);
        }
        // From the keyboard: the arrows move it, Alt with them resizes it, Ctrl+Alt+F puts it back.
        let (keys, keyed, called_off) = (actions.clone(), id.clone(), self.float_drag.clone());
        let (named, size) = (title.clone(), self.arranged.size);
        cx.on_key(
            header,
            Rc::new(move |key, modifiers| {
                if *key == Key::Escape && called_off.peek().is_some() {
                    called_off.set(None);
                    return true;
                }
                if float_key(key, modifiers) {
                    keys.dock_home(&keyed);
                    return true;
                }
                if *key == Key::Delete && closable {
                    keys.close(&keyed, None, &named);
                    return true;
                }
                let step = if modifiers.shift { 32.0 } else { 8.0 };
                let (dx, dy) = match key {
                    Key::ArrowLeft => (-step, 0.0),
                    Key::ArrowRight => (step, 0.0),
                    Key::ArrowUp => (0.0, -step),
                    Key::ArrowDown => (0.0, step),
                    _ => return false,
                };
                let next = if modifiers.alt {
                    Rect::new(rect.x(), rect.y(), rect.width() + dx, rect.height() + dy)
                } else {
                    Rect::new(rect.x() + dx, rect.y() + dy, rect.width(), rect.height())
                };
                keys.place(&keyed, geometry::clamp_floating(next, size, FLOAT_MIN));
                true
            }),
        );

        // What it shows.
        let content = (self.area.content)(&id);
        let content = cx.keyed_child(key_of(KEY_PANEL, &id), content.as_ref());
        let body = cx.container(
            LayoutStyle {
                grow: 1.0,
                height: Some(0.0),
                min_height: Some(0.0),
                min_width: Some(0.0),
                align: Align::Stretch,
                overflow_hidden: true,
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &[content],
            Paint::None,
        );
        cx.clip(body);
        cx.semantics(body, Semantics::new(Role::TabPanel).name(title.clone()));
        let column = cx.container(
            LayoutStyle {
                width: Some(rect.width()),
                height: Some(rect.height()),
                align: Align::Stretch,
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &[header, body],
            Paint::None,
        );

        // The edges and the corners that resize it, a little outside the box.
        let (w, h, g) = (rect.width(), rect.height(), GRAB);
        let grab = |left, right, top, bottom| Grab { left, right, top, bottom };
        let handles = [
            (grab(true, false, false, false), Rect::new(-g / 2.0, g, g, h - 2.0 * g), CursorIcon::ColResize),
            (
                grab(false, true, false, false),
                Rect::new(w - g / 2.0, g, g, h - 2.0 * g),
                CursorIcon::ColResize,
            ),
            (grab(false, false, true, false), Rect::new(g, -g / 2.0, w - 2.0 * g, g), CursorIcon::RowResize),
            (
                grab(false, false, false, true),
                Rect::new(g, h - g / 2.0, w - 2.0 * g, g),
                CursorIcon::RowResize,
            ),
            (grab(true, false, true, false), Rect::new(-g, -g, 2.0 * g, 2.0 * g), CursorIcon::NwseResize),
            (
                grab(false, true, false, true),
                Rect::new(w - g, h - g, 2.0 * g, 2.0 * g),
                CursorIcon::NwseResize,
            ),
            (grab(false, true, true, false), Rect::new(w - g, -g, 2.0 * g, 2.0 * g), CursorIcon::NeswResize),
            (grab(true, false, false, true), Rect::new(-g, h - g, 2.0 * g, 2.0 * g), CursorIcon::NeswResize),
        ];
        let mut children = vec![column];
        for (which, at, cursor) in handles {
            // Each its own view: a view drags by one element only.
            let handle = Grabber { view: self, which, at, cursor, lit, modifiers: modifiers.clone() };
            children.push(cx.child(&handle));
        }
        let node = cx.container(
            placed(rect),
            &children,
            Paint::Quad(QuadPaint {
                corner_radius: radius,
                border: Some(Border { width: 1.0, color: theme.color(Semantic::Border) }),
                shadow: Some(theme.elevation.raised),
                ..QuadPaint::new(theme.color(Semantic::Surface))
            }),
        );
        let semantics = Semantics::new(Role::Dialog).name(labels.dock_floating_name(&title));
        cx.semantics(node, if unsaved { semantics.description(labels.unsaved.clone()) } else { semantics });
        cx.test_id(node, &format!("floating:{id}"));
        node
    }
}

/// The list Ctrl+Tab shows while Ctrl is held: the panels by how lately they were used, the
/// documents apart from the tools, each with where it is.
struct Switcher<'a> {
    area: &'a DockArea,
    layout: &'a DockLayout,
    switch: Switch,
    state: State<Option<Switch>>,
    actions: &'a Actions,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl Switcher<'_> {
    /// Where a panel is, in a few words.
    fn whereabouts(&self, labels: &Labels, id: &str) -> String {
        if self.layout.is_floating(id) {
            return labels.dock_where_floating.clone();
        }
        if let Some(side) = self.layout.auto_hidden_side(id) {
            return labels.dock_where_hidden.replace("{place}", &labels.dock_place(side).to_lowercase());
        }
        self.layout.side_of(id).map(|side| labels.dock_place(side).to_lowercase()).unwrap_or_default()
    }
}

impl View for Switcher<'_> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let labels = cx.labels();
        let is_document =
            |id: &str| self.area.panel(id).is_some_and(|panel| panel.kind == DockPanelKind::Document);
        let documents: Vec<(usize, &String)> =
            self.switch.order.iter().enumerate().filter(|(_, id)| is_document(id)).collect();
        let tools: Vec<(usize, &String)> =
            self.switch.order.iter().enumerate().filter(|(_, id)| !is_document(id)).collect();
        let mut columns = Vec::new();
        let named: Vec<(String, Vec<(usize, &String)>)> = if documents.is_empty() {
            vec![(labels.dock_tabs_name.clone(), tools)]
        } else {
            vec![(labels.dock_documents.clone(), documents), (labels.dock_tools.clone(), tools)]
        };
        for (heading, rows) in named {
            let caption = util::attrs(&theme, TextStyle::Caption, None);
            let mut parts =
                vec![cx.text(&heading, &caption, theme.color(Semantic::InkMuted), LayoutStyle::leaf())];
            for (index, id) in rows {
                let on = index == self.switch.index;
                let (ink, muted) = if on {
                    (theme.color(Semantic::OnAccent), theme.color(Semantic::OnAccent))
                } else {
                    (theme.color(Semantic::Ink), theme.color(Semantic::InkMuted))
                };
                let title = self.area.title_of(id).to_owned();
                let whereabouts = self.whereabouts(&labels, id);
                let body = util::attrs(&theme, TextStyle::Callout, None);
                let name = cx.text_truncated(
                    &title,
                    &body,
                    ink,
                    LayoutStyle { grow: 1.0, min_width: Some(0.0), ..LayoutStyle::leaf() },
                );
                let place = cx.text(
                    &whereabouts,
                    &util::attrs(&theme, TextStyle::Caption, None),
                    muted,
                    LayoutStyle::leaf(),
                );
                let fill = if on { theme.color(Semantic::Accent) } else { Color::TRANSPARENT };
                let row = cx.container(
                    LayoutStyle {
                        height: Some(36.0),
                        padding: Insets::symmetric(0.0, theme.spacing.s3),
                        spacing: theme.spacing.s2,
                        align: Align::Center,
                        align_self: Some(Align::Stretch),
                        ..LayoutStyle::stack(Axis::Horizontal)
                    },
                    &[name, place],
                    Paint::Quad(QuadPaint { corner_radius: theme.radii.md, ..QuadPaint::new(fill) }),
                );
                let (actions, state, chosen) = (self.actions.clone(), self.state.clone(), id.clone());
                cx.on_click(
                    row,
                    Rc::new(move || {
                        state.set(None);
                        actions.go_to(&chosen);
                    }),
                );
                cx.cursor(row, CursorIcon::Pointer);
                cx.semantics(
                    row,
                    Semantics::new(Role::ListBoxOption).name(format!("{title}, {whereabouts}")).selected(on),
                );
                parts.push(row);
            }
            let column = cx.container(
                LayoutStyle {
                    grow: 1.0,
                    min_width: Some(0.0),
                    spacing: 2.0,
                    align: Align::Stretch,
                    ..LayoutStyle::stack(Axis::Vertical)
                },
                &parts,
                Paint::None,
            );
            cx.semantics(column, Semantics::new(Role::ListBox).name(heading.clone()));
            columns.push(column);
        }
        let node = cx.container(
            LayoutStyle {
                width: Some(SWITCHER_WIDTH),
                padding: Insets::uniform(theme.spacing.s4),
                spacing: theme.spacing.s3,
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &columns,
            Paint::Quad(QuadPaint {
                corner_radius: theme.radii.lg,
                border: Some(Border { width: 1.0, color: theme.color(Semantic::Border) }),
                shadow: Some(theme.elevation.raised),
                ..QuadPaint::new(theme.color(Semantic::SurfaceElevated))
            }),
        );
        cx.semantics(node, Semantics::new(Role::Dialog).name(labels.dock_switcher.clone()));
        node
    }
}

/// The mark of where a drop lands: a line between tabs, or the room a panel would take.
fn mark(cx: &mut BuildCx<'_>, rect: Rect) -> NodeId {
    let theme = *cx.theme();
    let accent = theme.color(Semantic::Accent);
    let line = rect.width() <= 2.5;
    let paint = if line {
        QuadPaint { corner_radius: 1.0, ..QuadPaint::new(accent) }
    } else {
        QuadPaint {
            corner_radius: theme.radii.sm,
            border: Some(Border { width: 2.0, color: accent }),
            ..QuadPaint::new(accent.with_alpha(0.18))
        }
    };
    let node = cx.container(placed(rect), &[], Paint::Quad(paint));
    cx.semantics(node, Semantics::hidden());
    node
}
