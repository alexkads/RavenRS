//! The stories of `StepBadge`, from the component gallery's example, by scripts/rcb-scaffold.py.
//! Reviewed for #755: it is no longer written over.

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

/// The gallery's example of `StepBadge`, with its settings taken from the story.
#[allow(unused_variables, unused_mut, clippy::redundant_clone, clippy::let_and_return, clippy::clone_on_copy)]
fn render(args: &Args) -> AnyView {
    Box::new({
        let arg_state = match args.choice("state") {
            "Upcoming" => StepState::Upcoming,
            "Current" => StepState::Current,
            "Error" => StepState::Error,
            _ => StepState::Done,
        };
        let arg_size = args.number("size") as f32;

        HStack::new((
            {
                let mut it = StepBadge::new(1);
                it = it.state(arg_state.clone());
                it = it.size(arg_size);
                it
            },
            StepBadge::new(2).state(StepState::Error),
            StepBadge::new(3).state(StepState::Current),
            StepBadge::new(4),
        ))
        .spacing(8.0)
    })
}

impl ComponentBook for StepBadge {
    fn book() -> Meta {
        Meta::new("Actions and inputs/StepBadge", render)
            .summary("The round mark of a step.")
            .arg(ArgType::choice("state", &["Upcoming", "Current", "Done", "Error"], "Done"))
            .arg(ArgType::number("size", 24.0, 0.0, 96.0, 1.0))
            .story("Example", Args::new())
            .story("State: Upcoming", Args::new().with("state", ArgValue::choice("Upcoming")))
            .story("State: Current", Args::new().with("state", ArgValue::choice("Current")))
            .story("State: Error", Args::new().with("state", ArgValue::choice("Error")))
    }
}
