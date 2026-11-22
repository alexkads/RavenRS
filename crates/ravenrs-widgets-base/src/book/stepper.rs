//! The stories of `Stepper`, from the component gallery's example, by scripts/rcb-scaffold.py.
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

/// The gallery's example of `Stepper`, with its settings taken from the story.
#[allow(unused_variables, unused_mut, clippy::redundant_clone, clippy::let_and_return, clippy::clone_on_copy)]
fn render(args: &Args) -> AnyView {
    Box::new({
        let arg_label = args.text("label");
        let arg_disabled = args.bool("disabled");
        let arg_read_only = args.bool("read_only");

        let seats = State::new(args.number("seats") as _);
        {
            let mut it = Stepper::new(seats.binding()).range(1, 50);
            it = it.label(arg_label.clone());
            it = it.disabled(arg_disabled);
            it = it.read_only(arg_read_only);
            it
        }
    })
}

impl ComponentBook for Stepper {
    fn book() -> Meta {
        Meta::new("Actions and inputs/Stepper", render)
            .summary("Decrement, value and increment for small quantities.")
            .arg(ArgType::text("label", "Seats"))
            .arg(ArgType::bool("disabled", false))
            .arg(ArgType::bool("read_only", false))
            .arg(ArgType::number("seats", 3.0, 0.0, 12.0, 1.0).description("How the example starts."))
            .story("Example", Args::new())
            .story("Disabled", Args::new().with("disabled", true))
            .story("Read only", Args::new().with("read_only", true))
    }
}
