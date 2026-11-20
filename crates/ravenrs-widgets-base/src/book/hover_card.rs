//! The stories of `HoverCard`, from the component gallery's example, by scripts/rcb-scaffold.py.
//! Reviewed for #758: it is no longer written over. It opens while the pointer rests on the trigger, which the book measures as the hover state.

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

/// The gallery's example of `HoverCard`, with its settings taken from the story.
#[allow(unused_variables, unused_mut, clippy::redundant_clone, clippy::let_and_return, clippy::clone_on_copy)]
fn render(args: &Args) -> AnyView {
    Box::new({
        {
            let mut it = HoverCard::new(
                Text::new("@ana.lima").foreground(Semantic::Accent),
                HStack::new((Avatar::new("Ana Lima"), Text::new("Design lead at Northwind"))).spacing(12.0),
            );
            it
        }
    })
}

impl ComponentBook for HoverCard<AnyView, AnyView> {
    fn book() -> Meta {
        Meta::new("Overlays/HoverCard", render)
            .summary("Rich content shown while the pointer is over a trigger.")
            .story("Example", Args::new())
    }
}
