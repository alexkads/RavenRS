//! The stories of `Spinner`, from the component gallery's example, by scripts/rcb-scaffold.py.
//! Reviewed for #761: it is no longer written over.

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

/// The gallery's example of `Spinner`, with its settings taken from the story.
#[allow(unused_variables, unused_mut, clippy::redundant_clone, clippy::let_and_return, clippy::clone_on_copy)]
fn render(args: &Args) -> AnyView {
    Box::new({
        {
            let mut it = Spinner::new();
            it = it.size(args.number("size") as f32);
            it
        }
    })
}

impl ComponentBook for Spinner {
    fn book() -> Meta {
        Meta::new("Display and feedback/Spinner", render)
            .summary("Indeterminate activity indicator.")
            .arg(ArgType::number("size", 20.0, 8.0, 96.0, 1.0).description("Its diameter, in points."))
            .story("Example", Args::new())
            .story("Large", Args::new().with("size", 40.0))
    }
}
