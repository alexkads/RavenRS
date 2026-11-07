//! Calendar and date picker.

use std::cell::Cell;
use std::rc::Rc;

use ravenrs_core::{Border, Color, CursorIcon, Insets, Key, Role};
use ravenrs_layout::{Align, Axis, Justify, LayoutStyle};
use ravenrs_reactive::{Binding, State};
use ravenrs_style::{Semantic, TextStyle, Weight};

use crate::context::{BuildCx, NodeId, Paint, Placement, QuadPaint, Semantics};
use crate::date::Date;
use crate::{ButtonSize, IconButton, IconName, View, util};

/// Two days and the days between them, both included.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DateRange {
    /// The first day.
    pub start: Date,
    /// The last day.
    pub end: Date,
}

impl DateRange {
    /// The days between these two, in whichever order they come.
    pub fn new(a: Date, b: Date) -> Self {
        if a <= b { Self { start: a, end: b } } else { Self { start: b, end: a } }
    }

    /// Whether this day is in the range.
    pub fn contains(self, date: Date) -> bool {
        self.start <= date && date <= self.end
    }

    /// How many days it covers.
    pub fn days(self) -> i32 {
        (self.end.to_days() - self.start.to_days()) as i32 + 1
    }
}

/// A month grid to pick a date. Use ‹ › to change month. From the keyboard the
/// grid is one Tab stop: arrows move by day and week, Home and End to the ends
/// of the week, Page Up and Page Down by month, and Enter or Space picks.
///
/// ```
/// use ravenrs_reactive::State;
/// use ravenrs_widgets::{Calendar, Date, Ui};
///
/// let today = Date::new(2026, 10, 31).unwrap();
/// let picked = State::new(None::<Date>);
/// let calendar = Calendar::new(picked.binding(), today).today(today);
/// let mut ui = Ui::new(calendar, ravenrs_style::Theme::raven_dark());
/// ui.update();
/// ```
pub struct Calendar {
    selected: Binding<Option<Date>>,
    initial_month: Date,
    today: Option<Date>,
    min: Option<Date>,
    max: Option<Date>,
    on_pick: Option<Rc<dyn Fn(Date)>>,
    span: Option<DateRange>,
}

impl std::fmt::Debug for Calendar {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Calendar").field("month", &self.initial_month).finish_non_exhaustive()
    }
}

impl Calendar {
    /// `month` is the month shown first (any day of it).
    pub fn new(selected: Binding<Option<Date>>, month: Date) -> Self {
        Self {
            selected,
            initial_month: month.first_of_month(),
            today: None,
            min: None,
            max: None,
            on_pick: None,
            span: None,
        }
    }

    /// Marks this day with a ring.
    pub fn today(mut self, today: Date) -> Self {
        self.today = Some(today);
        self
    }

    /// Days outside `min..=max` cannot be picked.
    pub fn range(mut self, min: Option<Date>, max: Option<Date>) -> Self {
        self.min = min;
        self.max = max;
        self
    }

    pub(crate) fn on_pick(mut self, f: Rc<dyn Fn(Date)>) -> Self {
        self.on_pick = Some(f);
        self
    }

    /// Shades these days, with both ends as if selected.
    fn span(mut self, span: Option<DateRange>) -> Self {
        self.span = span;
        self
    }
}

struct Day<'a> {
    date: Date,
    in_month: bool,
    calendar: &'a Calendar,
    /// The grid's single Tab stop.
    active: bool,
    /// Shared with the other days: the shown month, the keyboard cursor and
    /// the day that takes focus on its next build.
    keys: &'a GridKeys,
}

/// The keyboard over a month grid; an [`EventCalendar`](crate::EventCalendar) shares it.
#[derive(Clone)]
pub(crate) struct GridKeys {
    pub month: Binding<Date>,
    pub cursor: State<Option<Date>>,
    pub focus_to: Rc<Cell<Option<Date>>>,
}

