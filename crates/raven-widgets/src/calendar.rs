//! Calendar and date picker.

use std::rc::Rc;

use raven_core::{Border, Color, CursorIcon, Insets};
use raven_layout::{Align, Axis, Justify, LayoutStyle};
use raven_reactive::{Binding, State};
use raven_style::{Semantic, TextStyle, Weight};

use crate::context::{BuildCx, NodeId, Paint, Placement, QuadPaint};
use crate::date::Date;
use crate::{ButtonSize, IconButton, IconName, View, util};

/// A month grid to pick a date. Use ‹ › to change month.
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
}

impl View for Day<'_> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let c = self.calendar;
        let selected = c.selected.get() == Some(self.date);
        let enabled = c.min.is_none_or(|m| self.date >= m) && c.max.is_none_or(|m| self.date <= m);
        let hovered = cx.is_hovered() && enabled;
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
        let start = shown.add_days(-i32::from(shown.weekday()));
        for i in 0..42 {
            let date = start.add_days(i);
            let day = Day { date, in_month: date.month() == shown.month(), calendar: self };
            cells.push(cx.keyed_child(date.to_days() as u64, &day));
        }
        let grid = cx.container(
            LayoutStyle { grid_columns: Some(7), spacing: 2.0, ..LayoutStyle::stack(Axis::Vertical) },
            &cells,
            Paint::None,
        );
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
}

impl DatePicker {
    /// `month` is shown when no date is selected yet.
    pub fn new(value: Binding<Option<Date>>, month: Date) -> Self {
        Self { value, month, placeholder: "Pick a date".into(), today: None, width: 220.0 }
    }

    pub fn placeholder(mut self, text: impl Into<String>) -> Self {
        self.placeholder = text.into();
        self
    }

    pub fn today(mut self, today: Date) -> Self {
        self.today = Some(today);
        self
    }

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
                ..QuadPaint::new(theme.color(Semantic::Surface))
            }),
        );
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
