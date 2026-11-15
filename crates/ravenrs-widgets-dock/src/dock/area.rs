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
/// How long the pointer rests on a tab of a rail before its panel opens, in seconds.
const HOVER_OPEN: f64 = 0.4;
/// How long after the pointer leaves a panel it opened that the panel goes back, in seconds.
const HOVER_CLOSE: f64 = 0.6;
/// The least a hidden panel opens to, and what it leaves of the area at the most.
const OPEN_MIN: f32 = 160.0;
const OPEN_MARGIN: f32 = 48.0;

/// How long the reason of a refusal shows, in seconds, unless another gesture comes first.
const NOTICE_SECONDS: f64 = 4.0;

/// What runs when the arrangement settles.
type OnEnd = Rc<dyn Fn(&DockLayout)>;

/// Decides whether a person may make a change, given the arrangement before and after it. When
/// they may not, the arrangement stays as it was, nothing is saved, and the reason shows.
pub type DockGuard = Rc<dyn Fn(&DockLayout, &DockLayout, &DockChange) -> Result<(), String>>;

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
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl DockPanel {
    /// A panel with this id, whose tab says `title`.
    pub fn new(id: impl Into<String>, title: impl Into<String>) -> Self {
        Self { id: id.into(), title: title.into(), icon: None, closable: true }
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
        true
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
        let panel = match change {
            DockChange::Activate { panel }
            | DockChange::Move { panel, .. }
            | DockChange::Close { panel }
            | DockChange::Reopen { panel }
            | DockChange::AutoHide { panel }
            | DockChange::Pin { panel } => Some(panel),
            _ => None,
        };
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

    /// Sends a panel somewhere, keeping focus on its tab and saying where it went.
    fn send(&self, id: &str, target: DockDrop) -> bool {
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

    fn close(&self, id: &str, next: Option<String>, title: &str) {
        if self.apply(DockChange::Close { panel: id.to_owned() }, |layout| layout.close(id)) {
            *self.focus.borrow_mut() = next;
            self.spoken.set(self.labels.dock_closed_name(title));
        }
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
        for side in DockSide::ALL_IN_MENU {
            if layout.side_of(id) != Some(side) {
                places.push((labels.dock_place(side).to_owned(), DockDrop::Side(side)));
            }
        }
        for group in &arranged.groups {
            if own_group.is_some_and(|own| std::ptr::eq(own, group)) {
                continue;
            }
            let Some(first) = group.panels.first() else { continue };
            let shown = group.panels.get(group.active).unwrap_or(first);
            let target = DockDrop::Tab { beside: first.clone(), at: group.panels.len() };
            places.push((labels.dock_with_name(area.title_of(shown)), target));
        }
        if own_group.is_some_and(|own| own.panels.len() > 1) {
            let split = |edge| DockDrop::Split { beside: id.to_owned(), edge };
            places.push((labels.dock_split_right.clone(), split(DockEdge::Right)));
            places.push((labels.dock_split_below.clone(), split(DockEdge::Bottom)));
        }
        if !places.is_empty() {
            let mut item = MenuItem::new(labels.dock_move_to.clone());
            for (name, target) in places {
                let (actions, id) = (actions.clone(), id.to_owned());
                item = item.item(MenuItem::new(name).on_select(move || {
                    actions.send(&id, target.clone());
                }));
            }
            items.push(item);
        }
    }
    if can_hide(area, layout, id) {
        let (actions, id, title) = (actions.clone(), id.to_owned(), area.title_of(id).to_owned());
        items.push(MenuItem::new(labels.dock_auto_hide_item.clone()).on_select(move || {
            actions.hide(&id, &title);
        }));
    }
    if area.panel(id).is_some_and(|panel| panel.closable) {
        let (actions, id, title) = (actions.clone(), id.to_owned(), area.title_of(id).to_owned());
        items.push(
            MenuItem::new(labels.dock_close.clone()).on_select(move || actions.close(&id, None, &title)),
        );
    }
    if let Some(reopen) = reopen_item(area, layout, actions) {
        items.push(reopen);
    }
    items
}

/// The submenu of what can come back, if anything can.
fn reopen_item(area: &DockArea, layout: &DockLayout, actions: &Actions) -> Option<MenuItem> {
    let closed = layout.closed_panels();
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

/// What a tab's drag does: it aims as it goes, and moves on release.
struct DragContext {
    actions: Actions,
    state: State<Option<TabDrag>>,
    frames: Rc<Frames>,
    arranged: Rc<Arranged>,
    tab_bar: DockTabBar,
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
        // A drop that would leave things as they are marks nothing.
        self.actions.layout.get().move_panel(id, found.target.clone()).then_some(found)
    }

    fn handler(self, id: String) -> crate::context::DragHandler {
        Rc::new(move |drag| match drag.phase {
            DragPhase::Start => self.state.set(Some(TabDrag {
                panel: id.clone(),
                pressed_at: drag.position,
                moving: false,
                aim: None,
            })),
            DragPhase::Move => {
                // Nothing here when it was called off with Escape.
                let Some(mut now) = self.state.get() else { return };
                let (dx, dy) = (drag.position.x - now.pressed_at.x, drag.position.y - now.pressed_at.y);
                now.moving |= dx.hypot(dy) > reorder::DRAG_THRESHOLD;
                if now.moving {
                    now.aim = self.aim_at(&id, drag.position);
                }
                self.state.set_if_changed(Some(now));
            }
            DragPhase::End => {
                if let Some(done) = self.state.get()
                    && let Some(aim) = done.aim
                {
                    self.actions.send(&id, aim.target);
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
                .on_click(move || actions.close(&id, None, &title));
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
        cx.semantics(node, Semantics::new(Role::Tab).name(panel.title.clone()).selected(on));
        if self.actions.focus.borrow().as_deref() == Some(id.as_str()) {
            self.actions.focus.borrow_mut().take();
            cx.request_focus_node(node);
        }
        let movable = self.area.movable;
        if movable {
            let context = DragContext {
                actions: self.actions.clone(),
                state: self.drag.clone(),
                frames: self.frames.clone(),
                arranged: self.arranged.clone(),
                tab_bar: self.area.tab_bar,
            };
            cx.on_drag(node, context.handler(id.clone()));
        }
        let (index, count) = (self.index, group_panels.len());
        let first = group_panels.first().cloned().unwrap_or_default();
        let (actions, called_off, closable) = (self.actions.clone(), self.drag.clone(), panel.closable);
        let title = panel.title.clone();
        let key_id = id.clone();
        let hides = can_hide(self.area, &self.actions.layout.get(), &id);
        let before: crate::KeyHandler = Rc::new(move |key, modifiers| {
            if *key == Key::Escape && called_off.peek().is_some() {
                called_off.set(None);
                return true;
            }
            if hides && pin_key(key, modifiers) {
                actions.hide(&key_id, &title);
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
        let words = cx.text(&labels.dock_empty, &attrs, theme.color(Semantic::InkMuted), LayoutStyle::leaf());
        let mut buttons = Vec::new();
        for id in self.layout.closed_panels() {
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
        if let Some(aim) = drag.get().and_then(|d| d.aim) {
            nodes.push(mark(cx, aim.mark));
        }
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
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &[words],
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
        cx.semantics(
            node,
            Semantics::new(Role::Button).name(labels.dock_hidden_name(&title, side)).expanded(on),
        );
        if actions.focus.borrow().as_deref() == Some(id.as_str()) {
            actions.focus.borrow_mut().take();
            cx.request_focus_node(node);
        }
        if rail.area.movable {
            let context = DragContext {
                actions: actions.clone(),
                state: rail.drag.clone(),
                frames: rail.frames.clone(),
                arranged: rail.arranged.clone(),
                tab_bar: rail.area.tab_bar,
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
        let header = cx.container(
            LayoutStyle {
                height: Some(TAB_HEIGHT),
                align: Align::Center,
                spacing: theme.spacing.s1,
                padding: Insets { top: 0.0, bottom: 0.0, left: theme.spacing.s3, right: theme.spacing.s1 },
                align_self: Some(Align::Stretch),
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &[name, room, pin],
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
