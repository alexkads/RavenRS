//! A month, a week or a day with what happens on them, and the editor of how something repeats.

use std::cell::Cell;
use std::rc::Rc;

use ravenrs_core::{Border, Color, CursorIcon, Insets, Point, Role};
use ravenrs_layout::{Align, Axis, Justify, LayoutStyle};
use ravenrs_reactive::{Binding, State};
use ravenrs_style::{Semantic, TextStyle, Weight};

use crate::calendar::GridKeys;
use crate::context::{BuildCx, Drag, DragPhase, NodeId, Paint, QuadPaint, Semantics};
use crate::date::Date;
use crate::display::Tone;
use crate::recurrence::{Frequency, RecurrenceEnd, RecurrenceRule};
use crate::scheduler::TimePoint;
use crate::{
    Button, ButtonSize, ButtonStyle, DatePicker, FormField, HStack, IconButton, IconName, SegmentedControl,
    Stepper, Text, ToggleButton, VStack, View, util,
};

/// What runs when an event is moved: its id, its new start and its new end.
type MoveHandler = Rc<dyn Fn(u64, TimePoint, TimePoint)>;

/// Something on a [`EventCalendar`]'s day.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct CalendarEvent {
    /// Given back by [`EventCalendar::on_event`].
    pub id: u64,
    /// Its day.
    pub date: Date,
    /// What it is.
    pub title: String,
    /// Its color; say what it means in the title or next to the scheduler too.
    pub tone: Tone,
    /// It already happened, so it stands back.
    pub done: bool,
    /// When it starts and ends, for one with a time; one without takes its whole day.
    pub time: Option<(TimePoint, TimePoint)>,
    /// It cannot be dragged elsewhere, though it still opens.
    pub locked: bool,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl CalendarEvent {
    /// An event on this day.
    pub fn new(id: u64, date: Date, title: impl Into<String>) -> Self {
        Self { id, date, title: title.into(), tone: Tone::Accent, done: false, time: None, locked: false }
    }

    /// An event from `start` to `end`, on the day it starts. An end before the start
    /// counts as a quarter of an hour.
    pub fn timed(id: u64, start: TimePoint, end: TimePoint, title: impl Into<String>) -> Self {
        let end = if end > start { end } else { start.plus_minutes(15) };
        Self { time: Some((start, end)), ..Self::new(id, start.date(), title) }
    }

    /// Keeps it where it is: it cannot be dragged.
    pub fn locked(mut self, locked: bool) -> Self {
        self.locked = locked;
        self
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
}

/// What an [`EventCalendar`] shows: a month of days, or the hours of a week or a day.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "variants"))]
pub enum CalendarView {
    /// The weeks of a month, the events inside each day.
    #[default]
    Month,
    /// Seven days side by side, Sunday first, with an hour on each line.
    Week,
    /// One day with an hour on each line.
    Day,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl CalendarView {
    /// Every view, in the order the calendar offers them.
    pub const ALL: [Self; 3] = [Self::Month, Self::Week, Self::Day];

    /// The first and last day shown around `anchor`: the whole weeks a month touches,
    /// the week from Sunday, or the day itself. An app loads, or expands a series of,
    /// what falls in it.
    pub fn range(self, anchor: Date) -> (Date, Date) {
        match self {
            Self::Month => {
                let shown = anchor.first_of_month();
                let start = shown.add_days(-i32::from(shown.weekday()));
                let last = shown.add_months(1).add_days(-1);
                let days = i32::try_from(last.to_days() - start.to_days()).unwrap_or(34) / 7 * 7 + 7;
                (start, start.add_days(days - 1))
            }
            Self::Week => {
                let start = anchor.add_days(-i32::from(anchor.weekday()));
                (start, start.add_days(6))
            }
            Self::Day => (anchor, anchor),
        }
    }

    /// The anchor one month, week or day away.
    fn step(self, anchor: Date, forward: bool) -> Date {
        let sign = if forward { 1 } else { -1 };
        match self {
            Self::Month => anchor.first_of_month().add_months(sign),
            Self::Week => anchor.add_days(7 * sign),
            Self::Day => anchor.add_days(sign),
        }
    }
}

/// A month with its events inside each day, for agendas and bookings. Use a
/// [`Calendar`](crate::Calendar) to pick a date and this to see what is on them.
///
/// From the keyboard the grid is one Tab stop: arrows move by day and week, Home
/// and End to the ends of the week, Page Up and Page Down by month, and Enter or
/// Space picks. A day's name reads out what is on it.
///
/// Given a [`view`](Self::view), it also shows a week or a day as a column of hours per
/// day, with each event where its time falls; then an empty hour can create an event
/// ([`on_create`](Self::on_create)) and an event can be dragged to another time or day
/// ([`on_move`](Self::on_move)).
///
/// ```
/// use ravenrs_reactive::State;
/// use ravenrs_widgets::{Date, CalendarEvent, EventCalendar, Tone, Ui};
///
/// let today = Date::new(2026, 11, 15).unwrap();
/// let (month, day) = (State::new(today.first_of_month()), State::new(Some(today)));
/// let calendar = EventCalendar::new(month.binding(), day.binding())
///     .today(today)
///     .events([CalendarEvent::new(1, today, "Aluguel").tone(Tone::Warning)]);
/// let mut ui = Ui::new(calendar, ravenrs_style::Theme::raven_light());
/// ui.update();
/// ```
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct EventCalendar {
    month: Binding<Date>,
    selected: Binding<Option<Date>>,
    events: Vec<CalendarEvent>,
    today: Option<Date>,
    per_day: usize,
    on_event: Option<Rc<dyn Fn(u64)>>,
    view: Option<Binding<CalendarView>>,
    hours: (u32, u32),
    on_create: Option<Rc<dyn Fn(TimePoint, TimePoint)>>,
    on_move: Option<MoveHandler>,
}

impl std::fmt::Debug for EventCalendar {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EventCalendar").field("events", &self.events.len()).finish_non_exhaustive()
    }
}

/// The height of a day with room for its events, in logical pixels.
const DAY_HEIGHT: f32 = 104.0;
/// The height of an hour in a week or a day, in logical pixels.
const HOUR_HEIGHT: f32 = 48.0;
/// The width of the hours on the left of a week or a day.
const GUTTER: f32 = 56.0;
/// The tallest the hours get before they scroll.
const MOST_HOURS_HEIGHT: f32 = 600.0;
/// A drag moves an event by steps of this many minutes.
const SNAP_MINUTES: i64 = 15;

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl EventCalendar {
    /// `month` holds the month shown (any day of it), so the app can load its
    /// events; `selected` holds the day picked.
    pub fn new(month: Binding<Date>, selected: Binding<Option<Date>>) -> Self {
        Self {
            month,
            selected,
            events: Vec::new(),
            today: None,
            per_day: 3,
            on_event: None,
            view: None,
            hours: (0, 24),
            on_create: None,
            on_move: None,
        }
    }

    /// The events; those outside the days shown are ignored.
    pub fn events(mut self, events: impl IntoIterator<Item = CalendarEvent>) -> Self {
        self.events = events.into_iter().collect();
        self
    }

    /// Marks this day, and offers a button to come back to it.
    pub fn today(mut self, today: Date) -> Self {
        self.today = Some(today);
        self
    }

    /// How many events show in a day before the rest become a count (3 by default).
    pub fn events_per_day(mut self, count: usize) -> Self {
        self.per_day = count.max(1);
        self
    }

    /// Called with an event's `id` when it is clicked, after its day is picked.
    pub fn on_event(mut self, handler: impl Fn(u64) + 'static) -> Self {
        self.on_event = Some(Rc::new(handler));
        self
    }

    /// Offers the month, week and day, with `view` holding the one shown. In a week or a
    /// day, the binding given to [`new`](Self::new) holds the day the view is around.
    pub fn view(mut self, view: Binding<CalendarView>) -> Self {
        self.view = Some(view);
        self
    }

    /// The hours a week or a day shows, from `from` to `to` (0 to 24 by default).
    pub fn hours(mut self, from: u32, to: u32) -> Self {
        let to = to.clamp(1, 24);
        self.hours = (from.min(to - 1), to);
        self
    }

    /// Lets an empty hour of a week or a day create an event, called with its start and end.
    pub fn on_create(mut self, handler: impl Fn(TimePoint, TimePoint) + 'static) -> Self {
        self.on_create = Some(Rc::new(handler));
        self
    }

    /// Lets an event that is not [`locked`](CalendarEvent::locked) be dragged in a week or a
    /// day, called with its `id` and where it now starts and ends.
    pub fn on_move(mut self, handler: impl Fn(u64, TimePoint, TimePoint) + 'static) -> Self {
        self.on_move = Some(Rc::new(handler));
        self
    }

    fn shown_view(&self) -> CalendarView {
        self.view.as_ref().map_or(CalendarView::Month, Binding::get)
    }
}

struct EventChip<'a> {
    event: &'a CalendarEvent,
    selected: &'a Binding<Option<Date>>,
    on_event: Option<Rc<dyn Fn(u64)>>,
}

