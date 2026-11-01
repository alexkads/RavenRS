use std::cell::Cell;
use std::collections::{BTreeSet, HashMap};
use std::fmt;
use std::rc::Rc;

use ravenrs_core::{Border, CursorIcon, Insets, Key, Point, Rect, Role};
use ravenrs_layout::{Align, Axis, Justify, LayoutStyle};
use ravenrs_reactive::{Binding, State};
use ravenrs_style::{Semantic, TextStyle, Theme, Weight};

use super::time::{TimeAxis, TimePoint, TimeScale, assign_lanes};
use crate::context::{BuildCx, Drag, DragPhase, NodeId, Paint, QuadPaint, Semantics};
use crate::display::Tone;
use crate::{Button, ButtonSize, ButtonStyle, IconButton, IconName, Labels, Text, View, util};

const LANE_HEIGHT: f32 = 30.0;
const ROW_PADDING: f32 = 4.0;
const GROUP_HEIGHT: f32 = 30.0;
const HEADER_ROW_HEIGHT: f32 = 26.0;
const MIN_BAR_WIDTH: f32 = 6.0;
const HANDLE_WIDTH: f32 = 6.0;
/// Rows and ticks built beyond the edges, so a quick scroll shows no gap.
const OVERSCAN: f32 = 200.0;

/// A row of a [`Scheduler`]: a person, a room, a machine, an account.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Resource {
    /// What events refer to.
    pub id: u64,
    /// What the row is called.
    pub name: String,
    /// A second line under the name.
    pub subtitle: Option<String>,
    /// The collapsible group it goes under; rows with the same group stay together.
    pub group: Option<String>,
}

impl Resource {
    /// A row with this name.
    pub fn new(id: u64, name: impl Into<String>) -> Self {
        Self { id, name: name.into(), subtitle: None, group: None }
    }

    /// Sets the second line.
    pub fn subtitle(mut self, text: impl Into<String>) -> Self {
        self.subtitle = Some(text.into());
        self
    }

    /// Puts it in a group.
    pub fn group(mut self, name: impl Into<String>) -> Self {
        self.group = Some(name.into());
        self
    }
}

/// A bar on a [`Scheduler`]. One with no length shows as a milestone.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SchedulerEvent {
    /// Given back when it changes.
    pub id: u64,
    /// The row it is on.
    pub resource: u64,
    /// When it begins.
    pub start: TimePoint,
    /// When it ends.
    pub end: TimePoint,
    /// What it is.
    pub title: String,
    /// Its color; say what it means in the title or next to the scheduler too.
    pub tone: Tone,
    /// It already happened, so it stands back.
    pub done: bool,
    /// It cannot be moved or resized.
    pub locked: bool,
    /// Its length can change.
    pub resizable: bool,
}

impl SchedulerEvent {
    /// A bar on this row, from `start` to `end`.
    pub fn new(id: u64, resource: u64, start: TimePoint, end: TimePoint, title: impl Into<String>) -> Self {
        Self {
            id,
            resource,
            start,
            end,
            title: title.into(),
            tone: Tone::Accent,
            done: false,
            locked: false,
            resizable: true,
        }
    }

    /// Sets the color.
    pub fn tone(mut self, tone: Tone) -> Self {
        self.tone = tone;
        self
    }

    /// Marks it as already done.
    pub fn done(mut self, done: bool) -> Self {
        self.done = done;
        self
    }

    /// Stops it being moved or resized.
    pub fn locked(mut self, locked: bool) -> Self {
        self.locked = locked;
        self
    }

    /// Whether its length can change (on by default). It can still be moved.
    pub fn resizable(mut self, resizable: bool) -> Self {
        self.resizable = resizable;
        self
    }
}

/// A shaded stretch of time across all rows, such as a holiday or a deadline.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SchedulerRange {
    /// When it begins.
    pub start: TimePoint,
    /// When it ends.
    pub end: TimePoint,
    /// Its color.
    pub tone: Tone,
}

/// An event moved, resized, or drawn for the first time: where it would now be.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SchedulerChange {
    /// The event; for one being created, `None`.
    pub id: Option<u64>,
    /// Its row.
    pub resource: u64,
    /// When it begins.
    pub start: TimePoint,
    /// When it ends.
    pub end: TimePoint,
}

type CanDrop = Rc<dyn Fn(&SchedulerChange) -> bool>;
type OnChange = Rc<dyn Fn(SchedulerChange)>;

/// A timeline of resources: a row for each person, room or account, a time axis
/// across the top, and events as bars that can be dragged along the time and
/// between rows, resized by their ends, and drawn on an empty stretch. Weekends
/// are shaded, and only what is in view gets built, so large plans stay smooth.
///
/// The app holds the data: `on_change` reports where an event would go, and the
/// app stores it and passes the events again. `can_drop` can refuse a place.
///
/// From the keyboard, Tab reaches the events. Left and Right move one by a step,
/// Up and Down to the next row, Shift with Left and Right resize it, Enter opens
/// it and Delete removes it.
///
/// ```
/// use ravenrs_reactive::State;
/// use ravenrs_widgets::{Date, Resource, Scheduler, SchedulerEvent, TimeAxis, TimePoint, Ui};
///
/// let first = Date::new(2026, 11, 1).unwrap();
/// let axis = State::new(TimeAxis::days(first, 30));
/// let start = TimePoint::from_date(first.add_days(4));
/// let scheduler = Scheduler::new(axis.binding())
///     .resources([Resource::new(1, "Ana"), Resource::new(2, "Bruno")])
///     .events([SchedulerEvent::new(7, 1, start, start.plus_days(3), "Férias")])
///     .on_change(|change| println!("{change:?}"));
/// let mut ui = Ui::new(scheduler, ravenrs_style::Theme::raven_light());
/// ui.update();
/// ```
pub struct Scheduler {
    axis: Binding<TimeAxis>,
    resources: Vec<Resource>,
    events: Vec<SchedulerEvent>,
    ranges: Vec<SchedulerRange>,
    now: Option<TimePoint>,
    selected: Option<Binding<Option<u64>>>,
    resource_width: f32,
    read_only: bool,
    toolbar: bool,
    on_change: Option<OnChange>,
    on_create: Option<OnChange>,
    on_open: Option<Rc<dyn Fn(u64)>>,
    on_delete: Option<Rc<dyn Fn(u64)>>,
    can_drop: Option<CanDrop>,
}

