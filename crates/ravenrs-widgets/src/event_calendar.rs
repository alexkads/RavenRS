//! A month with what happens on each day, and the editor of how something repeats.

use std::cell::Cell;
use std::rc::Rc;

use ravenrs_core::{Border, Color, CursorIcon, Insets, Role};
use ravenrs_layout::{Align, Axis, Justify, LayoutStyle};
use ravenrs_reactive::Binding;
use ravenrs_style::{Semantic, TextStyle, Weight};

use crate::calendar::GridKeys;
use crate::context::{BuildCx, NodeId, Paint, QuadPaint, Semantics};
use crate::date::Date;
use crate::display::Tone;
use crate::recurrence::{Frequency, RecurrenceEnd, RecurrenceRule};
use crate::{
    Button, ButtonSize, ButtonStyle, DatePicker, FormField, HStack, IconButton, IconName, SegmentedControl,
    Stepper, Text, ToggleButton, VStack, View, util,
};

/// Something on a [`EventCalendar`]'s day.
#[derive(Clone, Debug, PartialEq, Eq)]
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
}

impl CalendarEvent {
    /// An event on this day.
    pub fn new(id: u64, date: Date, title: impl Into<String>) -> Self {
        Self { id, date, title: title.into(), tone: Tone::Accent, done: false }
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

/// A month with its events inside each day, for agendas and bookings. Use a
/// [`Calendar`](crate::Calendar) to pick a date and this to see what is on them.
///
/// From the keyboard the grid is one Tab stop: arrows move by day and week, Home
/// and End to the ends of the week, Page Up and Page Down by month, and Enter or
/// Space picks. A day's name reads out what is on it.
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
pub struct EventCalendar {
    month: Binding<Date>,
    selected: Binding<Option<Date>>,
    events: Vec<CalendarEvent>,
    today: Option<Date>,
    per_day: usize,
    on_event: Option<Rc<dyn Fn(u64)>>,
}

impl std::fmt::Debug for EventCalendar {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EventCalendar").field("events", &self.events.len()).finish_non_exhaustive()
    }
}

/// The height of a day with room for its events, in logical pixels.
const DAY_HEIGHT: f32 = 104.0;

impl EventCalendar {
    /// `month` holds the month shown (any day of it), so the app can load its
    /// events; `selected` holds the day picked.
    pub fn new(month: Binding<Date>, selected: Binding<Option<Date>>) -> Self {
        Self { month, selected, events: Vec::new(), today: None, per_day: 3, on_event: None }
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

impl View for EventCalendar {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let labels = cx.labels();
        let shown = self.month.get().first_of_month();
        let (prev, next) = (self.month.clone(), self.month.clone());
        let mut header = vec![
            cx.child(
                &IconButton::new(IconName::ChevronLeft)
                    .label(labels.previous_month.clone())
                    .size(ButtonSize::Small)
                    .on_click(move || prev.set(shown.add_months(-1))),
            ),
            cx.child(
                &IconButton::new(IconName::ChevronRight)
                    .label(labels.next_month.clone())
                    .size(ButtonSize::Small)
                    .on_click(move || next.set(shown.add_months(1))),
            ),
            util::label(
                cx,
                &labels.month_title(shown),
                TextStyle::Headline,
                None,
                theme.color(Semantic::Ink),
            ),
        ];
        if let Some(today) = self.today {
            let (month, selected) = (self.month.clone(), self.selected.clone());
            header.push(
                cx.child(
                    &Button::new(labels.today.clone())
                        .style(ButtonStyle::Ghost)
                        .size(ButtonSize::Small)
                        .on_click(move || {
                            month.set(today.first_of_month());
                            selected.set(Some(today));
                        }),
                ),
            );
        }
        let header = cx.container(
            LayoutStyle { spacing: 6.0, align: Align::Center, ..LayoutStyle::stack(Axis::Horizontal) },
            &header,
            Paint::None,
        );
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
        let start = shown.add_days(-i32::from(shown.weekday()));
        let last = shown.add_months(1).add_days(-1);
        let days = i32::try_from(last.to_days() - start.to_days()).unwrap_or(34) / 7 * 7 + 7;
        for i in 0..days {
            let date = start.add_days(i);
            let events = self.events.iter().filter(|e| e.date == date).collect();
            let day = CalendarDay {
                date,
                in_month: in_shown(&date),
                active: date == active,
                events,
                scheduler: self,
                keys: &keys,
            };
            cells.push(cx.keyed_child(date.to_days() as u64, &day));
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
        cx.container(
            LayoutStyle { spacing: 10.0, align: Align::Stretch, ..LayoutStyle::stack(Axis::Vertical) },
            &[header, grid],
            Paint::None,
        )
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
pub struct RecurrenceEditor {
    rule: Binding<RecurrenceRule>,
    start: Date,
}

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