impl GridKeys {
    /// Moves the cursor (and the shown month, if needed) for a navigation key.
    pub fn handler(&self, from: Date) -> crate::context::KeyHandler {
        let keys = self.clone();
        Rc::new(move |key, _| {
            let to = match key {
                Key::ArrowLeft => from.add_days(-1),
                Key::ArrowRight => from.add_days(1),
                Key::ArrowUp => from.add_days(-7),
                Key::ArrowDown => from.add_days(7),
                Key::Home => from.add_days(-i32::from(from.weekday())),
                Key::End => from.add_days(6 - i32::from(from.weekday())),
                Key::PageUp => from.add_months(-1),
                Key::PageDown => from.add_months(1),
                _ => return false,
            };
            if to.first_of_month() != keys.month.get() {
                keys.month.set(to.first_of_month());
            }
            keys.cursor.set(Some(to));
            keys.focus_to.set(Some(to));
            true
        })
    }
}

impl View for Day<'_> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let c = self.calendar;
        let is_end = c.span.is_some_and(|s| s.start == self.date || s.end == self.date);
        let inside = c.span.is_some_and(|s| s.contains(self.date));
        let selected = c.selected.get() == Some(self.date) || is_end;
        let enabled = c.min.is_none_or(|m| self.date >= m) && c.max.is_none_or(|m| self.date <= m);
        let hovered = (cx.is_hovered() && enabled) || cx.is_focus_visible();
        let ink = theme.color(Semantic::Ink);
        let color = if selected {
            theme.color(Semantic::OnAccent)
        } else if !enabled {
            theme.color(Semantic::InkMuted).with_alpha(0.4)
        } else if self.in_month {
            ink
        } else {
            theme.color(Semantic::InkMuted)
        };
        let weight = (selected || c.today == Some(self.date)).then_some(Weight::SEMIBOLD);
        let label = util::label(cx, &self.date.day().to_string(), TextStyle::Callout, weight, color);
        let fill = if selected {
            theme.color(Semantic::Accent)
        } else if hovered {
            ink.with_alpha(0.08)
        } else if inside {
            theme.color(Semantic::Accent).with_alpha(0.14)
        } else {
            Color::TRANSPARENT
        };
        let border = (c.today == Some(self.date) && !selected)
            .then(|| Border { width: 1.0, color: theme.color(Semantic::Accent) });
        let node = cx.container(
            LayoutStyle {
                width: Some(36.0),
                height: Some(36.0),
                align: Align::Center,
                justify: Justify::Center,
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &[label],
            Paint::Quad(QuadPaint { corner_radius: 9.0, border, ..QuadPaint::new(fill) }),
        );
        // The full date, so the day reads on its own ("31 October 2026").
        let labels = cx.labels();
        let name = labels.long_date(self.date);
        let mut semantics =
            Semantics::new(Role::Button).name(name).selected(selected || inside).disabled(!enabled);
        if c.today == Some(self.date) {
            semantics = semantics.description(labels.today.clone());
        }
        cx.semantics(node, semantics);
        if self.keys.focus_to.get() == Some(self.date) {
            self.keys.focus_to.set(None);
            cx.request_focus();
        }
        if self.active {
            cx.focusable(node, 9.0);
            cx.on_key(node, self.keys.handler(self.date));
        }
        if enabled {
            let (sel, date, pick) = (c.selected.clone(), self.date, c.on_pick.clone());
            cx.on_click(
                node,
                Rc::new(move || {
                    sel.set(Some(date));
                    if let Some(p) = &pick {
                        p(date);
                    }
                }),
            );
            cx.cursor(node, CursorIcon::Pointer);
        }
        node
    }
}

