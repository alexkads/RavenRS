//! The stories of `QuarterTurn`, from the component gallery's example, by scripts/rcb-scaffold.py.
//! Reviewed for #762: it is no longer written over.

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

/// The gallery's example of `QuarterTurn`, with its settings taken from the story.
#[allow(unused_variables, unused_mut, clippy::redundant_clone, clippy::let_and_return, clippy::clone_on_copy)]
fn render(args: &Args) -> AnyView {
    Box::new({
        HStack::new((
            {
                let mut it = QuarterTurn::new(Text::new("Files"), Turn::Clockwise);
                it
            },
            QuarterTurn::new(Text::new("Search"), Turn::CounterClockwise),
            Text::new("Names down a narrow strip, read the same by screen readers"),
        ))
        .spacing(16.0)
    })
}

impl ComponentBook for QuarterTurn<AnyView> {
    fn book() -> Meta {
        Meta::new("Media/QuarterTurn", render).summary("QuarterTurn.").story("Example", Args::new())
    }
}
