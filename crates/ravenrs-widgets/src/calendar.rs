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

#[derive(Clone)]
struct GridKeys {
    month: State<Date>,
    cursor: State<Option<Date>>,
    focus_to: Rc<Cell<Option<Date>>>,
}

impl GridKeys {
    /// Moves the cursor (and the shown month, if needed) for a navigation key.
    fn handler(&self, from: Date) -> crate::context::KeyHandler {
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
        let selected = c.selected.get() == Some(self.date);
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
        let name = format!("{} {}", self.date.day(), self.date.month_title());
        let mut semantics = Semantics::new(Role::Button).name(name).selected(selected).disabled(!enabled);
        if c.today == Some(self.date) {
            semantics = semantics.description("Today");
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
        let (prev, next) = (month.clone(), month.clone());
        let back = cx.child(
            &IconButton::new(IconName::ChevronLeft)
                .label("Previous month")
                .size(ButtonSize::Small)
                .on_click(move || prev.update(|m| *m = m.add_months(-1))),
        );
        let title = util::label(
            cx,
            &shown.month_title(),
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
                .label("Next month")
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
        let mut cells: Vec<NodeId> = ["Su", "Mo", "Tu", "We", "Th", "Fr", "Sa"]
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
            month: month.clone(),
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
                align_self: Some(Align::Start),
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
    placeholder: String,
    today: Option<Date>,
    width: f32,
    label: Option<String>,
}

impl DatePicker {
    /// `month` is shown when no date is selected yet.
    pub fn new(value: Binding<Option<Date>>, month: Date) -> Self {
        Self { value, month, placeholder: "Pick a date".into(), today: None, width: 220.0, label: None }
    }

    /// Text shown while no date is selected ("Pick a date" by default).
    pub fn placeholder(mut self, text: impl Into<String>) -> Self {
        self.placeholder = text.into();
        self
    }

    /// Marks this day with a ring in the calendar.
    pub fn today(mut self, today: Date) -> Self {
        self.today = Some(today);
        self
    }

    /// Field width in logical pixels (default 220).
    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
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
        let is_open = open.get();
        let hovered = cx.is_hovered();
        let icon = util::icon(cx, IconName::Calendar, 16.0, theme.color(Semantic::InkMuted));
        let (text, color) = match self.value.get() {
            Some(d) => (d.short(), theme.color(Semantic::Ink)),
            None => (self.placeholder.clone(), theme.color(Semantic::InkMuted)),
        };
        let label = util::label(cx, &text, TextStyle::Body, None, color);
        let border = if is_open {
            Border { width: 1.5, color: theme.color(Semantic::Accent) }
        } else {
            let base = theme.color(Semantic::Border);
            Border {
                width: 1.0,
                color: if hovered { base.lerp(theme.color(Semantic::Ink), 0.25) } else { base },
            }
        };
        let node = cx.container(
            LayoutStyle {
                width: Some(self.width),
                height: Some(40.0),
                padding: Insets::symmetric(0.0, 12.0),
                spacing: 10.0,
                align: Align::Center,
                align_self: Some(Align::Start),
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &[icon, label],
            Paint::Quad(QuadPaint {
                corner_radius: theme.radii.md,
                border: Some(border),
                ..QuadPaint::new(theme.color(Semantic::Control))
            }),
        );
        cx.semantics(label, Semantics::hidden());
        let value = self.value.get().map(|d| format!("{} {}", d.day(), d.month_title())).unwrap_or_default();
        let mut semantics = Semantics::new(Role::ComboBox).value(value).expanded(is_open);
        if let Some(label) = &self.label {
            semantics = semantics.name(label.clone());
        }
        cx.semantics(node, semantics);
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