impl View for Calendar {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let initial = self.selected.get().map_or(self.initial_month, Date::first_of_month);
        let month: State<Date> = cx.use_state(move || initial);
        let shown = month.get();
        let labels = cx.labels();
        let (prev, next) = (month.clone(), month.clone());
        let back = cx.child(
            &IconButton::new(IconName::ChevronLeft)
                .label(labels.previous_month.clone())
                .size(ButtonSize::Small)
                .on_click(move || prev.update(|m| *m = m.add_months(-1))),
        );
        let title = util::label(
            cx,
            &labels.month_title(shown),
            TextStyle::Body,
            Some(Weight::SEMIBOLD),
            theme.color(Semantic::Ink),
        );
        let title = cx.container(
            LayoutStyle { grow: 1.0, justify: Justify::Center, ..LayoutStyle::stack(Axis::Horizontal) },
            &[title],
            Paint::None,
        );
        let forward = cx.child(
            &IconButton::new(IconName::ChevronRight)
                .label(labels.next_month.clone())
                .size(ButtonSize::Small)
                .on_click(move || next.update(|m| *m = m.add_months(1))),
        );
        let header = cx.container(
            LayoutStyle {
                align: Align::Center,
                align_self: Some(Align::Stretch),
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &[back, title, forward],
            Paint::None,
        );
        let mut cells: Vec<NodeId> = labels
            .weekdays
            .iter()
            .map(|d| {
                let l = util::label(cx, d, TextStyle::Caption, None, theme.color(Semantic::InkMuted));
                // Each day's name already says the date.
                cx.semantics(l, Semantics::hidden());
                cx.container(
                    LayoutStyle {
                        width: Some(36.0),
                        height: Some(28.0),
                        align: Align::Center,
                        justify: Justify::Center,
                        ..LayoutStyle::stack(Axis::Horizontal)
                    },
                    &[l],
                    Paint::None,
                )
            })
            .collect();
        let keys = GridKeys {
            month: month.binding(),
            cursor: cx.use_state(|| None),
            focus_to: cx.use_state(|| Rc::new(Cell::new(None))).get(),
        };
        // The Tab stop: the keyboard cursor, else the selection, else today, in
        // the shown month; otherwise its first day.
        let in_shown = |d: &Date| d.first_of_month() == shown;
        let active = [keys.cursor.get(), self.selected.get(), self.today]
            .into_iter()
            .flatten()
            .find(in_shown)
            .unwrap_or(shown);
        let start = shown.add_days(-i32::from(shown.weekday()));
        for i in 0..42 {
            let date = start.add_days(i);
            let in_month = date.month() == shown.month();
            let day = Day { date, in_month, calendar: self, active: date == active, keys: &keys };
            cells.push(cx.keyed_child(date.to_days() as u64, &day));
        }
        let grid = cx.container(
            LayoutStyle { grid_columns: Some(7), spacing: 2.0, ..LayoutStyle::stack(Axis::Vertical) },
            &cells,
            Paint::None,
        );
        cx.semantics(grid, Semantics::new(Role::Calendar).name(shown.month_title()));
        cx.container(
            LayoutStyle {
                spacing: 10.0,
                width: Some(7.0 * 36.0 + 12.0),
                align_self: Some(Align::Fit),
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &[header, grid],
            Paint::None,
        )
    }
}

/// A field that opens a calendar in a popover.
#[derive(Debug)]
pub struct DatePicker {
    value: Binding<Option<Date>>,
    month: Date,
    placeholder: Option<String>,
    today: Option<Date>,
    width: f32,
    fill: bool,
    label: Option<String>,
    state: util::EditorState,
}

impl DatePicker {
    /// `month` is shown when no date is selected yet.
    pub fn new(value: Binding<Option<Date>>, month: Date) -> Self {
        Self {
            value,
            month,
            placeholder: None,
            today: None,
            width: 220.0,
            fill: false,
            label: None,
            state: util::EditorState::default(),
        }
    }

    /// Text shown while no date is selected ("Pick a date" by default).
    pub fn placeholder(mut self, text: impl Into<String>) -> Self {
        self.placeholder = Some(text.into());
        self
    }

    /// Marks this day with a ring in the calendar.
    pub fn today(mut self, today: Date) -> Self {
        self.today = Some(today);
        self
    }

    /// Takes the width of its parent, as other fields of a form do.
    pub fn fill(mut self) -> Self {
        self.fill = true;
        self
    }

    /// Field width in logical pixels (default 220).
    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    /// Turns the control off: it fades, leaves the tab order and ignores input.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.state.disabled = disabled;
        self
    }

    /// Shows the value without letting it change; it can still be focused and read.
    pub fn read_only(mut self, read_only: bool) -> Self {
        self.state.read_only = read_only;
        self
    }
}

struct CalendarPanel<'a>(&'a DatePicker, State<bool>);

