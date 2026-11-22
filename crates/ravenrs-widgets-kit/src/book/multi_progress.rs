//! The stories of `MultiProgress`, from the component gallery's example, by scripts/rcb-scaffold.py.
//! Reviewed for #761: it is no longer written over.

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

/// The gallery's example of `MultiProgress`, with its settings taken from the story.
#[allow(unused_variables, unused_mut, clippy::redundant_clone, clippy::let_and_return, clippy::clone_on_copy)]
fn render(args: &Args) -> AnyView {
    Box::new({
        let arg_label = args.text("label");

        {
            let mut it = MultiProgress::new().meter("Local copy", 84, 120).meter("Previews", 12, 84);
            it = it.label(arg_label.clone());
            it
        }
    })
}

impl ComponentBook for MultiProgress {
    fn book() -> Meta {
        Meta::new("Display and feedback/MultiProgress", render)
            .summary("Several progress bars stacked, for work with more than one part.")
            .arg(ArgType::text("label", "Import"))
            .story("Example", Args::new())
    }
}
