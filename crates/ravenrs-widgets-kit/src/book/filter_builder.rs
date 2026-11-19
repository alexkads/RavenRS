//! The stories of `FilterBuilder`, from the component gallery's example, by scripts/rcb-scaffold.py.
//! Reviewed for #755: it is no longer written over.

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

/// The gallery's example of `FilterBuilder`, with its settings taken from the story.
#[allow(unused_variables, unused_mut, clippy::redundant_clone, clippy::let_and_return, clippy::clone_on_copy)]
fn render(args: &Args) -> AnyView {
    Box::new({
        let filter = State::new(args.text("filter"));
        {
            let mut it = FilterBuilder::new(
                filter.binding(),
                [
                    FilterField::new("Name", "Name", FilterFieldKind::Text),
                    FilterField::new("Price", "Price", FilterFieldKind::Number),
                    FilterField::new("Active", "Active", FilterFieldKind::Boolean),
                ],
            );
            it
        }
    })
}

impl ComponentBook for FilterBuilder {
    fn book() -> Meta {
        Meta::new("Actions and inputs/FilterBuilder", render)
            .summary("Builds a filter out of lines, in the expression language.")
            .arg(
                ArgType::text("filter", "[Price] > 10 And Contains([Name], 'pro')")
                    .description("How the example starts."),
            )
            .story("Example", Args::new())
    }
}