impl View for EventChip<'_> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let event = self.event;
        let hovered = cx.is_hovered();
        let color = if event.done { theme.color(Semantic::InkMuted) } else { event.tone.color(&theme) };
        let fill = color.with_alpha(match (event.done, hovered) {
            (true, false) => 0.08,
            (true, true) | (false, false) => 0.16,
            (false, true) => 0.26,
        });
        let mut children = Vec::new();
        if event.done {
            children.push(util::icon(cx, IconName::Check, 12.0, color));
        }
        let ink = if event.done { color } else { theme.color(Semantic::Ink) };
        let title =
            cx.child(&Text::new(event.title.clone()).style(TextStyle::Caption).foreground(ink).truncate());
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
        let node = cx.container(
            LayoutStyle {
                padding: Insets::symmetric(2.0, 6.0),
                spacing: 4.0,
                align: Align::Center,
                min_width: Some(0.0),
                overflow_hidden: true,
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &children,
            Paint::Quad(QuadPaint { corner_radius: theme.radii.sm, ..QuadPaint::new(fill) }),
        );
        cx.semantics(node, Semantics::new(Role::Button).name(event.title.clone()));
        let (selected, date, id, on_event) =
            (self.selected.clone(), event.date, event.id, self.on_event.clone());
        cx.on_click(
            node,
            Rc::new(move || {
                selected.set(Some(date));
                if let Some(handler) = &on_event {
                    handler(id);
                }
            }),
        );
        cx.cursor(node, CursorIcon::Pointer);
        node
    }
}

