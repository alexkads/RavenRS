//! A whole year at a glance.

use std::cell::Cell;
use std::fmt;
use std::rc::Rc;

use ravenrs_core::{Border, Color, CursorIcon, Role};
use ravenrs_layout::{Align, Axis, Justify, LayoutStyle};
use ravenrs_reactive::{Binding, State};
use ravenrs_style::{Semantic, TextStyle, Weight};

use crate::calendar::GridKeys;
use crate::context::{BuildCx, NodeId, Paint, QuadPaint, Semantics};
use crate::date::{Date, days_in_month};
use crate::display::Tone;
use crate::{ButtonSize, IconButton, IconName, View, util};

/// The side of a day.
const DAY: f32 = 26.0;
/// The gap between days.
const DAY_GAP: f32 = 2.0;
/// The gap between months.
const MONTH_GAP: f32 = 20.0;
/// How many months go on a line: only those that divide a year into whole lines.
const LINES: [usize; 5] = [6, 4, 3, 2, 1];

/// What marks a day of a [`YearView`].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct DayMark {
    /// The color of its dot.
    pub tone: Tone,
    /// What screen readers say of the day, instead of the word in [`Labels`](crate::Labels).
    pub label: Option<String>,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl DayMark {
    /// A mark in the neutral tone.
    pub fn new() -> Self {
        Self::default()
    }

    /// The color of its dot.
    pub fn tone(mut self, tone: Tone) -> Self {
        self.tone = tone;
        self
    }

    /// What screen readers say of the day.
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }
}

/// The twelve months of a year, with a dot on the days that have something and a click on any
/// of them. As many months go on a line as fit. From the keyboard the year is one Tab stop:
/// arrows move by day and week, Home and End to the ends of the week, Page Up and Page Down by
/// month, and Enter or Space picks.
///
/// ```
/// use ravenrs_reactive::State;
/// use ravenrs_widgets::{Date, DayMark, Ui, YearView};
///
/// let year = State::new(2026);
/// let view = YearView::new(year.binding())
///     .today(Date::new(2026, 11, 5).unwrap())
///     .marks(|day| (day.day() == 1).then(DayMark::new))
///     .on_pick(|day| println!("{day}"));
/// let mut ui = Ui::new(view, ravenrs_style::Theme::raven_dark());
/// ui.update();
/// ```
pub struct YearView {
    year: Binding<i32>,
    selected: Option<Binding<Option<Date>>>,
    today: Option<Date>,
    marks: Option<Rc<dyn Fn(Date) -> Option<DayMark>>>,
    on_pick: Option<Rc<dyn Fn(Date)>>,
    on_month: Option<Rc<dyn Fn(Date)>>,
    columns: Option<usize>,
}

impl fmt::Debug for YearView {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("YearView").field("year", &self.year.get()).finish_non_exhaustive()
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl YearView {
    /// The year in `year`, which its buttons and the keyboard also change.
    pub fn new(year: Binding<i32>) -> Self {
        Self { year, selected: None, today: None, marks: None, on_pick: None, on_month: None, columns: None }
    }

    /// The chosen day, shown filled.
    pub fn selected(mut self, selected: Binding<Option<Date>>) -> Self {
        self.selected = Some(selected);
        self
    }

    /// Marks this day with a ring.
    pub fn today(mut self, today: Date) -> Self {
        self.today = Some(today);
        self
    }

    /// Says which days have something. It is asked for each day on each build.
    pub fn marks(mut self, marks: impl Fn(Date) -> Option<DayMark> + 'static) -> Self {
        self.marks = Some(Rc::new(marks));
        self
    }

    /// Called with a day on a click, or on Enter or Space.
    pub fn on_pick(mut self, handler: impl Fn(Date) + 'static) -> Self {
        self.on_pick = Some(Rc::new(handler));
        self
    }

    /// Makes the name of each month a button, called with its first day.
    pub fn on_month(mut self, handler: impl Fn(Date) + 'static) -> Self {
        self.on_month = Some(Rc::new(handler));
        self
    }

    /// How many months go on a line. As many as fit by default.
    pub fn columns(mut self, columns: usize) -> Self {
        self.columns = Some(columns.clamp(1, 12));
        self
    }
}

struct YearDay<'a> {
    date: Date,
    view: &'a YearView,
    active: bool,
    keys: &'a GridKeys,
}

