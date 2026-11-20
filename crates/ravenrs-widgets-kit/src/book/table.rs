//! The stories of `Table`, from the component gallery's example, by scripts/rcb-scaffold.py.
//! Reviewed for #759: it is no longer written over.

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

/// The gallery's example of `Table`, with its settings taken from the story.
#[allow(unused_variables, unused_mut, clippy::redundant_clone, clippy::let_and_return, clippy::clone_on_copy)]
fn render(args: &Args) -> AnyView {
    Box::new({
        {
            let mut it = Table::new([
                Column::new("Invoice"),
                Column::new("Customer").weight(2.0),
                Column::new("Amount").numeric(),
            ])
            .row(["INV-1042", "Northwind Labs", "R$ 4.800"])
            .row(["INV-1043", "Aurora Studio", "R$ 1.720"]);
            it
        }
    })
}

impl ComponentBook for Table {
    fn book() -> Meta {
        Meta::new("Data/Table", render)
            .summary("Simple table with a header, hover rows and numeric columns.")
            .story("Example", Args::new())
    }
}