impl fmt::Debug for Scheduler {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Scheduler")
            .field("resources", &self.resources.len())
            .field("events", &self.events.len())
            .finish_non_exhaustive()
    }
}

impl Scheduler {
    /// A scheduler over this stretch of time. Zooming and moving through time change `axis`.
    pub fn new(axis: Binding<TimeAxis>) -> Self {
        Self {
            axis,
            resources: Vec::new(),
            events: Vec::new(),
            ranges: Vec::new(),
            now: None,
            selected: None,
            resource_width: 200.0,
            read_only: false,
            toolbar: true,
            on_change: None,
            on_create: None,
            on_open: None,
            on_delete: None,
            can_drop: None,
        }
    }

    /// The rows.
    pub fn resources(mut self, resources: impl IntoIterator<Item = Resource>) -> Self {
        self.resources = resources.into_iter().collect();
        self
    }

    /// The bars. Those on no row are ignored.
    pub fn events(mut self, events: impl IntoIterator<Item = SchedulerEvent>) -> Self {
        self.events = events.into_iter().collect();
        self
    }

    /// Shaded stretches of time across all rows.
    pub fn ranges(mut self, ranges: impl IntoIterator<Item = SchedulerRange>) -> Self {
        self.ranges = ranges.into_iter().collect();
        self
    }

    /// Draws a line at this moment, and offers a button to come back to it.
    pub fn now(mut self, now: TimePoint) -> Self {
        self.now = Some(now);
        self
    }

    /// Holds the selected event.
    pub fn selection(mut self, selected: Binding<Option<u64>>) -> Self {
        self.selected = Some(selected);
        self
    }

    /// The width of the resources column (200 by default).
    pub fn resource_width(mut self, width: f32) -> Self {
        self.resource_width = width.max(80.0);
        self
    }

    /// Nothing can be moved, resized or created.
    pub fn read_only(mut self, read_only: bool) -> Self {
        self.read_only = read_only;
        self
    }

    /// The buttons to move through time and zoom (on by default).
    pub fn toolbar(mut self, show: bool) -> Self {
        self.toolbar = show;
        self
    }

    /// An event was moved or resized.
    pub fn on_change(mut self, handler: impl Fn(SchedulerChange) + 'static) -> Self {
        self.on_change = Some(Rc::new(handler));
        self
    }

    /// A stretch of time was drawn on a row, by dragging or double-clicking.
    pub fn on_create(mut self, handler: impl Fn(SchedulerChange) + 'static) -> Self {
        self.on_create = Some(Rc::new(handler));
        self
    }

    /// An event was double-clicked, or Enter was pressed on it.
    pub fn on_open(mut self, handler: impl Fn(u64) + 'static) -> Self {
        self.on_open = Some(Rc::new(handler));
        self
    }

    /// Delete was pressed on an event.
    pub fn on_delete(mut self, handler: impl Fn(u64) + 'static) -> Self {
        self.on_delete = Some(Rc::new(handler));
        self
    }

    /// Says whether a place is allowed. A refused one shows in red and changes nothing.
    pub fn can_drop(mut self, rule: impl Fn(&SchedulerChange) -> bool + 'static) -> Self {
        self.can_drop = Some(Rc::new(rule));
        self
    }
}

/// A line of the timeline.
#[derive(Clone)]
enum Row {
    Group { name: String, count: usize, collapsed: bool, y: f32 },
    Resource { index: usize, y: f32, height: f32 },
}

impl Row {
    fn y(&self) -> f32 {
        match self {
            Row::Group { y, .. } | Row::Resource { y, .. } => *y,
        }
    }

    fn height(&self) -> f32 {
        match self {
            Row::Group { .. } => GROUP_HEIGHT,
            Row::Resource { height, .. } => *height,
        }
    }
}

/// What a drag is doing to an event.
#[derive(Clone, Copy, Debug, PartialEq)]
enum DragKind {
    /// Along with the pointer; `grab` is how far into the event it was taken.
    Move {
        grab: i64,
    },
    ResizeStart,
    ResizeEnd,
}

/// Where the event under a drag would land.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Preview {
    change: SchedulerChange,
    allowed: bool,
}

/// What a drag or a key needs, shared by every bar and by the background.
#[derive(Clone)]
struct Editing {
    axis: TimeAxis,
    rows: Rc<Vec<(u64, f32, f32)>>,
    preview: State<Option<Preview>>,
    on_change: Option<OnChange>,
    on_create: Option<OnChange>,
    can_drop: Option<CanDrop>,
}

impl Editing {
    fn resource_at(&self, y: f32) -> Option<u64> {
        let found = self.rows.iter().find(|(_, top, height)| y >= *top && y < top + height);
        found.or_else(|| if y < 0.0 { self.rows.first() } else { self.rows.last() }).map(|r| r.0)
    }

