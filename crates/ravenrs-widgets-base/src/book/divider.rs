//! The stories of `Divider`, from the component gallery's example, by scripts/rcb-scaffold.py.
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

/// The gallery's example of `Divider`, with its settings taken from the story.
#[allow(unused_variables, unused_mut, clippy::redundant_clone, clippy::let_and_return, clippy::clone_on_copy)]
fn render(args: &Args) -> AnyView {
    Box::new({
        VStack::new((
            Text::new("Above"),
            {
                let mut it = Divider::horizontal();
                it
            },
            Text::new("Below"),
        ))
        .spacing(12.0)
        .align(Align::Stretch)
    })
}

impl ComponentBook for Divider {
    fn book() -> Meta {
        Meta::new("Layout/Divider", render).summary("Hairline separator.").story("Example", Args::new())
    }
}