struct CalendarDay<'a> {
    date: Date,
    in_month: bool,
    active: bool,
    events: Vec<&'a CalendarEvent>,
    scheduler: &'a EventCalendar,
    keys: &'a GridKeys,
}

impl View for CalendarDay<'_> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let s = self.scheduler;
        let labels = cx.labels();
        let (selected, is_today) = (s.selected.get() == Some(self.date), s.today == Some(self.date));
        let hovered = cx.is_hovered() || cx.is_focus_visible();
        let ink = theme.color(Semantic::Ink);
        let number_color = if is_today {
            theme.color(Semantic::OnAccent)
        } else if self.in_month {
            ink
        } else {
            theme.color(Semantic::InkMuted)
        };
        let weight = is_today.then_some(Weight::SEMIBOLD);
        let number = util::label(cx, &self.date.day().to_string(), TextStyle::Caption, weight, number_color);
        cx.semantics(number, Semantics::hidden());
        let number = cx.container(
            LayoutStyle {
                width: Some(22.0),
                height: Some(22.0),
                align: Align::Center,
                justify: Justify::Center,
                align_self: Some(Align::Fit),
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &[number],
            if is_today {
                Paint::Quad(QuadPaint {
                    corner_radius: 11.0,
                    ..QuadPaint::new(theme.color(Semantic::Accent))
                })
            } else {
                Paint::None
            },
        );
        let mut children = vec![number];
        for event in self.events.iter().take(s.per_day) {
            let chip = EventChip { event, selected: &s.selected, on_event: s.on_event.clone() };
            children.push(cx.keyed_child(event.id, &chip));
        }
        let hidden = self.events.len().saturating_sub(s.per_day);
        if hidden > 0 {
            let more = format!("+{hidden} {}", labels.calendar_more);
            let line = util::label(cx, &more, TextStyle::Caption, None, theme.color(Semantic::InkMuted));
            cx.semantics(line, Semantics::hidden());
            children.push(line);
        }
        let fill = if hovered {
            ink.with_alpha(0.05)
        } else if self.in_month {
            Color::TRANSPARENT
        } else {
            ink.with_alpha(0.02)
        };
        let border = if selected {
            Border { width: 1.5, color: theme.color(Semantic::Accent) }
        } else {
            Border { width: 1.0, color: theme.color(Semantic::Border) }
        };
        let node = cx.container(
            LayoutStyle {
                min_height: Some(DAY_HEIGHT),
                min_width: Some(0.0),
                padding: Insets::uniform(4.0),
                spacing: 3.0,
                align: Align::Stretch,
                overflow_hidden: true,
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &children,
            Paint::Quad(QuadPaint {
                corner_radius: theme.radii.sm,
                border: Some(border),
                ..QuadPaint::new(fill)
            }),
        );
        let mut semantics = Semantics::new(Role::Button).name(labels.long_date(self.date)).selected(selected);
        let mut notes: Vec<String> = Vec::new();
        if is_today {
            notes.push(labels.today.clone());
        }
        notes.extend(self.events.iter().map(|e| e.title.clone()));
        if !notes.is_empty() {
            semantics = semantics.description(notes.join(", "));
        }
        cx.semantics(node, semantics);
        if self.keys.focus_to.get() == Some(self.date) {
            self.keys.focus_to.set(None);
            cx.request_focus();
        }
        if self.active {
            cx.focusable(node, theme.radii.sm);
            cx.on_key(node, self.keys.handler(self.date));
        }
        let (pick, date) = (s.selected.clone(), self.date);
        cx.on_click(node, Rc::new(move || pick.set(Some(date))));
        cx.cursor(node, CursorIcon::Pointer);
        node
    }
}

/// One day of a week or a day: its hours, and its events where their time falls.
struct DayColumn<'a> {
    date: Date,
    events: Vec<&'a CalendarEvent>,
    calendar: &'a EventCalendar,
    /// How many days are side by side, to guess the width before the first layout.
    count: usize,
}

/// `HH:MM`.
fn clock(time: TimePoint) -> String {
    let (hour, minute) = time.hour_minute();
    format!("{hour:02}:{minute:02}")
}