    fn allowed(&self, change: &SchedulerChange) -> bool {
        self.can_drop.as_ref().is_none_or(|rule| rule(change))
    }

    fn propose(&self, change: SchedulerChange) {
        self.preview.set(Some(Preview { change, allowed: self.allowed(&change) }));
    }

    /// Applies a finished change if it is allowed and really is one.
    fn commit(&self, original: Option<SchedulerChange>) {
        let Some(preview) = *self.preview.peek() else { return };
        self.preview.set(None);
        if !preview.allowed || original == Some(preview.change) {
            return;
        }
        let handler = if preview.change.id.is_some() { &self.on_change } else { &self.on_create };
        if let Some(handler) = handler {
            handler(preview.change);
        }
    }

    /// Follows a drag on an event or on one of its ends.
    fn follow(&self, event: &SchedulerEvent, kind: DragKind, origin: &Cell<Point>, drag: Drag, bar: Point) {
        if drag.phase == DragPhase::Start {
            origin.set(Point::new(drag.container.x() - bar.x, drag.container.y() - bar.y));
        }
        let at = Point::new(drag.position.x - origin.get().x, drag.position.y - origin.get().y);
        let time = self.axis.snap(self.axis.time_at(at.x));
        let original = SchedulerChange {
            id: Some(event.id),
            resource: event.resource,
            start: event.start,
            end: event.end,
        };
        let step = self.axis.snap_minutes();
        let change = match kind {
            DragKind::Move { grab } => {
                let start = self.axis.snap(self.axis.time_at(at.x).plus_minutes(-grab));
                let length = event.end.minutes() - event.start.minutes();
                let resource = self.resource_at(at.y).unwrap_or(event.resource);
                SchedulerChange { start, end: start.plus_minutes(length), resource, ..original }
            }
            DragKind::ResizeStart => {
                SchedulerChange { start: time.min(event.end.plus_minutes(-step)), ..original }
            }
            DragKind::ResizeEnd => {
                SchedulerChange { end: time.max(event.start.plus_minutes(step)), ..original }
            }
        };
        match drag.phase {
            DragPhase::End => {
                self.propose(change);
                self.commit(Some(original));
            }
            _ => self.propose(change),
        }
    }
}

struct EndHandle {
    event: SchedulerEvent,
    kind: DragKind,
    editing: Editing,
    bar: Point,
    width: f32,
}

impl View for EndHandle {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let at_start = self.kind == DragKind::ResizeStart;
        let x = if at_start { 0.0 } else { (self.width - HANDLE_WIDTH).max(0.0) };
        let node = cx.container(
            LayoutStyle {
                absolute: Some((x, 0.0)),
                width: Some(HANDLE_WIDTH),
                height: Some(LANE_HEIGHT - 4.0),
                ..LayoutStyle::leaf()
            },
            &[],
            Paint::None,
        );
        let origin = cx.use_state(|| Rc::new(Cell::new(Point::ZERO))).get();
        let (event, kind, editing) = (self.event.clone(), self.kind, self.editing.clone());
        let bar = self.bar;
        cx.on_drag(
            node,
            // A handle's parent is the bar, so the timeline's origin comes from where the bar is.
            Rc::new(move |drag: Drag| editing.follow(&event, kind, &origin, drag, bar)),
        );
        cx.cursor(node, CursorIcon::ColResize);
        node
    }
}

struct Bar<'a> {
    event: &'a SchedulerEvent,
    /// Where it is drawn, in the timeline's content.
    frame: Rect,
    title: String,
    /// It is the one a drag is moving, and this says if that place is allowed.
    dragging: Option<bool>,
    selected: bool,
    /// The event keyboard users reach with Tab.
    tab_stop: bool,
    editable: bool,
    editing: Editing,
    /// Row by row, for Up and Down.
    selection: Option<Binding<Option<u64>>>,
    on_open: Option<Rc<dyn Fn(u64)>>,
    on_delete: Option<Rc<dyn Fn(u64)>>,
}

impl Bar<'_> {
    fn color(&self, theme: &Theme) -> ravenrs_core::Color {
        match self.dragging {
            Some(false) => theme.color(Semantic::Danger),
            _ if self.event.done => theme.color(Semantic::InkMuted),
            _ => self.event.tone.color(theme),
        }
    }

    fn key_handler(&self) -> crate::KeyHandler {
        let event = self.event.clone();
        let (editing, editable) = (self.editing.clone(), self.editable && !event.locked);
        let (on_open, on_delete) = (self.on_open.clone(), self.on_delete.clone());
        Rc::new(move |key, modifiers| {
            let step = editing.axis.snap_minutes();
            let original = SchedulerChange {
                id: Some(event.id),
                resource: event.resource,
                start: event.start,
                end: event.end,
            };
            let change = match key {
                Key::Enter => {
                    if let Some(open) = &on_open {
                        open(event.id);
                    }
                    return on_open.is_some();
                }
                Key::Delete | Key::Backspace if editable => {
                    if let Some(delete) = &on_delete {
                        delete(event.id);
                    }
                    return on_delete.is_some();
                }
                Key::ArrowLeft | Key::ArrowRight if editable => {
                    let by = if *key == Key::ArrowLeft { -step } else { step };
                    if modifiers.shift && !event.resizable {
                        return true;
                    }
                    if modifiers.shift {
                        let end = event.end.plus_minutes(by);
                        SchedulerChange { end: end.max(event.start.plus_minutes(step)), ..original }
                    } else {
                        SchedulerChange {
                            start: event.start.plus_minutes(by),
                            end: event.end.plus_minutes(by),
                            ..original
                        }
                    }
                }
                Key::ArrowUp | Key::ArrowDown if editable => {
                    let here = editing.rows.iter().position(|r| r.0 == event.resource);
                    let there =
                        here.and_then(|i| if *key == Key::ArrowUp { i.checked_sub(1) } else { Some(i + 1) });
                    let Some(row) = there.and_then(|i| editing.rows.get(i)) else { return true };
                    SchedulerChange { resource: row.0, ..original }
                }
                _ => return false,
            };
            editing.propose(change);
            editing.commit(Some(original));
            true
        })
    }
}