impl View for CalendarPanel<'_> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let close = self.1.clone();
        let mut cal =
            Calendar::new(self.0.value.clone(), self.0.month).on_pick(Rc::new(move |_| close.set(false)));
        if let Some(t) = self.0.today {
            cal = cal.today(t);
        }
        let child = cx.child(&cal);
        let node = cx.container(
            LayoutStyle { padding: Insets::uniform(12.0), ..LayoutStyle::stack(Axis::Vertical) },
            &[child],
            Paint::Quad(QuadPaint {
                corner_radius: theme.radii.lg,
                border: Some(Border { width: 1.0, color: theme.color(Semantic::Border) }),
                shadow: Some(theme.elevation.raised),
                ..QuadPaint::new(theme.color(Semantic::SurfaceElevated))
            }),
        );
        cx.cursor(node, CursorIcon::Default);
        node
    }
}

impl DatePicker {
    /// The name screen readers announce, such as "Due date". Inside a
    /// [`crate::FormField`], its label is used instead.
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }
}

impl View for DatePicker {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let open: State<bool> = cx.use_state(|| false);
        let state = self.state;
        let is_open = open.get() && state.accepts_input();
        let hovered = cx.is_hovered() && state.accepts_input();
        let icon = util::icon(cx, IconName::Calendar, 16.0, state.ink(theme.color(Semantic::InkMuted)));
        let labels = cx.labels();
        let (text, color) = match self.value.get() {
            Some(d) => (labels.short_date(d), theme.color(Semantic::Ink)),
            None => {
                let hint = self.placeholder.clone().unwrap_or_else(|| labels.pick_date.clone());
                (hint, theme.color(Semantic::InkMuted))
            }
        };
        let label = util::label(cx, &text, TextStyle::Body, None, state.ink(color));
        let border = if is_open {
            Border { width: 1.5, color: theme.color(Semantic::Accent) }
        } else {
            let base = theme.color(Semantic::Border);
            Border {
                width: 1.0,
                color: state.ink(if hovered { base.lerp(theme.color(Semantic::Ink), 0.25) } else { base }),
            }
        };
        let node = cx.container(
            LayoutStyle {
                width: (!self.fill).then_some(self.width),
                height: Some(40.0),
                padding: Insets::symmetric(0.0, 12.0),
                spacing: 10.0,
                align: Align::Center,
                align_self: Some(if self.fill { Align::Stretch } else { Align::Fit }),
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &[icon, label],
            Paint::Quad(QuadPaint {
                corner_radius: theme.radii.md,
                border: Some(border),
                ..QuadPaint::new(state.fill(theme.color(Semantic::Control)))
            }),
        );
        cx.semantics(label, Semantics::hidden());
        let value = self.value.get().map(|d| labels.long_date(d)).unwrap_or_default();
        let mut semantics = Semantics::new(Role::ComboBox).value(value).expanded(is_open);
        if let Some(label) = &self.label {
            semantics = semantics.name(label.clone());
        }
        cx.semantics(node, state.semantics(semantics));
        if state.disabled {
            return node;
        }
        if !state.accepts_input() {
            cx.focusable(node, theme.radii.md);
            return node;
        }
        let toggle = open.clone();
        cx.on_click(node, Rc::new(move || toggle.update(|o| *o = !*o)));
        cx.focusable(node, theme.radii.md);
        cx.cursor(node, CursorIcon::Pointer);
        if is_open {
            let close = open.clone();
            cx.overlay(
                node,
                Placement::Below,
                &CalendarPanel(self, open.clone()),
                Some(Rc::new(move || close.set(false))),
            );
        }
        node
    }
}

/// A field that opens two months to pick where a period starts and ends, with
/// ready-made periods on the side. The first click starts the period, the second ends it.
///
/// ```
/// use ravenrs_reactive::State;
/// use ravenrs_widgets::{Date, DateRange, DateRangePicker, Ui};
///
/// let today = Date::new(2026, 11, 2).unwrap();
/// let period = State::new(None::<DateRange>);
/// let picker = DateRangePicker::new(period.binding(), today)
///     .preset("Last 7 days", DateRange::new(today.add_days(-6), today));
/// let mut ui = Ui::new(picker, ravenrs_style::Theme::raven_dark());
/// ui.update();
/// ```
pub struct DateRangePicker {
    value: Binding<Option<DateRange>>,
    month: Date,
    today: Option<Date>,
    min: Option<Date>,
    max: Option<Date>,
    presets: Vec<(String, DateRange)>,
    placeholder: Option<String>,
    width: f32,
    label: Option<String>,
    state: util::EditorState,
}