/// Which of side-by-side lanes each event takes, and how many lanes its group of
/// overlapping events needs, for events sorted by start.
fn lanes(times: &[(i64, i64)]) -> Vec<(usize, usize)> {
    let mut placed: Vec<(usize, usize)> = Vec::with_capacity(times.len());
    let mut group_start = 0;
    // Where each lane of the current group is free again.
    let mut free_at: Vec<i64> = Vec::new();
    let mut group_end = i64::MIN;
    for (at, (start, end)) in times.iter().enumerate() {
        if *start >= group_end && at > 0 {
            let width = free_at.len();
            for lane in &mut placed[group_start..] {
                lane.1 = width;
            }
            group_start = at;
            free_at.clear();
        }
        let lane = match free_at.iter().position(|free| *free <= *start) {
            Some(lane) => {
                free_at[lane] = *end;
                lane
            }
            None => {
                free_at.push(*end);
                free_at.len() - 1
            }
        };
        group_end = if at == group_start { *end } else { group_end.max(*end) };
        placed.push((lane, 0));
    }
    let width = free_at.len();
    for lane in &mut placed[group_start..] {
        lane.1 = width;
    }
    placed
}

impl View for DayColumn<'_> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let labels = cx.labels();
        let c = self.calendar;
        let (first, last) = c.hours;
        let width = cx.scroll_viewport().map_or_else(
            || ((cx.viewport().width - GUTTER) / self.count.max(1) as f32 - 4.0).max(40.0),
            |size| size.width,
        );
        let line = theme.color(Semantic::Border);
        let is_today = c.today == Some(self.date);
        let mut children: Vec<NodeId> = (first..last)
            .map(|hour| {
                let slot = HourSlot { date: self.date, hour, calendar: c, line };
                cx.keyed_child(u64::from(hour), &slot)
            })
            .collect();
        // The events, clipped to the hours shown.
        let (top, bottom) = (i64::from(first) * 60, i64::from(last) * 60);
        let day_start = TimePoint::from_date(self.date).minutes();
        let mut timed: Vec<(&CalendarEvent, i64, i64)> = self
            .events
            .iter()
            .filter_map(|event| {
                let (start, end) = event.time?;
                // What falls outside the hours shown stays at their edge, so it is still seen.
                let from = (start.minutes() - day_start).clamp(top, bottom - SNAP_MINUTES);
                let to = (end.minutes() - day_start).clamp(top, bottom);
                Some((*event, from, to.max(from + SNAP_MINUTES)))
            })
            .collect();
        timed.sort_by_key(|(event, from, to)| (*from, -*to, event.id));
        let places = lanes(&timed.iter().map(|(_, from, to)| (*from, *to)).collect::<Vec<_>>());
        for ((event, from, to), (lane, of)) in timed.iter().zip(places) {
            let lane_width = (width - 4.0) / of.max(1) as f32;
            let chip = TimedChip {
                event,
                x: 2.0 + lane as f32 * lane_width,
                y: (*from - top) as f32 / 60.0 * HOUR_HEIGHT,
                width: (lane_width - 2.0).max(8.0),
                height: ((*to - *from) as f32 / 60.0 * HOUR_HEIGHT - 2.0).max(18.0),
                day_width: width,
                calendar: c,
            };
            children.push(cx.keyed_child(event.id ^ (1 << 62), &chip));
        }
        let node = cx.container(
            LayoutStyle {
                grow: 1.0,
                min_width: Some(0.0),
                height: Some((last - first) as f32 * HOUR_HEIGHT),
                align: Align::Stretch,
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &children,
            Paint::Quad(QuadPaint {
                corner_radius: theme.radii.sm,
                border: Some(Border { width: 1.0, color: line }),
                ..QuadPaint::new(if is_today {
                    theme.color(Semantic::Accent).with_alpha(0.04)
                } else {
                    Color::TRANSPARENT
                })
            }),
        );
        cx.report_frame(node);
        cx.semantics(node, Semantics::new(Role::Group).name(labels.long_date(self.date)));
        node
    }
}

/// An hour of a day, which creates an event when the calendar can.
struct HourSlot<'a> {
    date: Date,
    hour: u32,
    calendar: &'a EventCalendar,
    line: Color,
}

impl View for HourSlot<'_> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let labels = cx.labels();
        let theme = *cx.theme();
        let creates = self.calendar.on_create.clone();
        let hovered = creates.is_some() && (cx.is_hovered() || cx.is_focus_visible());
        let rule = cx.container(
            LayoutStyle { height: Some(1.0), ..LayoutStyle::leaf() },
            &[],
            Paint::Quad(QuadPaint::new(if self.hour == self.calendar.hours.0 {
                Color::TRANSPARENT
            } else {
                self.line.with_alpha(0.6)
            })),
        );
        let node = cx.container(
            LayoutStyle {
                height: Some(HOUR_HEIGHT),
                align: Align::Stretch,
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &[rule],
            Paint::Quad(QuadPaint::new(if hovered {
                theme.color(Semantic::Ink).with_alpha(0.04)
            } else {
                Color::TRANSPARENT
            })),
        );
        let Some(create) = creates else {
            cx.semantics(node, Semantics::hidden());
            return node;
        };
        let start = TimePoint::at(self.date, self.hour, 0);
        let name = format!("{} {} {}", labels.calendar_new_event, labels.long_date(self.date), clock(start));
        cx.semantics(node, Semantics::new(Role::Button).name(name));
        cx.focusable(node, 0.0);
        let end = start.plus_minutes(60);
        cx.on_click(node, Rc::new(move || create(start, end)));
        cx.cursor(node, CursorIcon::Pointer);
        node
    }
}

