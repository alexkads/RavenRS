//! The stories of `Link`, from the component gallery's example, by scripts/rcb-scaffold.py.
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

/// The gallery's example of `Link`, with its settings taken from the story.
#[allow(unused_variables, unused_mut, clippy::redundant_clone, clippy::let_and_return, clippy::clone_on_copy)]
fn render(args: &Args) -> AnyView {
    Box::new({
        let arg_muted = args.bool("muted");

        VStack::new((
            {
                let mut it = Link::new(
                    "Read the getting started guide",
                    "https://github.com/alexkads/RavenRS/blob/master/docs/GETTING_STARTED.md",
                )
                .icon(IconName::ArrowRight);
                if arg_muted {
                    it = it.muted();
                }
                it
            },
            Link::new("Components", "https://alexkads.github.io/RavenRS/components/").muted(),
        ))
        .spacing(4.0)
    })
}

impl ComponentBook for Link {
    fn book() -> Meta {
        Meta::new("Layout/Link", render)
            .summary("Text that opens an address, or scrolls to an anchor.")
            .arg(ArgType::bool("muted", false))
            .story("Example", Args::new())
            .story("Muted", Args::new().with("muted", true))
    }
}