impl View for Bar<'_> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let e = self.event;
        let color = self.color(&theme);
        let hovered = cx.is_hovered() || cx.is_focus_visible();
        let solid = !e.done && self.dragging.is_none();
        let fill = if self.selected || hovered && solid {
            color
        } else {
            color.with_alpha(if e.done { 0.16 } else { 0.8 })
        };
        let ink = if e.done { theme.color(Semantic::Ink) } else { theme.color(Semantic::OnAccent) };
        let mut children = Vec::new();
        if e.done {
            children.push(util::icon(cx, IconName::Check, 12.0, color));
        }
        let title =
            cx.child(&Text::new(self.title.clone()).style(TextStyle::Caption).foreground(ink).truncate());
        children.push(cx.container(
            LayoutStyle {
                grow: 1.0,
                min_width: Some(0.0),
                overflow_hidden: true,
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &[title],
            Paint::None,
        ));
        let bar_point = Point::new(self.frame.x(), self.frame.y());
        if self.editable && !e.locked && e.resizable {
            for kind in [DragKind::ResizeStart, DragKind::ResizeEnd] {
                let handle = EndHandle {
                    event: e.clone(),
                    kind,
                    editing: self.editing.clone(),
                    bar: bar_point,
                    width: self.frame.width(),
                };
                children.push(cx.child(&handle));
            }
        }
        let border = (self.selected || self.dragging.is_some())
            .then(|| Border { width: 1.5, color: theme.color(Semantic::Ink).with_alpha(0.55) });
        let node = cx.container(
            LayoutStyle {
                absolute: Some((self.frame.x(), self.frame.y())),
                width: Some(self.frame.width()),
                height: Some(self.frame.height()),
                padding: Insets::symmetric(0.0, 8.0),
                spacing: 4.0,
                align: Align::Center,
                overflow_hidden: true,
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &children,
            Paint::Quad(QuadPaint { corner_radius: theme.radii.sm, border, ..QuadPaint::new(fill) }),
        );
        let labels = cx.labels();
        let range = time_range(&labels, self.editing.axis.scale, e.start, e.end);
        cx.semantics(
            node,
            Semantics::new(Role::Button).name(e.title.clone()).description(range).selected(self.selected),
        );
        if self.tab_stop {
            cx.focusable(node, theme.radii.sm);
            cx.on_key(node, self.key_handler());
        }
        let (selection, id) = (self.selection.clone(), e.id);
        cx.on_click(
            node,
            Rc::new(move || {
                if let Some(selection) = &selection {
                    selection.set(Some(id));
                }
            }),
        );
        if let Some(open) = self.on_open.clone() {
            cx.on_double_click(node, Rc::new(move || open(id)));
        }
        if self.editable && !e.locked {
            let origin = cx.use_state(|| Rc::new(Cell::new(Point::ZERO))).get();
            let grab = cx.use_state(|| Rc::new(Cell::new(0_i64))).get();
            let (event, editing) = (e.clone(), self.editing.clone());
            cx.on_drag(
                node,
                Rc::new(move |drag: Drag| {
                    if drag.phase == DragPhase::Start {
                        // The pointer stays over the same part of the event while it moves.
                        let inside = drag.position.x - drag.frame.x();
                        let minutes = (f64::from(inside)
                            / f64::from(editing.axis.x_of(editing.axis.start.plus_days(1)).max(0.001))
                            * 1440.0) as i64;
                        grab.set(minutes);
                    }
                    let kind = DragKind::Move { grab: grab.get() };
                    let content_origin = Point::ZERO;
                    editing.follow(&event, kind, &origin, drag, content_origin);
                }),
            );
        }
        cx.cursor(node, CursorIcon::Pointer);
        node
    }
}

/// The empty part of the timeline: draws a new event, and holds the bars.
struct Lanes {
    /// Its size.
    width: f32,
    height: f32,
    children: Rc<Vec<NodeId>>,
}