/// An event in a column of hours, placed by its time; dragged, it moves.
struct TimedChip<'a> {
    event: &'a CalendarEvent,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    /// How wide a day is, so a drag sideways counts days.
    day_width: f32,
    calendar: &'a EventCalendar,
}

impl View for TimedChip<'_> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let event = self.event;
        let Some((start, end)) = event.time else { return cx.empty() };
        let shift: State<Option<(f32, f32)>> = cx.use_state(|| None);
        let hovered = cx.is_hovered();
        let color = if event.done { theme.color(Semantic::InkMuted) } else { event.tone.color(&theme) };
        let fill = color.with_alpha(match (event.done, hovered || shift.get().is_some()) {
            (true, false) => 0.10,
            (true, true) | (false, false) => 0.20,
            (false, true) => 0.30,
        });
        let ink = if event.done { color } else { theme.color(Semantic::Ink) };
        let when = format!("{} – {}", clock(start), clock(end));
        let mut lines = vec![
            cx.child(&Text::new(event.title.clone()).style(TextStyle::Caption).foreground(ink).truncate()),
        ];
        if self.height >= 34.0 {
            lines.push(
                cx.child(
                    &Text::new(when.clone())
                        .style(TextStyle::Caption)
                        .font_weight(Weight::REGULAR)
                        .foreground(ink)
                        .truncate(),
                ),
            );
        }
        let (dx, dy) = shift.get().unwrap_or((0.0, 0.0));
        let node = cx.container(
            LayoutStyle {
                absolute: Some((self.x + dx, self.y + dy)),
                width: Some(self.width),
                height: Some(self.height),
                padding: Insets::symmetric(2.0, 6.0),
                overflow_hidden: true,
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &lines,
            Paint::Quad(QuadPaint {
                corner_radius: theme.radii.sm,
                border: Some(Border { width: 1.0, color: color.with_alpha(0.7) }),
                ..QuadPaint::new(fill)
            }),
        );
        cx.semantics(node, Semantics::new(Role::Button).name(event.title.clone()).description(when));
        let dragged = cx.use_state(|| Rc::new(Cell::new(false))).get();
        let c = self.calendar;
        let (selected, date, id, on_event, after_drag) =
            (c.selected.clone(), event.date, event.id, c.on_event.clone(), dragged.clone());
        cx.on_click(
            node,
            Rc::new(move || {
                // The release of a drag is not a click.
                if after_drag.replace(false) {
                    return;
                }
                selected.set(Some(date));
                if let Some(handler) = &on_event {
                    handler(id);
                }
            }),
        );
        if let (Some(on_move), false) = (c.on_move.clone(), event.locked) {
            let begin = cx.use_state(|| Rc::new(Cell::new(Point::ZERO))).get();
            let day_width = self.day_width.max(1.0);
            cx.on_drag(
                node,
                Rc::new(move |drag: Drag| {
                    let (dx, dy) = (drag.position.x - begin.get().x, drag.position.y - begin.get().y);
                    match drag.phase {
                        DragPhase::Start => {
                            dragged.set(false);
                            begin.set(drag.position);
                            shift.set(Some((0.0, 0.0)));
                        }
                        DragPhase::Move => shift.set(Some((dx, dy))),
                        DragPhase::End => {
                            shift.set(None);
                            let steps = (dy / HOUR_HEIGHT * 60.0 / SNAP_MINUTES as f32).round() as i64;
                            let days = (dx / day_width).round() as i64;
                            let minutes = steps * SNAP_MINUTES + days * 1440;
                            if minutes != 0 {
                                dragged.set(true);
                                on_move(id, start.plus_minutes(minutes), end.plus_minutes(minutes));
                            }
                        }
                    }
                }),
            );
        }
        cx.cursor(node, CursorIcon::Pointer);
        node
    }
}

