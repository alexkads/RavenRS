//! The stories of `DateRangePicker`, from the component gallery's example, by scripts/rcb-scaffold.py.
//! Reviewed for #755: it is no longer written over.

#[allow(unused_imports)]
use crate::book::{ArgType, ArgValue, Args, ComponentBook, Meta};
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
use std::collections::{BTreeMap, BTreeSet, HashMap};
#[allow(unused_imports)]
use std::rc::Rc;

/// The gallery's example of `DateRangePicker`, with its settings taken from the story.
#[allow(unused_variables, unused_mut, clippy::redundant_clone, clippy::let_and_return, clippy::clone_on_copy)]
fn render(args: &Args) -> AnyView {
    Box::new({
        let arg_label = args.text("label");
        let arg_disabled = args.bool("disabled");
        let arg_placeholder = args.text("placeholder");
        let arg_read_only = args.bool("read_only");
        let arg_width = args.number("width") as f32;

        let today = Date::new(2026, 11, 2).unwrap_or_else(|| unreachable!("a valid date"));
        let period = State::new(Some(DateRange::new(today, today.add_days(6))));
        {
            let mut it = DateRangePicker::new(period.binding(), today)
                .today(today)
                .preset("Last 7 days", DateRange::new(today.add_days(-6), today))
                .preset("Last 30 days", DateRange::new(today.add_days(-29), today));
            it = it.label(arg_label.clone());
            it = it.disabled(arg_disabled);
            if !arg_placeholder.is_empty() {
                it = it.placeholder(arg_placeholder.clone());
            }
            it = it.read_only(arg_read_only);
            it = it.width(arg_width);
            it
        }
    })
}

impl ComponentBook for DateRangePicker {
    fn book() -> Meta {
        Meta::new("Actions and inputs/DateRangePicker", render)
            .summary("A period in two clicks, or ready-made.")
            .arg(ArgType::text("label", "Period"))
            .arg(ArgType::bool("disabled", false))
            .arg(ArgType::text("placeholder", "").description("Empty leaves it as the component has it."))
            .arg(ArgType::bool("read_only", false))
            .arg(ArgType::number("width", 340.0, 0.0, 1360.0, 1.0))
            .story("Example", Args::new())
            .story("Disabled", Args::new().with("disabled", true))
            .story("Read only", Args::new().with("read_only", true))
    }
}