impl std::fmt::Debug for DateRangePicker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DateRangePicker").field("month", &self.month).finish_non_exhaustive()
    }
}

impl DateRangePicker {
    /// `month` is shown when no period is chosen yet.
    pub fn new(value: Binding<Option<DateRange>>, month: Date) -> Self {
        Self {
            value,
            month,
            today: None,
            min: None,
            max: None,
            presets: Vec::new(),
            placeholder: None,
            width: 340.0,
            label: None,
            state: util::EditorState::default(),
        }
    }

    /// A ready-made period, picked with one click.
    pub fn preset(mut self, name: impl Into<String>, range: DateRange) -> Self {
        self.presets.push((name.into(), range));
        self
    }

    /// Marks this day with a ring in the calendars.
    pub fn today(mut self, today: Date) -> Self {
        self.today = Some(today);
        self
    }

    /// Days outside `min..=max` cannot be picked.
    pub fn range(mut self, min: Option<Date>, max: Option<Date>) -> Self {
        self.min = min;
        self.max = max;
        self
    }

    /// Text shown while no period is chosen.
    pub fn placeholder(mut self, text: impl Into<String>) -> Self {
        self.placeholder = Some(text.into());
        self
    }

    /// Field width in logical pixels (default 340).
    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    /// The name screen readers announce. Inside a [`crate::FormField`], its label is used instead.
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Turns the control off: it fades, leaves the tab order and ignores input.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.state.disabled = disabled;
        self
    }

    /// Shows the value without letting it change; it can still be focused and read.
    pub fn read_only(mut self, read_only: bool) -> Self {
        self.state.read_only = read_only;
        self
    }
}

struct RangePanel<'a>(&'a DateRangePicker, State<bool>);

impl View for RangePanel<'_> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let picker = self.0;
        let started: State<Option<Date>> = cx.use_state(|| None);
        let pending = started.get();
        let chosen = picker.value.get();
        let span = match pending {
            Some(day) => Some(DateRange::new(day, day)),
            None => chosen,
        };
        let first = chosen.map_or(picker.month, |r| r.start).first_of_month();
        let pick: Rc<dyn Fn(Date)> = {
            let (started, value, close) = (started.clone(), picker.value.clone(), self.1.clone());
            Rc::new(move |day| match started.get() {
                None => started.set(Some(day)),
                Some(from) => {
                    started.set(None);
                    value.set(Some(DateRange::new(from, day)));
                    close.set(false);
                }
            })
        };
        // A calendar's selection is the period's, which its own binding must not change.
        let inert = || Binding::new(|| None, |_: Option<Date>| {});
        let calendar = |month: Date| {
            let mut calendar =
                Calendar::new(inert(), month).range(picker.min, picker.max).on_pick(pick.clone()).span(span);
            if let Some(today) = picker.today {
                calendar = calendar.today(today);
            }
            calendar
        };
        let mut parts = Vec::new();
        if !picker.presets.is_empty() {
            let mut rows = Vec::with_capacity(picker.presets.len());
            for (i, (name, range)) in picker.presets.iter().enumerate() {
                let (value, close, range) = (picker.value.clone(), self.1.clone(), *range);
                let button = crate::Button::new(name.clone())
                    .style(crate::ButtonStyle::Ghost)
                    .size(ButtonSize::Small)
                    .on_click(move || {
                        value.set(Some(range));
                        close.set(false);
                    });
                rows.push(cx.keyed_child(i as u64, &button));
            }
            parts.push(cx.container(
                LayoutStyle { spacing: 2.0, align: Align::Stretch, ..LayoutStyle::stack(Axis::Vertical) },
                &rows,
                Paint::None,
            ));
        }
        parts.push(cx.keyed_child(1 << 32, &calendar(first)));
        // The second month needs room, which a phone does not have.
        if cx.viewport().width >= 720.0 {
            parts.push(cx.keyed_child(1 << 33, &calendar(first.add_months(1))));
        }
        let node = cx.container(
            LayoutStyle {
                padding: Insets::uniform(12.0),
                spacing: 16.0,
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &parts,
            Paint::Quad(QuadPaint {
                corner_radius: theme.radii.lg,
                border: Some(Border { width: 1.0, color: theme.color(Semantic::Border) }),
                shadow: Some(theme.elevation.raised),
                ..QuadPaint::new(theme.color(Semantic::SurfaceElevated))
            }),
        );
        cx.cursor(node, CursorIcon::Default);
        node
    }
}