impl View for EventCalendar {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let view = self.shown_view();
        let header = self.header(cx, view);
        let body = match view {
            CalendarView::Month => self.month_grid(cx),
            CalendarView::Week | CalendarView::Day => {
                let (from, to) = view.range(self.month.get());
                let days: Vec<Date> =
                    (0..=(to.to_days() - from.to_days())).map(|i| from.add_days(i as i32)).collect();
                self.hourly(cx, &days)
            }
        };
        cx.container(
            LayoutStyle { spacing: 10.0, align: Align::Stretch, ..LayoutStyle::stack(Axis::Vertical) },
            &[header, body],
            Paint::None,
        )
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose(part = "part1"))]
impl EventCalendar {
    fn header(&self, cx: &mut BuildCx<'_>, view: CalendarView) -> NodeId {
        let theme = *cx.theme();
        let labels = cx.labels();
        let anchor = self.month.get();
        let (back, forward, title) = match view {
            CalendarView::Month => {
                (labels.previous_month.clone(), labels.next_month.clone(), labels.month_title(anchor))
            }
            CalendarView::Week => {
                let (from, to) = view.range(anchor);
                let title = format!("{} – {}", labels.short_date(from), labels.short_date(to));
                (labels.calendar_previous.clone(), labels.calendar_next.clone(), title)
            }
            CalendarView::Day => {
                (labels.calendar_previous.clone(), labels.calendar_next.clone(), labels.long_date(anchor))
            }
        };
        let (prev, next) = (self.month.clone(), self.month.clone());
        let mut header = vec![
            cx.child(
                &IconButton::new(IconName::ChevronLeft)
                    .label(back)
                    .size(ButtonSize::Small)
                    .on_click(move || prev.set(view.step(anchor, false))),
            ),
            cx.child(
                &IconButton::new(IconName::ChevronRight)
                    .label(forward)
                    .size(ButtonSize::Small)
                    .on_click(move || next.set(view.step(anchor, true))),
            ),
            util::label(cx, &title, TextStyle::Headline, None, theme.color(Semantic::Ink)),
        ];
        if let Some(today) = self.today {
            let (month, selected) = (self.month.clone(), self.selected.clone());
            header.push(
                cx.child(
                    &Button::new(labels.today.clone())
                        .style(ButtonStyle::Ghost)
                        .size(ButtonSize::Small)
                        .on_click(move || {
                            month.set(if view == CalendarView::Month {
                                today.first_of_month()
                            } else {
                                today
                            });
                            selected.set(Some(today));
                        }),
                ),
            );
        }
        if let Some(chosen) = &self.view {
            header.push(cx.container(LayoutStyle { grow: 1.0, ..LayoutStyle::leaf() }, &[], Paint::None));
            let (read, write, month, selected) =
                (chosen.clone(), chosen.clone(), self.month.clone(), self.selected.clone());
            let index = Binding::new(
                move || CalendarView::ALL.iter().position(|v| *v == read.get()).unwrap_or(0),
                move |at: usize| {
                    let next = CalendarView::ALL[at.min(CalendarView::ALL.len() - 1)];
                    // The day picked goes along, when it is in what was shown.
                    let (from, to) = write.get().range(month.get());
                    if let Some(day) = selected.get().filter(|day| (from..=to).contains(day)) {
                        month.set(day);
                    }
                    write.set(next);
                },
            );
            header.push(
                cx.child(
                    &SegmentedControl::new(index, labels.calendar_views.to_vec())
                        .label(labels.calendar_view_name.clone()),
                ),
            );
        }
        cx.container(
            LayoutStyle { spacing: 6.0, align: Align::Center, ..LayoutStyle::stack(Axis::Horizontal) },
            &header,
            Paint::None,
        )
    }

