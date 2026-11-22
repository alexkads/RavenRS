//! The stories of `Spacer`, from the component gallery's example, by scripts/rcb-scaffold.py.
//! Reviewed for #760: it is no longer written over.

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

/// The gallery's example of `Spacer`, with its settings taken from the story.
#[allow(unused_variables, unused_mut, clippy::redundant_clone, clippy::let_and_return, clippy::clone_on_copy)]
fn render(args: &Args) -> AnyView {
    Box::new({
        HStack::new((
            Text::new("Left"),
            {
                let mut it = Spacer::new();
                it
            },
            Button::new("Next"),
        ))
        .align(Align::Center)
    })
}

impl ComponentBook for Spacer {
    fn book() -> Meta {
        Meta::new("Layout/Spacer", render)
            .summary("Flexible space that pushes its siblings apart.")
            .story("Example", Args::new())
    }
}