impl View for YearDay<'_> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let labels = cx.labels();
        let view = self.view;
        let selected = view.selected.as_ref().is_some_and(|s| s.get() == Some(self.date));
        let today = view.today == Some(self.date);
        let mark = view.marks.as_ref().and_then(|marks| marks(self.date));
        let lit = cx.is_hovered() || cx.is_focus_visible();
        let ink = theme.color(Semantic::Ink);
        let color = if selected { theme.color(Semantic::OnAccent) } else { ink };
        let weight = (selected || today || mark.is_some()).then_some(Weight::SEMIBOLD);
        let number = util::label(cx, &self.date.day().to_string(), TextStyle::Caption, weight, color);
        let mut parts = vec![number];
        if let Some(mark) = &mark {
            let dot = if selected { color } else { mark.tone.color(&theme) };
            parts.push(cx.container(
                LayoutStyle {
                    absolute: Some((DAY / 2.0 - 2.0, DAY - 6.0)),
                    width: Some(4.0),
                    height: Some(4.0),
                    ..LayoutStyle::leaf()
                },
                &[],
                Paint::Quad(QuadPaint { corner_radius: 2.0, ..QuadPaint::new(dot) }),
            ));
        }
        let fill = if selected {
            theme.color(Semantic::Accent)
        } else if lit {
            ink.with_alpha(0.08)
        } else {
            Color::TRANSPARENT
        };
        let border =
            (today && !selected).then(|| Border { width: 1.0, color: theme.color(Semantic::Accent) });
        let node = cx.container(
            LayoutStyle {
                width: Some(DAY),
                height: Some(DAY),
                align: Align::Center,
                justify: Justify::Center,
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &parts,
            Paint::Quad(QuadPaint { corner_radius: theme.radii.sm, border, ..QuadPaint::new(fill) }),
        );
        let mut words: Vec<&str> = Vec::new();
        if today {
            words.push(&labels.today);
        }
        if let Some(mark) = &mark {
            words.push(mark.label.as_deref().unwrap_or(&labels.day_marked));
        }
        let mut semantics = Semantics::new(Role::Button).name(labels.long_date(self.date)).selected(selected);
        if !words.is_empty() {
            semantics = semantics.description(words.join(", "));
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
        let (chosen, date, pick) = (view.selected.clone(), self.date, view.on_pick.clone());
        cx.on_click(
            node,
            Rc::new(move || {
                if let Some(chosen) = &chosen {
                    chosen.set(Some(date));
                }
                if let Some(pick) = &pick {
                    pick(date);
                }
            }),
        );
        cx.cursor(node, CursorIcon::Pointer);
        node
    }
}

/// The name of a month that goes to it.
struct MonthTitle<'a> {
    text: &'a str,
    date: Date,
    open: Rc<dyn Fn(Date)>,
}

impl View for MonthTitle<'_> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let lit = cx.is_hovered() || cx.is_focus_visible();
        let color = theme.color(if lit { Semantic::Accent } else { Semantic::Ink });
        let node = util::label(cx, self.text, TextStyle::Callout, Some(Weight::SEMIBOLD), color);
        let node = cx.container(LayoutStyle::stack(Axis::Horizontal), &[node], Paint::None);
        let (open, date) = (self.open.clone(), self.date);
        cx.on_click(node, Rc::new(move || open(date)));
        cx.semantics(node, Semantics::new(Role::Button).name(self.text.to_owned()));
        cx.focusable(node, theme.radii.sm);
        cx.cursor(node, CursorIcon::Pointer);
        node
    }
}

impl View for YearView {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let labels = cx.labels();
        let year = self.year.get();
        let first_of_year = |y: i32| Date::new(y, 1, 1).unwrap_or_else(|| Date::from_days(0));
        let month: State<Date> = cx.use_state(|| first_of_year(year));
        let seen = cx.use_state(|| Rc::new(Cell::new(year))).peek().clone();
        // The keyboard walks past December through the month it keeps, and the year follows;
        // a year set from outside takes the keyboard with it.
        let shown = if year != seen.get() {
            month.set(first_of_year(year));
            year
        } else if month.get().year() != year {
            self.year.set(month.peek().year());
            month.peek().year()
        } else {
            year
        };
        seen.set(shown);
        let keys = GridKeys {
            month: month.binding(),
            cursor: cx.use_state(|| None),
            focus_to: cx.use_state(|| Rc::new(Cell::new(None))).get(),
        };
        let selected = self.selected.as_ref().and_then(Binding::get);
        let active = [keys.cursor.get(), selected, self.today]
            .into_iter()
            .flatten()
            .find(|d| d.year() == shown)
            .unwrap_or(first_of_year(shown));

