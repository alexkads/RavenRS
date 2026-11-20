//! The stories of `Stat`, from the component gallery's example, by scripts/rcb-scaffold.py.
//! Reviewed for #759: it is no longer written over.

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

/// The gallery's example of `Stat`, with its settings taken from the story.
#[allow(unused_variables, unused_mut, clippy::redundant_clone, clippy::let_and_return, clippy::clone_on_copy)]
fn render(args: &Args) -> AnyView {
    Box::new({
        let arg_large = args.bool("large");

        HStack::new((
            {
                let mut it = Stat::new("Revenue", "R$ 128.450").delta("+12.4%", true);
                if arg_large {
                    it = it.large();
                }
                it
            },
            Stat::new("Churn", "1.8%").delta("-0.4%", false),
        ))
        .spacing(32.0)
    })
}

impl ComponentBook for Stat {
    fn book() -> Meta {
        Meta::new("Data/Stat", render)
            .summary("Headline number with a label and a change indicator.")
            .arg(ArgType::bool("large", false))
            .story("Example", Args::new())
            .story("Large", Args::new().with("large", true))
    }
}