impl Lanes {
    fn build(&self, cx: &mut BuildCx<'_>, s: &Scheduler, editing: &Editing) -> NodeId {
        let node = cx.container(
            LayoutStyle {
                width: Some(self.width),
                height: Some(self.height),
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &self.children,
            Paint::None,
        );
        if !s.read_only && s.on_create.is_some() {
            let from: Rc<Cell<Option<(TimePoint, u64)>>> = cx.use_state(|| Rc::new(Cell::new(None))).get();
            let editor = editing.clone();
            cx.on_drag(
                node,
                Rc::new(move |drag: Drag| {
                    let at = Point::new(drag.position.x - drag.frame.x(), drag.position.y - drag.frame.y());
                    let time = editor.axis.snap(editor.axis.time_at(at.x));
                    if drag.phase == DragPhase::Start
                        && let Some(resource) = editor.resource_at(at.y)
                    {
                        from.set(Some((time, resource)));
                    }
                    let Some((begin, resource)) = from.get() else { return };
                    let step = editor.axis.snap_minutes();
                    let (start, end) = if time >= begin {
                        (begin, time.max(begin.plus_minutes(step)))
                    } else {
                        (time, begin)
                    };
                    editor.propose(SchedulerChange { id: None, resource, start, end });
                    if drag.phase == DragPhase::End {
                        from.set(None);
                        editor.commit(None);
                    }
                }),
            );
        }
        node
    }
}

/// A moment as a person reads it on this scale.
fn time_label(labels: &Labels, scale: TimeScale, time: TimePoint) -> String {
    let date = (labels.short_date)(time.date());
    match scale {
        TimeScale::Hours => {
            let (hour, minute) = time.hour_minute();
            format!("{date} {hour:02}:{minute:02}")
        }
        _ => date,
    }
}

/// A stretch of time as a person reads it. Days are whole, so the last one shown is the last one in it.
fn time_range(labels: &Labels, scale: TimeScale, start: TimePoint, end: TimePoint) -> String {
    let last = if scale == TimeScale::Hours { end } else { end.plus_minutes(-1).max(start) };
    let (from, to) = (time_label(labels, scale, start), time_label(labels, scale, last));
    if from == to { from } else { format!("{from} – {to}") }
}

fn tick_text(labels: &Labels, scale: TimeScale, time: TimePoint) -> String {
    let date = time.date();
    match scale {
        TimeScale::Hours => format!("{:02}", time.hour_minute().0),
        TimeScale::Days => date.day().to_string(),
        TimeScale::Weeks => format!("{:02}", date.day()),
        TimeScale::Months => {
            let name = &labels.months[usize::from(date.month().clamp(1, 12)) - 1];
            name.chars().take(3).collect()
        }
    }
}

fn group_text(labels: &Labels, scale: TimeScale, time: TimePoint) -> String {
    let date = time.date();
    match scale {
        TimeScale::Hours => (labels.short_date)(date),
        TimeScale::Days | TimeScale::Weeks => labels.month_title(date),
        TimeScale::Months => date.year().to_string(),
    }
}

/// A filled stretch at a fixed place.
fn band(cx: &mut BuildCx<'_>, frame: Rect, fill: ravenrs_core::Color) -> NodeId {
    let node = cx.container(
        LayoutStyle {
            absolute: Some((frame.x(), frame.y())),
            width: Some(frame.width()),
            height: Some(frame.height()),
            ..LayoutStyle::leaf()
        },
        &[],
        Paint::Quad(QuadPaint::new(fill)),
    );
    cx.semantics(node, Semantics::hidden());
    node
}

/// A text at a fixed place.
fn placed_text(
    cx: &mut BuildCx<'_>,
    frame: Rect,
    content: &str,
    style: TextStyle,
    color: ravenrs_core::Color,
) -> NodeId {
    let label = util::label(cx, content, style, None, color);
    cx.container(
        LayoutStyle {
            absolute: Some((frame.x(), frame.y())),
            width: Some(frame.width()),
            height: Some(frame.height()),
            align: Align::Center,
            justify: Justify::Center,
            overflow_hidden: true,
            ..LayoutStyle::stack(Axis::Horizontal)
        },
        &[label],
        Paint::None,
    )
}

impl Scheduler {
    /// Groups stay together, in the order their first resource appears.
    fn rows(&self, lanes_of: &HashMap<u64, usize>, collapsed: &BTreeSet<String>) -> (Vec<Row>, f32) {
        let mut order: Vec<Option<&str>> = Vec::new();
        for resource in &self.resources {
            let group = resource.group.as_deref();
            if !order.contains(&group) {
                order.push(group);
            }
        }
        let mut rows = Vec::with_capacity(self.resources.len() + order.len());
        let mut y = 0.0;
        for group in order {
            let members: Vec<usize> =
                (0..self.resources.len()).filter(|i| self.resources[*i].group.as_deref() == group).collect();
            let mut hidden = false;
            if let Some(name) = group {
                hidden = collapsed.contains(name);
                rows.push(Row::Group { name: name.to_owned(), count: members.len(), collapsed: hidden, y });
                y += GROUP_HEIGHT;
            }
            if hidden {
                continue;
            }
            for index in members {
                let lanes = lanes_of.get(&self.resources[index].id).copied().unwrap_or(1);
                let height = lanes as f32 * LANE_HEIGHT + 2.0 * ROW_PADDING;
                rows.push(Row::Resource { index, y, height });
                y += height;
            }
        }
        (rows, y)
    }

