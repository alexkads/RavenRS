//! The stories of `EventCalendar`, from the component gallery's example, by scripts/rcb-scaffold.py.
//! Reviewed for #756: it is no longer written over.

#[allow(unused_imports)]
use crate::*;
#[allow(unused_imports)]
use ravenrs_core::*;
#[allow(unused_imports)]
use ravenrs_layout::*;
#[allow(unused_imports)]
use ravenrs_reactive::*;
#[allow(unused_imports)]
use ravenrs_style::*;
#[allow(unused_imports)]
use ravenrs_widgets_base::book::{ArgType, ArgValue, Args, ComponentBook, Meta};
#[allow(unused_imports)]
use ravenrs_widgets_base::*;
#[allow(unused_imports)]
use ravenrs_widgets_kit::*;
#[allow(unused_imports)]
use std::collections::{BTreeMap, BTreeSet, HashMap};
#[allow(unused_imports)]
use std::rc::Rc;

/// The gallery's example of `EventCalendar`, with its settings taken from the story.
#[allow(unused_variables, unused_mut, clippy::redundant_clone, clippy::let_and_return, clippy::clone_on_copy)]
fn render(args: &Args) -> AnyView {
    Box::new({
        let arg_events_per_day = args.number("events_per_day") as usize;

        let today = Date::new(2026, 11, 16).unwrap_or_else(|| unreachable!("a real date"));
        let (month, day) = (State::new(today.first_of_month()), State::new(Some(today)));
        {
            let mut it = EventCalendar::new(month.binding(), day.binding()).today(today).events([
                CalendarEvent::new(1, today.add_days(-4), "Rent").done(true),
                CalendarEvent::new(2, today, "Review").tone(Tone::Warning),
                CalendarEvent::new(3, today.add_days(3), "Salary").tone(Tone::Success),
            ]);
            it = it.events_per_day(arg_events_per_day);
            let view = State::new(match args.choice("view") {
                "Week" => CalendarView::Week,
                "Day" => CalendarView::Day,
                "Agenda" => CalendarView::Agenda,
                "Year" => CalendarView::Year,
                "Season" => CalendarView::Season,
                _ => CalendarView::Month,
            });
            // The month, week and day keep the views they always offered.
            if !matches!(view.get(), CalendarView::Month | CalendarView::Week | CalendarView::Day) {
                it = it.views(CalendarView::ALL);
            }
            it = it.view(view.binding());
            it
        }
    })
}

impl ComponentBook for EventCalendar {
    fn book() -> Meta {
        Meta::new("Forms/EventCalendar", render)
            .summary("A month with its events inside each day.")
            .arg(ArgType::number("events_per_day", 3.0, 0.0, 12.0, 1.0))
            .arg(
                ArgType::choice("view", &["Month", "Week", "Day", "Agenda", "Year", "Season"], "Month")
                    .description(
                        "A month of days, a week or a day with an hour on each line, the events in a list, \
                         the year or the season.",
                    ),
            )
            .story("Month", Args::new())
            .story("Week", Args::new().with("view", ArgValue::choice("Week")))
            .story("Day", Args::new().with("view", ArgValue::choice("Day")))
            .story("Agenda", Args::new().with("view", ArgValue::choice("Agenda")))
            .story("Year", Args::new().with("view", ArgValue::choice("Year")))
            .story("Season", Args::new().with("view", ArgValue::choice("Season")))
    }
}
