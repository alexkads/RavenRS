//! The stories of `YearView`, from the component gallery's example, by scripts/rcb-scaffold.py.
//! Reviewed for #757: it is no longer written over.

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

/// The gallery's example of `YearView`, with its settings taken from the story.
#[allow(unused_variables, unused_mut, clippy::redundant_clone, clippy::let_and_return, clippy::clone_on_copy)]
fn render(args: &Args) -> AnyView {
    Box::new({
        let year = State::new(args.number("year") as _);
        let chosen = State::new(None::<Date>);
        {
            let mut it = YearView::new(year.binding())
                .selected(chosen.binding())
                .today(Date::new(2026, 11, 5).unwrap_or_else(|| Date::from_days(0)))
                .marks(|day| match day.day() % 11 {
                    0 => Some(DayMark::new().tone(Tone::Accent).label("Shoot")),
                    5 => Some(DayMark::new().tone(Tone::Success).label("Delivery")),
                    _ => None,
                });
            it
        }
    })
}

impl ComponentBook for YearView {
    fn book() -> Meta {
        Meta::new("Navigation/YearView", render)
            .summary("The twelve months of a year, with marks on the days.")
            .arg(ArgType::number("year", 2026.0, 2000.0, 2050.0, 1.0).description("How the example starts."))
            .story("Example", Args::new())
    }
}
