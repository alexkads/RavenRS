//! The stories of `StepIndicator`, from the component gallery's example, by scripts/rcb-scaffold.py.
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

/// The gallery's example of `StepIndicator`, with its settings taken from the story.
#[allow(unused_variables, unused_mut, clippy::redundant_clone, clippy::let_and_return, clippy::clone_on_copy)]
fn render(args: &Args) -> AnyView {
    Box::new({
        let arg_vertical = args.bool("vertical");

        {
            let mut it = StepIndicator::new(vec![
                IndicatorStep::new("Client", StepState::Done),
                IndicatorStep::new("Photos", StepState::Error).note("Two are missing"),
                IndicatorStep::new("Review", StepState::Current),
                IndicatorStep::new("Send", StepState::Upcoming),
            ])
            .on_select(|_step| {});
            if arg_vertical {
                it = it.vertical();
            }
            it
        }
    })
}

impl ComponentBook for StepIndicator {
    fn book() -> Meta {
        Meta::new("Actions and inputs/StepIndicator", render)
            .summary("Where a task in steps stands, for an app's own layout.")
            .arg(ArgType::bool("vertical", true))
            .story("Example", Args::new())
    }
}