    fn month_grid(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let labels = cx.labels();
        let shown = self.month.get().first_of_month();
        let mut cells: Vec<NodeId> = labels
            .weekdays
            .iter()
            .map(|d| {
                let name = util::label(cx, d, TextStyle::Caption, None, theme.color(Semantic::InkMuted));
                // Each day's name already says the date.
                cx.semantics(name, Semantics::hidden());
                name
            })
            .collect();
        let keys = GridKeys {
            month: self.month.clone(),
            cursor: cx.use_state(|| None),
            focus_to: cx.use_state(|| Rc::new(Cell::new(None))).get(),
        };
        let in_shown = |d: &Date| d.first_of_month() == shown;
        let active = [keys.cursor.get(), self.selected.get(), self.today]
            .into_iter()
            .flatten()
            .find(in_shown)
            .unwrap_or(shown);
        // As many weeks as the month touches, so a short month leaves no empty row.
        let (start, last) = CalendarView::Month.range(shown);
        let mut date = start;
        while date <= last {
            let mut events: Vec<&CalendarEvent> = self.events.iter().filter(|e| e.date == date).collect();
            events.sort_by_key(|e| e.time.map(|(start, _)| start));
            let day = CalendarDay {
                date,
                in_month: in_shown(&date),
                active: date == active,
                events,
                scheduler: self,
                keys: &keys,
            };
            cells.push(cx.keyed_child(date.to_days() as u64, &day));
            date = date.add_days(1);
        }
        let grid = cx.container(
            LayoutStyle {
                grid_columns: Some(7),
                spacing: 4.0,
                align_self: Some(Align::Stretch),
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &cells,
            Paint::None,
        );
        cx.semantics(grid, Semantics::new(Role::Calendar).name(labels.month_title(shown)));
        grid
    }

    /// A column of hours for each day, the events placed by their time.
    fn hourly(&self, cx: &mut BuildCx<'_>, days: &[Date]) -> NodeId {
        let theme = *cx.theme();
        let labels = cx.labels();
        let muted = theme.color(Semantic::InkMuted);
        let gutter = |cx: &mut BuildCx<'_>, children: &[NodeId]| {
            cx.container(
                LayoutStyle { width: Some(GUTTER), ..LayoutStyle::stack(Axis::Vertical) },
                children,
                Paint::None,
            )
        };
        let column = LayoutStyle {
            grow: 1.0,
            min_width: Some(0.0),
            spacing: 3.0,
            align: Align::Stretch,
            ..LayoutStyle::stack(Axis::Vertical)
        };
        let row = LayoutStyle { spacing: 4.0, align: Align::Stretch, ..LayoutStyle::stack(Axis::Horizontal) };

        // The names of the days, today marked.
        let mut names = vec![gutter(cx, &[])];
        for day in days {
            let is_today = self.today == Some(*day);
            let text = format!("{} {}", labels.weekdays[usize::from(day.weekday())], day.day());
            let color = if is_today { theme.color(Semantic::Accent) } else { muted };
            let weight = is_today.then_some(Weight::SEMIBOLD);
            let name = util::label(cx, &text, TextStyle::Caption, weight, color);
            cx.semantics(name, Semantics::hidden());
            names.push(cx.container(column, &[name], Paint::None));
        }
        let names = cx.container(row, &names, Paint::None);

        // What takes a whole day, over the hours.
        let whole: Vec<Vec<&CalendarEvent>> = days
            .iter()
            .map(|day| self.events.iter().filter(|e| e.time.is_none() && e.date == *day).collect())
            .collect();
        let mut parts = vec![names];
        if whole.iter().any(|events| !events.is_empty()) {
            let caption = util::label(cx, &labels.calendar_all_day, TextStyle::Caption, None, muted);
            let mut cells = vec![gutter(cx, &[caption])];
            for events in &whole {
                let chips: Vec<NodeId> = events
                    .iter()
                    .map(|event| {
                        let chip =
                            EventChip { event, selected: &self.selected, on_event: self.on_event.clone() };
                        cx.keyed_child(event.id, &chip)
                    })
                    .collect();
                cells.push(cx.container(column, &chips, Paint::None));
            }
            parts.push(cx.container(row, &cells, Paint::None));
        }

        // The hours, and a column for each day.
        let (first, last) = self.hours;
        let hours: Vec<NodeId> = (first..last)
            .map(|hour| {
                let text = util::label(cx, &format!("{hour:02}:00"), TextStyle::Caption, None, muted);
                cx.semantics(text, Semantics::hidden());
                cx.container(
                    LayoutStyle { height: Some(HOUR_HEIGHT), ..LayoutStyle::stack(Axis::Vertical) },
                    &[text],
                    Paint::None,
                )
            })
            .collect();
        let mut columns = vec![gutter(cx, &hours)];
        for day in days {
            let (begins, ends) = (TimePoint::from_date(*day), TimePoint::from_date(day.add_days(1)));
            let events = self
                .events
                .iter()
                .filter(|e| e.time.is_some_and(|(start, end)| start < ends && end > begins))
                .collect();
            let day_column = DayColumn { date: *day, events, calendar: self, count: days.len() };
            columns.push(cx.keyed_child(day.to_days() as u64, &day_column));
        }
        let grid = cx.container(row, &columns, Paint::None);
        let height = (last - first) as f32 * HOUR_HEIGHT;
        // Scrolls, like a scroll view, once the hours are taller than the most it shows.
        let scroll = cx.container(
            LayoutStyle {
                height: Some(height.min(MOST_HOURS_HEIGHT)),
                align: Align::Stretch,
                overflow_hidden: true,
                min_height: Some(0.0),
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &[grid],
            Paint::None,
        );
        cx.scrollable(scroll);
        parts.push(scroll);
        let node = cx.container(
            LayoutStyle { spacing: 4.0, align: Align::Stretch, ..LayoutStyle::stack(Axis::Vertical) },
            &parts,
            Paint::None,
        );
        let name = match days {
            [day] => labels.long_date(*day),
            [first, .., last] => format!("{} – {}", labels.short_date(*first), labels.short_date(*last)),
            [] => String::new(),
        };
        cx.semantics(node, Semantics::new(Role::Calendar).name(name));
        node
    }
}

/// Edits a [`RecurrenceRule`]: how often, every how many, on which weekdays and
/// until when, and says in words what it chose. `start` is the first day, which
/// the rule counts from.
///
/// ```
/// use ravenrs_reactive::State;
/// use ravenrs_widgets::{Date, RecurrenceEditor, RecurrenceRule, Ui};
///
/// let rule = State::new(RecurrenceRule::default());
/// let start = Date::new(2026, 11, 5).unwrap();
/// let mut ui = Ui::new(RecurrenceEditor::new(rule.binding(), start), ravenrs_style::Theme::raven_light());
/// ui.update();
/// ```
#[derive(Debug)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct RecurrenceEditor {
    rule: Binding<RecurrenceRule>,
    start: Date,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl RecurrenceEditor {
    /// An editor of `rule`, for something first happening on `start`.
    pub fn new(rule: Binding<RecurrenceRule>, start: Date) -> Self {
        Self { rule, start }
    }
}

/// A binding to one part of the rule.
fn part<T: 'static>(
    rule: &Binding<RecurrenceRule>,
    read: impl Fn(&RecurrenceRule) -> T + 'static,
    write: impl Fn(&mut RecurrenceRule, T) + 'static,
) -> Binding<T> {
    let (from, to) = (rule.clone(), rule.clone());
    Binding::new(move || read(&from.get()), move |value| to.update(|r| write(r, value)))
}

