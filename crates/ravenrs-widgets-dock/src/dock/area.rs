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
use super::layout::{DockDrop, DockEdge, DockLayout, DockSide};
use crate::commands::menu_trigger;
use crate::context::{BuildCx, DragPhase, NodeId, Paint, QuadPaint, Semantics};
use crate::overlay::Entry;
use crate::reorder;
use crate::stack::ScrollEdgeArrow;
use crate::util::{self, FocusTarget, Roving};
use crate::view::{AnyView, Keyed, View};
use crate::{
    Button, ButtonSize, ButtonStyle, ContextMenu, IconButton, IconName, Labels, MenuItem, SplitHandle,
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

/// What runs when the arrangement settles.
type OnEnd = Rc<dyn Fn(&DockLayout)>;

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

/// Panels in a tree of splits and groups of tabs, at the left, the right, the bottom and the center
/// of a space. Bars between them resize with the pointer or the keyboard, docks collapse from the
/// strips on their edges, and panels close and come back.
///
/// A tab goes anywhere with a drag, with the target marked: to another group, to one side of a
/// group, or to a border that has no dock showing. Without a pointer, the arrows walk between tabs,
/// Alt with an arrow moves one within its group, Alt and Shift with an arrow send it to a dock, the
/// menu of a tab or of the button at the end of the strip has "Move to…", and Delete closes.
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
    tab_bar: DockTabBar,
    on_end: Option<OnEnd>,
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
                center_min: (200.0, 120.0),
                pane_min: 100.0,
            },
            movable: true,
            tab_bar: DockTabBar::Always,
            on_end: None,
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

/// What the parts of the area change the arrangement through.
#[derive(Clone)]
struct Actions {
    layout: Binding<DockLayout>,
    on_end: Option<OnEnd>,
    spoken: State<String>,
    /// The panel whose tab takes focus when it is next built.
    focus: Rc<RefCell<Option<String>>>,
    labels: Rc<Labels>,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl Actions {
    fn apply(&self, change: impl FnOnce(&mut DockLayout) -> bool) -> bool {
        let mut next = self.layout.get();
        if !change(&mut next) {
            return false;
        }
        self.layout.set(next.clone());
        if let Some(on_end) = &self.on_end {
            on_end(&next);
        }
        true
    }

    fn settled(&self) {
        if let Some(on_end) = &self.on_end {
            on_end(&self.layout.get());
        }
    }

    /// Sends a panel somewhere, keeping focus on its tab and saying where it went.
    fn send(&self, id: &str, target: DockDrop) -> bool {
        let moved = self.apply(|layout| layout.move_panel(id, target));
        if moved {
            *self.focus.borrow_mut() = Some(id.to_owned());
            if let Some(side) = self.layout.get().side_of(id) {
                self.spoken.set(self.labels.dock_moved_to(side));
            }
        }
        moved
    }

    fn close(&self, id: &str, next: Option<String>, title: &str) {
        if self.apply(|layout| layout.close(id)) {
            *self.focus.borrow_mut() = next;
            self.spoken.set(self.labels.dock_closed_name(title));
        }
    }

    fn reopen(&self, id: &str) {
        if self.apply(|layout| layout.reopen(id)) {
            *self.focus.borrow_mut() = Some(id.to_owned());
        }
    }
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
                actions.apply(|layout| !layout.is_active(&to_show) && layout.activate(&to_show));
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
        let before: crate::KeyHandler = Rc::new(move |key, modifiers| {
            if *key == Key::Escape && called_off.peek().is_some() {
                called_off.set(None);
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
                actions.apply(|layout| layout.activate(id));
            }
        });
        self.roving.wire_with(cx, node, Some(opens), Some(before));
        node
    }
}

/// Which dock Alt and Shift with an arrow send a panel to.
fn dock_key(key: &Key, modifiers: Modifiers) -> Option<DockSide> {
    if !(modifiers.alt && modifiers.shift) {
        return None;
    }
    match key {
        Key::ArrowLeft => Some(DockSide::Left),
        Key::ArrowRight => Some(DockSide::Right),
        Key::ArrowDown => Some(DockSide::Bottom),
        Key::ArrowUp => Some(DockSide::Center),
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
                actions.apply(|layout| {
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
        let actions = Actions {
            layout: self.layout.clone(),
            on_end: self.on_end.clone(),
            spoken: spoken.clone(),
            focus,
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
        if layout.node(DockSide::Center).is_none() {
            let empty = EmptyCenter { area: self, layout: &layout, actions: &actions, rect: arranged.center };
            nodes.push(cx.child(&empty));
        }
        if let Some(aim) = drag.get().and_then(|d| d.aim) {
            nodes.push(mark(cx, aim.mark));
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
        let (value, write) = (bar.value, self.layout.clone());
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
        let kind = bar.kind.clone();
        let binding = Binding::new(
            move || value,
            move |next: f32| {
                let mut layout = write.get();
                match &kind {
                    BarKind::Dock(side) => layout.set_size(*side, next),
                    BarKind::Split { side, path, index, both, .. } => {
                        layout.resize_split(*side, path, *index, next, *both);
                    }
                }
                write.set(layout);
            },
        );
        let mut handle = SplitHandle::new(bar.axis, binding).range(bar.min, bar.max).label(name);
        if inverted {
            handle = handle.inverted();
        }
        let actions = actions.clone();
        let handle = handle.on_end(move || actions.settled());
        let handle = cx.keyed_child(key_of(KEY_BAR, &key), &handle);
        // The bar stretches across its stack, so the stack goes the way the bar's own axis does.
        let style = LayoutStyle { axis: Some(bar.axis), ..placed(bar.rect) };
        cx.container(style, &[handle], Paint::None)
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
