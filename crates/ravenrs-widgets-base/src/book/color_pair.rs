//! The stories of `ColorPair`, from the component gallery's example, by scripts/rcb-scaffold.py.
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

/// The gallery's example of `ColorPair`, with its settings taken from the story.
#[allow(unused_variables, unused_mut, clippy::redundant_clone, clippy::let_and_return, clippy::clone_on_copy)]
fn render(args: &Args) -> AnyView {
    Box::new({
        let arg_size = args.number("size") as f32;

        let (front, back) = (State::new(Color::hex(0xe11d48)), State::new(Color::WHITE));
        {
            let mut it = ColorPair::new(front.binding(), back.binding()).defaults(Color::BLACK, Color::WHITE);
            it = it.size(arg_size);
            it
        }
    })
}

impl ComponentBook for ColorPair {
    fn book() -> Meta {
        Meta::new("Actions and inputs/ColorPair", render)
            .summary("Foreground and background colors, one over the other, with a swap button.")
            .arg(ArgType::number("size", 24.0, 0.0, 96.0, 1.0))
            .story("Example", Args::new())
    }
}