impl View for RecurrenceEditor {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let labels = cx.labels();
        let rule = self.rule.get();
        let repeats = rule.frequency != Frequency::Once;
        let frequency = part(
            &self.rule,
            |r| Frequency::ALL.iter().position(|f| *f == r.frequency).unwrap_or(0),
            |r, i: usize| r.frequency = Frequency::ALL[i.min(Frequency::ALL.len() - 1)],
        );
        let mut rows: Vec<crate::AnyView> = vec![Box::new(
            SegmentedControl::new(frequency, labels.recurrence_frequencies.clone())
                .label(labels.recurrence_name.clone()),
        )];
        if repeats {
            let interval = part(
                &self.rule,
                |r| i32::try_from(r.interval.max(1)).unwrap_or(1),
                |r, n: i32| r.interval = u32::try_from(n.max(1)).unwrap_or(1),
            );
            rows.push(Box::new(
                HStack::new((
                    Text::new(labels.recurrence_every.clone()).style(TextStyle::Callout),
                    Stepper::new(interval).range(1, 99).label(labels.recurrence_every.clone()),
                    Text::new(labels.recurrence_unit(rule.frequency, rule.interval).to_owned())
                        .style(TextStyle::Callout),
                ))
                .spacing(8.0)
                .align(Align::Center),
            ));
        }
        if rule.frequency == Frequency::Weekly {
            let own = usize::from(self.start.weekday());
            let days: Vec<crate::AnyView> = (0..7)
                .map(|day| {
                    let chosen = part(
                        &self.rule,
                        // With none chosen the rule keeps the first day's weekday, so that one shows on.
                        move |r| if r.weekdays.contains(&true) { r.weekdays[day] } else { day == own },
                        move |r, on: bool| {
                            if !r.weekdays.contains(&true) {
                                r.weekdays[own] = true;
                            }
                            r.weekdays[day] = on;
                        },
                    );
                    Box::new(ToggleButton::new(chosen, labels.weekdays[day].clone())) as crate::AnyView
                })
                .collect();
            rows.push(Box::new(HStack::new(days).spacing(4.0)));
        }
        if repeats {
            let start = self.start;
            let kind = part(
                &self.rule,
                |r| match r.end {
                    RecurrenceEnd::Until(_) => 1,
                    RecurrenceEnd::After(_) => 2,
                    _ => 0,
                },
                move |r, i: usize| {
                    r.end = match i {
                        1 => RecurrenceEnd::Until(start.add_months(12)),
                        2 => RecurrenceEnd::After(12),
                        _ => RecurrenceEnd::Never,
                    };
                },
            );
            let ends = SegmentedControl::new(kind, labels.recurrence_ends.clone())
                .label(labels.recurrence_ends_name.clone());
            let extra: Option<crate::AnyView> = match rule.end {
                RecurrenceEnd::Until(day) => {
                    let last = part(
                        &self.rule,
                        |r| if let RecurrenceEnd::Until(day) = r.end { Some(day) } else { None },
                        |r, day: Option<Date>| {
                            if let Some(day) = day {
                                r.end = RecurrenceEnd::Until(day);
                            }
                        },
                    );
                    Some(Box::new(
                        DatePicker::new(last, day).label(labels.recurrence_ends_name.clone()).width(190.0),
                    ))
                }
                RecurrenceEnd::After(_) => {
                    let count = part(
                        &self.rule,
                        |r| {
                            if let RecurrenceEnd::After(n) = r.end {
                                i32::try_from(n).unwrap_or(1)
                            } else {
                                1
                            }
                        },
                        |r, n: i32| r.end = RecurrenceEnd::After(u32::try_from(n.max(1)).unwrap_or(1)),
                    );
                    Some(Box::new(
                        HStack::new((
                            Stepper::new(count).range(1, 999).label(labels.recurrence_times.clone()),
                            Text::new(labels.recurrence_times.clone()).style(TextStyle::Callout),
                        ))
                        .spacing(8.0)
                        .align(Align::Center),
                    ))
                }
                _ => None,
            };
            rows.push(Box::new(FormField::new(
                labels.recurrence_ends_name.clone(),
                VStack::new((ends, extra)).spacing(8.0),
            )));
        }
        let summary = labels.recurrence_summary(&rule, self.start);
        rows.push(Box::new(Text::new(summary).style(TextStyle::Callout).foreground(Semantic::InkMuted)));
        cx.child(&VStack::new(rows).spacing(10.0).align(Align::Stretch))
    }
}