    fn toolbar_view(&self, cx: &mut BuildCx<'_>, axis: TimeAxis) -> NodeId {
        let labels = cx.labels();
        let (back, ahead, zoom_in, zoom_out) =
            (self.axis.clone(), self.axis.clone(), self.axis.clone(), self.axis.clone());
        let mut items = vec![
            cx.child(
                &IconButton::new(IconName::ChevronLeft)
                    .label(labels.previous_month.clone())
                    .size(ButtonSize::Small)
                    .on_click(move || back.set(axis.shifted(-1))),
            ),
            cx.child(
                &IconButton::new(IconName::ChevronRight)
                    .label(labels.next_month.clone())
                    .size(ButtonSize::Small)
                    .on_click(move || ahead.set(axis.shifted(1))),
            ),
        ];
        let title = format!(
            "{} – {}",
            (labels.short_date)(axis.start.date()),
            (labels.short_date)(axis.end.plus_minutes(-1).date())
        );
        items.push(util::label(cx, &title, TextStyle::Headline, None, cx.theme().color(Semantic::Ink)));
        if let Some(now) = self.now {
            let today = self.axis.clone();
            items.push(
                cx.child(
                    &Button::new(labels.today.clone())
                        .style(ButtonStyle::Ghost)
                        .size(ButtonSize::Small)
                        .on_click(move || {
                            let length = axis.end.minutes() - axis.start.minutes();
                            let start = axis.snap(now.plus_minutes(-length / 8));
                            today.set(TimeAxis { start, end: start.plus_minutes(length), ..axis });
                        }),
                ),
            );
        }
        items.push(cx.container(LayoutStyle { grow: 1.0, ..LayoutStyle::leaf() }, &[], Paint::None));
        let mut out =
            IconButton::new(IconName::ZoomOut).label(labels.zoom_out.clone()).size(ButtonSize::Small);
        if let Some(farther) = axis.zoomed_out() {
            out = out.on_click(move || zoom_out.set(farther));
        }
        items.push(cx.child(&out));
        let mut near =
            IconButton::new(IconName::ZoomIn).label(labels.zoom_in.clone()).size(ButtonSize::Small);
        if let Some(nearer) = axis.zoomed_in() {
            near = near.on_click(move || zoom_in.set(nearer));
        }
        items.push(cx.child(&near));
        cx.container(
            LayoutStyle {
                spacing: 6.0,
                align: Align::Center,
                padding: Insets::symmetric(8.0, 8.0),
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &items,
            Paint::None,
        )
    }
}

impl View for Scheduler {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let labels = cx.labels();
        let axis = self.axis.get();
        let preview: State<Option<Preview>> = cx.use_state(|| None);
        let collapsed: State<BTreeSet<String>> = cx.use_state(BTreeSet::new);
        let own_selection: State<Option<u64>> = cx.use_state(|| None);
        let selection = self.selected.clone().unwrap_or_else(|| own_selection.binding());
        let selected = selection.get();
        let current = preview.get();

        // Where each event is drawn, the one being dragged at its new place.
        let placed: Vec<(usize, SchedulerChange)> = self
            .events
            .iter()
            .enumerate()
            .map(|(i, e)| {
                let here =
                    SchedulerChange { id: Some(e.id), resource: e.resource, start: e.start, end: e.end };
                (i, current.filter(|p| p.change.id == Some(e.id)).map_or(here, |p| p.change))
            })
            .collect();
        let mut by_resource: HashMap<u64, Vec<usize>> = HashMap::new();
        for (slot, (_, change)) in placed.iter().enumerate() {
            by_resource.entry(change.resource).or_default().push(slot);
        }
        let mut lane_of = vec![0_usize; placed.len()];
        let mut lanes_of: HashMap<u64, usize> = HashMap::new();
        for (resource, slots) in &by_resource {
            let spans: Vec<(TimePoint, TimePoint)> =
                slots.iter().map(|s| (placed[*s].1.start, placed[*s].1.end)).collect();
            let (lanes, count) = assign_lanes(&spans);
            for (slot, lane) in slots.iter().zip(lanes) {
                lane_of[*slot] = lane;
            }
            lanes_of.insert(*resource, count);
        }
        let (rows, total_height) = self.rows(&lanes_of, &collapsed.get());
        let top_of: HashMap<u64, f32> = rows
            .iter()
            .filter_map(|row| match row {
                Row::Resource { index, y, .. } => Some((self.resources[*index].id, *y)),
                Row::Group { .. } => None,
            })
            .collect();
        let row_lines: Rc<Vec<(u64, f32, f32)>> = Rc::new(
            rows.iter()
                .filter_map(|row| match row {
                    Row::Resource { index, y, height, .. } => Some((self.resources[*index].id, *y, *height)),
                    Row::Group { .. } => None,
                })
                .collect(),
        );

        let scroll = cx.scroll_offset();
        let window = cx.viewport();
        // Opens at now, once its size is known, with some of what came before in view.
        let opened: Rc<Cell<bool>> = cx.use_state(|| Rc::new(Cell::new(false))).get();
        if !opened.get() && self.now.is_some() {
            match cx.scroll_viewport() {
                Some(size) => {
                    opened.set(true);
                    if let Some(now) = self.now {
                        cx.set_scroll_offset(Point::new((axis.x_of(now) - size.width / 4.0).max(0.0), 0.0));
                    }
                }
                // The size comes from the first layout, and nothing else asks for a second build.
                None => {
                    let now = cx.now();
                    cx.wake_at(now);
                }
            }
        }
        let view = cx.scroll_viewport().unwrap_or(window);
        let (left, right) = (scroll.x - OVERSCAN, scroll.x + view.width + OVERSCAN);
        let (top, bottom) = (scroll.y - OVERSCAN, scroll.y + view.height + OVERSCAN);
        let width = axis.width();
        let line = theme.color(Semantic::Border);
        let muted = theme.color(Semantic::InkMuted);
        let ink = theme.color(Semantic::Ink);

        let editing = Editing {
            axis,
            rows: row_lines,
            preview: preview.clone(),
            on_change: self.on_change.clone(),
            on_create: self.on_create.clone(),
            can_drop: self.can_drop.clone(),
        };

