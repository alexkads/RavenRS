//! The stories of `DaySeparator`, from the component gallery's example, by scripts/rcb-scaffold.py.
//! Reviewed for #761: it is no longer written over.

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
use std::collections::{BTreeMap, BTreeSet, HashMap};
#[allow(unused_imports)]
use std::rc::Rc;

/// The gallery's example of `DaySeparator`, with its settings taken from the story.
#[allow(unused_variables, unused_mut, clippy::redundant_clone, clippy::let_and_return, clippy::clone_on_copy)]
fn render(args: &Args) -> AnyView {
    Box::new({
        let today = Date::new(2026, 11, 4).unwrap_or_else(|| Date::from_days(0));
        VStack::new((
            {
                let mut it = DaySeparator::of(today, today);
                it
            },
            DaySeparator::of(today.add_days(-1), today),
            DaySeparator::new("Last week"),
        ))
        .spacing(16.0)
        .align(Align::Stretch)
    })
}

impl ComponentBook for DaySeparator {
    fn book() -> Meta {
        Meta::new("Display and feedback/DaySeparator", render)
            .summary("A line with a word in the middle that divides a list by day.")
            .story("Example", Args::new())
    }
}