        let (back_to, on_to) = (self.year.clone(), self.year.clone());
        let back = cx.child(
            &IconButton::new(IconName::ChevronLeft)
                .label(labels.previous_year.clone())
                .size(ButtonSize::Small)
                .on_click(move || back_to.set(shown - 1)),
        );
        let title = util::label(
            cx,
            &shown.to_string(),
            TextStyle::Body,
            Some(Weight::SEMIBOLD),
            theme.color(Semantic::Ink),
        );
        let forward = cx.child(
            &IconButton::new(IconName::ChevronRight)
                .label(labels.next_year.clone())
                .size(ButtonSize::Small)
                .on_click(move || on_to.set(shown + 1)),
        );
        let header = cx.container(
            LayoutStyle {
                spacing: theme.spacing.s2,
                align: Align::Center,
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &[back, title, forward],
            Paint::None,
        );

        let width = 7.0 * DAY + 6.0 * DAY_GAP;
        let room = cx.scroll_viewport().map_or(cx.viewport().width, |size| size.width);
        let columns = self.columns.unwrap_or_else(|| {
            let fit = ((room + MONTH_GAP) / (width + MONTH_GAP)).floor() as usize;
            LINES.into_iter().find(|line| *line <= fit).unwrap_or(1)
        });
        let mut months = Vec::with_capacity(12);
        for number in 1..=12_u8 {
            let Some(first) = Date::new(shown, number, 1) else { continue };
            let name = labels.months[usize::from(number - 1)].clone();
            let title = match &self.on_month {
                Some(open) => cx.keyed_child(
                    u64::from(number),
                    &MonthTitle { text: &name, date: first, open: open.clone() },
                ),
                None => util::label(
                    cx,
                    &name,
                    TextStyle::Callout,
                    Some(Weight::SEMIBOLD),
                    theme.color(Semantic::Ink),
                ),
            };
            let mut cells = Vec::with_capacity(49);
            for short in &labels.weekdays {
                let letter: String = short.chars().take(1).collect();
                let l = util::label(cx, &letter, TextStyle::Caption, None, theme.color(Semantic::InkMuted));
                cx.semantics(l, Semantics::hidden());
                cells.push(cx.container(
                    LayoutStyle {
                        width: Some(DAY),
                        align: Align::Center,
                        justify: Justify::Center,
                        ..LayoutStyle::stack(Axis::Horizontal)
                    },
                    &[l],
                    Paint::None,
                ));
            }
            for _ in 0..first.weekday() {
                cells.push(cx.container(
                    LayoutStyle { width: Some(DAY), height: Some(DAY), ..LayoutStyle::leaf() },
                    &[],
                    Paint::None,
                ));
            }
            for day in 1..=days_in_month(shown, number) {
                let date = first.add_days(i32::from(day) - 1);
                let view = YearDay { date, view: self, active: date == active, keys: &keys };
                cells.push(cx.keyed_child(date.to_days() as u64 + (1 << 32), &view));
            }
            let grid = cx.container(
                LayoutStyle { grid_columns: Some(7), spacing: DAY_GAP, ..LayoutStyle::stack(Axis::Vertical) },
                &cells,
                Paint::None,
            );
            cx.semantics(grid, Semantics::new(Role::Calendar).name(labels.month_title(first)));
            months.push(cx.container(
                LayoutStyle {
                    spacing: theme.spacing.s2,
                    width: Some(width),
                    ..LayoutStyle::stack(Axis::Vertical)
                },
                &[title, grid],
                Paint::None,
            ));
        }
        let grid = cx.container(
            LayoutStyle {
                grid_columns: Some(columns as u16),
                spacing: MONTH_GAP,
                align_self: Some(Align::Start),
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &months,
            Paint::None,
        );
        let node = cx.container(
            LayoutStyle {
                spacing: theme.spacing.s3,
                align: Align::Start,
                align_self: Some(Align::Stretch),
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &[header, grid],
            Paint::None,
        );
        cx.report_frame(node);
        node
    }
}
