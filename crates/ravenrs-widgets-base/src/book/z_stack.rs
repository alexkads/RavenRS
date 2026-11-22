//! The stories of `ZStack`, from the component gallery's example, by scripts/rcb-scaffold.py.
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

/// The gallery's example of `ZStack`, with its settings taken from the story.
#[allow(unused_variables, unused_mut, clippy::redundant_clone, clippy::let_and_return, clippy::clone_on_copy)]
fn render(args: &Args) -> AnyView {
    Box::new({
        {
            let mut it = ZStack::new((
                Spacer::new().frame(Some(220.0), Some(80.0)).background(Semantic::Accent).corner_radius(8.0),
                Badge::new("On top").tone(Tone::Success),
            ))
            .align(Align::Center);
            it
        }
    })
}

impl ComponentBook for ZStack<(AnyView,)> {
    fn book() -> Meta {
        Meta::new("Layout/ZStack", render)
            .summary("Views on top of each other, in a box as large as the largest.")
            .story("Example", Args::new())
    }
}