        // The header: months or days above, ticks below; it follows the sideways scroll.
        let ticks = axis.ticks(left, right);
        let mut header_cells = Vec::new();
        let mut previous = None;
        for (i, tick) in ticks.iter().enumerate() {
            if axis.starts_group(tick.start, previous) {
                let until = ticks[i + 1..]
                    .iter()
                    .find(|t| axis.starts_group(t.start, Some(tick.start)))
                    .map_or(right.min(width), |t| t.x);
                let label = group_text(&labels, axis.scale, tick.start);
                let cell = util::label(cx, &label, TextStyle::Caption, Some(Weight::SEMIBOLD), ink);
                // It stays at the edge while its month is in view, however far that has scrolled.
                let x = (tick.x.max(scroll.x) + 6.0).min(until);
                header_cells.push(cx.container(
                    LayoutStyle {
                        absolute: Some((x, 0.0)),
                        width: Some((until - x).max(0.0)),
                        height: Some(HEADER_ROW_HEIGHT),
                        align: Align::Center,
                        overflow_hidden: true,
                        ..LayoutStyle::stack(Axis::Horizontal)
                    },
                    &[cell],
                    Paint::None,
                ));
            }
            previous = Some(tick.start);
            let text = tick_text(&labels, axis.scale, tick.start);
            let frame = Rect::new(tick.x, HEADER_ROW_HEIGHT, tick.width, HEADER_ROW_HEIGHT);
            header_cells.push(placed_text(cx, frame, &text, TextStyle::Caption, muted));
            header_cells.push(band(cx, Rect::new(tick.x, HEADER_ROW_HEIGHT, 1.0, HEADER_ROW_HEIGHT), line));
        }
        let header_row = cx.container(
            LayoutStyle {
                width: Some(width),
                height: Some(2.0 * HEADER_ROW_HEIGHT),
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &header_cells,
            Paint::None,
        );
        cx.translate(header_row, Point::new(-scroll.x, 0.0));
        let header_view = cx.container(
            LayoutStyle {
                grow: 1.0,
                min_width: Some(0.0),
                height: Some(2.0 * HEADER_ROW_HEIGHT),
                overflow_hidden: true,
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &[header_row],
            Paint::Quad(QuadPaint::new(theme.color(Semantic::Surface))),
        );
        cx.clip(header_view);
        let corner_text =
            util::label(cx, &labels.scheduler_resources, TextStyle::Caption, Some(Weight::SEMIBOLD), muted);
        let corner = cx.container(
            LayoutStyle {
                width: Some(self.resource_width),
                height: Some(2.0 * HEADER_ROW_HEIGHT),
                padding: Insets::symmetric(0.0, 10.0),
                align: Align::Center,
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &[corner_text],
            Paint::Quad(QuadPaint::new(theme.color(Semantic::Surface))),
        );
        let header = cx.container(
            LayoutStyle { align: Align::Stretch, ..LayoutStyle::stack(Axis::Horizontal) },
            &[corner, header_view],
            Paint::None,
        );

        // Behind the bars: weekends, marked stretches, row lines.
        let mut under = Vec::new();
        if axis.scale == TimeScale::Days || axis.scale == TimeScale::Hours {
            for tick in &ticks {
                let weekday = tick.start.date().weekday();
                if weekday == 0 || weekday == 6 {
                    under.push(band(
                        cx,
                        Rect::new(tick.x, 0.0, tick.width, total_height),
                        ink.with_alpha(0.035),
                    ));
                }
            }
        }
        for tick in &ticks {
            under.push(band(cx, Rect::new(tick.x, 0.0, 1.0, total_height), line.faded(0.5)));
        }
        for range in &self.ranges {
            let (from, to) = (axis.x_of(range.start), axis.x_of(range.end));
            if to > left && from < right {
                let color = range.tone.color(&theme).with_alpha(0.12);
                under.push(band(cx, Rect::new(from, 0.0, (to - from).max(1.0), total_height), color));
            }
        }
        let mut names = Vec::new();
        for row in rows.iter().filter(|r| r.y() + r.height() > top && r.y() < bottom) {
            let y = row.y();
            under.push(band(cx, Rect::new(0.0, y + row.height() - 1.0, width, 1.0), line));
            match row {
                Row::Group { name, count, collapsed: is_collapsed, .. } => {
                    under.push(band(cx, Rect::new(0.0, y, width, GROUP_HEIGHT), ink.with_alpha(0.03)));
                    let chevron = if *is_collapsed { IconName::ChevronRight } else { IconName::ChevronDown };
                    let icon = util::icon(cx, chevron, 14.0, muted);
                    let label = util::label(cx, name, TextStyle::Caption, Some(Weight::SEMIBOLD), ink);
                    let note = util::label(cx, &count.to_string(), TextStyle::Caption, None, muted);
                    let node = cx.keyed_child(
                        0x6772_6f75_7000_0000 ^ hash_text(name),
                        &GroupHeader {
                            name: name.clone(),
                            collapsed: *is_collapsed,
                            state: collapsed.clone(),
                            children: Rc::new(vec![icon, label, note]),
                            frame: Rect::new(0.0, y, self.resource_width, GROUP_HEIGHT),
                        },
                    );
                    names.push(node);
                }
                Row::Resource { index, height, .. } => {
                    let resource = &self.resources[*index];
                    let name =
                        cx.child(&Text::new(resource.name.clone()).style(TextStyle::Callout).truncate());
                    let mut lines = vec![name];
                    if let Some(subtitle) = &resource.subtitle {
                        lines.push(
                            cx.child(
                                &Text::new(subtitle.clone())
                                    .style(TextStyle::Caption)
                                    .foreground(Semantic::InkMuted)
                                    .truncate(),
                            ),
                        );
                    }
                    let node = cx.container(
                        LayoutStyle {
                            absolute: Some((0.0, y)),
                            width: Some(self.resource_width),
                            height: Some(*height),
                            padding: Insets::symmetric(0.0, 10.0),
                            justify: Justify::Center,
                            align: Align::Stretch,
                            overflow_hidden: true,
                            ..LayoutStyle::stack(Axis::Vertical)
                        },
                        &lines,
                        Paint::None,
                    );
                    cx.semantics(node, Semantics::new(Role::Row).name(resource.name.clone()));
                    names.push(node);
                    names.push(band(cx, Rect::new(0.0, y + height - 1.0, self.resource_width, 1.0), line));
                }
            }
        }

        // The bars in view.
        let tab_stop = selected
            .filter(|id| self.events.iter().any(|e| e.id == *id))
            .or_else(|| self.events.first().map(|e| e.id));
        let mut bars = Vec::new();
        for (slot, (index, change)) in placed.iter().enumerate() {
            let Some(row_top) = top_of.get(&change.resource) else { continue };
            let event = &self.events[*index];
            let x = axis.x_of(change.start);
            let bar_width = (axis.x_of(change.end) - x).max(MIN_BAR_WIDTH);
            let y = row_top + ROW_PADDING + lane_of[slot] as f32 * LANE_HEIGHT + 2.0;
            let is_tab_stop = tab_stop == Some(event.id);
            let in_view = x + bar_width > left && x < right && y + LANE_HEIGHT > top && y < bottom;
            // The keyboard's way in stays built, or Tab could not reach the plan.
            if !in_view && !is_tab_stop {
                continue;
            }
            let dragging = current.filter(|p| p.change.id == Some(event.id)).map(|p| p.allowed);
            let title = match dragging {
                Some(_) => time_range(&labels, axis.scale, change.start, change.end),
                None => event.title.clone(),
            };
            let bar = Bar {
                event,
                frame: Rect::new(x, y, bar_width, LANE_HEIGHT - 4.0),
                title,
                dragging,
                selected: selected == Some(event.id),
                tab_stop: is_tab_stop,
                editable: !self.read_only && self.on_change.is_some(),
                editing: editing.clone(),
                selection: Some(selection.clone()),
                on_open: self.on_open.clone(),
                on_delete: self.on_delete.clone(),
            };
            bars.push(cx.keyed_child(event.id, &bar));
        }
        // What is being drawn.
        if let Some(Preview { change, allowed }) = current.filter(|p| p.change.id.is_none())
            && let Some(row_top) = top_of.get(&change.resource)
        {
            let x = axis.x_of(change.start);
            let frame = Rect::new(
                x,
                row_top + ROW_PADDING + 2.0,
                (axis.x_of(change.end) - x).max(MIN_BAR_WIDTH),
                LANE_HEIGHT - 4.0,
            );
            let color = if allowed { theme.color(Semantic::Accent) } else { theme.color(Semantic::Danger) };
            bars.push(band(cx, frame, color.with_alpha(0.45)));
        }
        if let Some(now) = self.now {
            let x = axis.x_of(now);
            if x >= 0.0 && x <= width {
                bars.push(band(cx, Rect::new(x, 0.0, 2.0, total_height), theme.color(Semantic::Danger)));
            }
        }
        under.extend(bars);

        let lanes = Lanes { width, height: total_height, children: Rc::new(under) };
        let content = lanes.build(cx, self, &editing);
        let body = cx.container(
            LayoutStyle {
                grow: 1.0,
                min_width: Some(0.0),
                min_height: Some(0.0),
                overflow_hidden: true,
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &[content],
            Paint::None,
        );
        cx.scrollable_virtual(body, true);
        cx.semantics(body, Semantics::new(Role::Grid).name(labels.scheduler_resources.clone()));

        // The names follow the vertical scroll.
        let names_content = cx.container(
            LayoutStyle {
                width: Some(self.resource_width),
                height: Some(total_height),
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &names,
            Paint::None,
        );
        cx.translate(names_content, Point::new(0.0, -scroll.y));
        let names_view = cx.container(
            LayoutStyle {
                width: Some(self.resource_width),
                min_height: Some(0.0),
                overflow_hidden: true,
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &[names_content],
            Paint::None,
        );
        cx.clip(names_view);
        let middle = cx.container(
            LayoutStyle {
                grow: 1.0,
                min_height: Some(0.0),
                align: Align::Stretch,
                overflow_hidden: true,
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &[names_view, body],
            Paint::None,
        );

        let mut parts = Vec::new();
        if self.toolbar {
            parts.push(self.toolbar_view(cx, axis));
        }
        parts.push(header);
        parts.push(middle);
        let root = cx.container(
            LayoutStyle {
                grow: 1.0,
                min_height: Some(0.0),
                align: Align::Stretch,
                overflow_hidden: true,
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &parts,
            Paint::Quad(QuadPaint {
                corner_radius: theme.radii.md,
                border: Some(Border { width: 1.0, color: line }),
                ..QuadPaint::new(theme.color(Semantic::SurfaceElevated))
            }),
        );
        cx.clip(root);
        root
    }
}

fn hash_text(text: &str) -> u64 {
    text.bytes()
        .fold(0xcbf2_9ce4_8422_2325, |hash, byte| (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3))
}

struct GroupHeader {
    name: String,
    collapsed: bool,
    state: State<BTreeSet<String>>,
    children: Rc<Vec<NodeId>>,
    frame: Rect,
}

impl View for GroupHeader {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let radius = cx.theme().radii.sm;
        let node = cx.container(
            LayoutStyle {
                absolute: Some((self.frame.x(), self.frame.y())),
                width: Some(self.frame.width()),
                height: Some(self.frame.height()),
                padding: Insets::symmetric(0.0, 8.0),
                spacing: 6.0,
                align: Align::Center,
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &self.children,
            Paint::None,
        );
        cx.semantics(node, Semantics::new(Role::Button).name(self.name.clone()).expanded(!self.collapsed));
        let (state, name) = (self.state.clone(), self.name.clone());
        cx.on_click(
            node,
            Rc::new(move || {
                state.update(|set| {
                    if !set.remove(&name) {
                        set.insert(name.clone());
                    }
                });
            }),
        );
        cx.focusable(node, radius);
        cx.cursor(node, CursorIcon::Pointer);
        node
    }
}