impl View for DateRangePicker {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let open: State<bool> = cx.use_state(|| false);
        let state = self.state;
        let is_open = open.get() && state.accepts_input();
        let hovered = cx.is_hovered() && state.accepts_input();
        let icon = util::icon(cx, IconName::Calendar, 16.0, state.ink(theme.color(Semantic::InkMuted)));
        let labels = cx.labels();
        let chosen = self.value.get();
        let (text, color) = match chosen {
            Some(r) => (
                format!("{} – {}", labels.short_date(r.start), labels.short_date(r.end)),
                theme.color(Semantic::Ink),
            ),
            None => (
                self.placeholder.clone().unwrap_or_else(|| labels.pick_date_range.clone()),
                theme.color(Semantic::InkMuted),
            ),
        };
        let label = cx.text_truncated(
            &text,
            &util::attrs(&theme, TextStyle::Body, None),
            state.ink(color),
            LayoutStyle { grow: 1.0, min_width: Some(0.0), ..LayoutStyle::leaf() },
        );
        cx.semantics(label, Semantics::hidden());
        let mut children = vec![icon, label];
        if chosen.is_some() && state.accepts_input() {
            let value = self.value.clone();
            children.push(
                cx.child(
                    &IconButton::new(IconName::X)
                        .label(labels.clear.clone())
                        .size(ButtonSize::Small)
                        .on_click(move || value.set(None)),
                ),
            );
        }
        let border = if is_open {
            Border { width: 1.5, color: theme.color(Semantic::Accent) }
        } else {
            let base = theme.color(Semantic::Border);
            Border {
                width: 1.0,
                color: state.ink(if hovered { base.lerp(theme.color(Semantic::Ink), 0.25) } else { base }),
            }
        };
        let node = cx.container(
            LayoutStyle {
                width: Some(self.width),
                height: Some(40.0),
                padding: Insets::symmetric(0.0, 12.0),
                spacing: 10.0,
                align: Align::Center,
                align_self: Some(Align::Fit),
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &children,
            Paint::Quad(QuadPaint {
                corner_radius: theme.radii.md,
                border: Some(border),
                ..QuadPaint::new(state.fill(theme.color(Semantic::Control)))
            }),
        );
        let mut semantics = Semantics::new(Role::ComboBox).value(text).expanded(is_open);
        if let Some(label) = &self.label {
            semantics = semantics.name(label.clone());
        }
        cx.semantics(node, state.semantics(semantics));
        if state.disabled {
            return node;
        }
        cx.focusable(node, theme.radii.md);
        if !state.accepts_input() {
            return node;
        }
        let toggle = open.clone();
        cx.on_click(node, Rc::new(move || toggle.update(|o| *o = !*o)));
        let opener = open.clone();
        cx.on_key(
            node,
            Rc::new(move |key, _| {
                let opens = matches!(key, Key::ArrowDown | Key::ArrowUp);
                if opens {
                    opener.set(true);
                }
                opens
            }),
        );
        cx.cursor(node, CursorIcon::Pointer);
        let was_open = cx.use_state(|| Rc::new(Cell::new(false))).get();
        if is_open {
            was_open.set(true);
            let close = open.clone();
            cx.overlay(
                node,
                Placement::Below,
                &RangePanel(self, open.clone()),
                Some(Rc::new(move || close.set(false))),
            );
        } else if was_open.replace(false) {
            cx.request_focus();
        }
        node
    }
}
