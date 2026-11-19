//! The stories of `Disclosure`, from the component gallery's example, by scripts/rcb-scaffold.py.
//! Reviewed for #757: it is no longer written over.

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

/// The gallery's example of `Disclosure`, with its settings taken from the story.
#[allow(unused_variables, unused_mut, clippy::redundant_clone, clippy::let_and_return, clippy::clone_on_copy)]
fn render(args: &Args) -> AnyView {
    Box::new({
        let arg_open = args.bool("open");

        {
            let mut it = Disclosure::new(
                "Advanced options",
                Text::new("Shown when open.").foreground(Semantic::InkMuted),
            );
            it = it.open(arg_open);
            it
        }
        // Tells what is inside while it is closed. `.switch` and `.expanded` take bindings.
        .mark(Dot::new().tone(Tone::Warning).label("Changed"))
    })
}

impl ComponentBook for Disclosure {
    fn book() -> Meta {
        Meta::new("Navigation/Disclosure", render)
            .summary("Header that shows or hides its content.")
            .arg(ArgType::bool("open", false))
            .story("Example", Args::new())
            .story("Open", Args::new().with("open", true))
    }
}
