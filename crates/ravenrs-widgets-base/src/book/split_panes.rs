//! The stories of `SplitPanes`, from the component gallery's example, by scripts/rcb-scaffold.py.
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

/// The gallery's example of `SplitPanes`, with its settings taken from the story.
#[allow(unused_variables, unused_mut, clippy::redundant_clone, clippy::let_and_return, clippy::clone_on_copy)]
fn render(args: &Args) -> AnyView {
    Box::new({
        let arg_height = args.number("height") as f32;

        {
            let mut it = SplitPanes::horizontal()
                .pane(SplitPane::new(Text::new("Sessions").padding(16.0)).size(180.0).range(120.0, 260.0))
                .pane(SplitPane::new(Text::new("Receipt").padding(16.0)).range(120.0, f32::MAX))
                .pane(SplitPane::new(Text::new("Activity").padding(16.0)).size(180.0).range(120.0, 260.0));
            it = it.height(arg_height);
            it
        }
    })
}

impl ComponentBook for SplitPanes {
    fn book() -> Meta {
        Meta::new("Layout/SplitPanes", render)
            .summary("Any number of panes with a bar between each two, and limits.")
            .arg(ArgType::number("height", 190.0, 0.0, 760.0, 1.0))
            .story("Example", Args::new())
    }
}
