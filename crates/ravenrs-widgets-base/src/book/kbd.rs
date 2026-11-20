//! The stories of `Kbd`, from the component gallery's example, by scripts/rcb-scaffold.py.
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

/// The gallery's example of `Kbd`, with its settings taken from the story.
#[allow(unused_variables, unused_mut, clippy::redundant_clone, clippy::let_and_return, clippy::clone_on_copy)]
fn render(args: &Args) -> AnyView {
    Box::new({
        let arg_inherit_color = args.bool("inherit_color");

        HStack::new((
            {
                let mut it = Kbd::new("⌘");
                if arg_inherit_color {
                    it = it.inherit_color();
                }
                it
            },
            Kbd::new("Shift"),
            Kbd::new("K"),
            // The button gives the key its own color.
            Button::new("Open").size(ButtonSize::Small).shortcut("F8"),
        ))
        .spacing(8.0)
        .align(Align::Center)
    })
}

impl ComponentBook for Kbd {
    fn book() -> Meta {
        Meta::new("Display and feedback/Kbd", render)
            .summary("Keyboard key, for showing shortcuts.")
            .arg(ArgType::bool("inherit_color", false))
            .story("Example", Args::new())
    }
}
