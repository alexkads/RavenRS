//! The stories of `DataInspector`, from the component gallery's example, by scripts/rcb-scaffold.py.
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
use ravenrs_widgets_kit::*;
#[allow(unused_imports)]
use std::collections::{BTreeMap, BTreeSet, HashMap};
#[allow(unused_imports)]
use std::rc::Rc;

/// The gallery's example of `DataInspector`, with its settings taken from the story.
#[allow(unused_variables, unused_mut, clippy::redundant_clone, clippy::let_and_return, clippy::clone_on_copy)]
fn render(args: &Args) -> AnyView {
    Box::new({
        let arg_label = args.text("label");

        {
            let mut it = DataInspector::new(
                r#"[{ "order": 101, "customer": { "name": "Ana" }, "discount": null },
            { "order": 102, "customer": { "name": "Bia" } }]"#,
            );
            it = it.label(arg_label.clone());
            it
        }
    })
}

impl ComponentBook for DataInspector {
    fn book() -> Meta {
        Meta::new("Data/DataInspector", render)
            .summary("JSON items as a table, a tree, the JSON and a schema, with the field chosen kept between them.")
            .arg(ArgType::text("label", "Orders"))
            .story("Example", Args::new())
    }
}
