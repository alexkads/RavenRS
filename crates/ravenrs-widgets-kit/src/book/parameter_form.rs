//! The stories of `ParameterForm`, from the component gallery's example, by scripts/rcb-scaffold.py.
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

/// The gallery's example of `ParameterForm`, with its settings taken from the story.
#[allow(unused_variables, unused_mut, clippy::redundant_clone, clippy::let_and_return, clippy::clone_on_copy)]
fn render(args: &Args) -> AnyView {
    Box::new({
        let values = State::new(ParamValues::new());
        {
            let mut it = ParameterForm::new(
                vec![
                    ParamField::new("url", "Address", ParamKind::Text).required(),
                    ParamField::new("retries", "Retries", ParamKind::Number).default(3),
                    ParamField::new("when", "Only when", ParamKind::Text).expressions(),
                    ParamField::new(
                        "headers",
                        "Headers",
                        ParamKind::List(vec![
                            ParamField::new("name", "Name", ParamKind::Text),
                            ParamField::new("value", "Value", ParamKind::Text),
                        ]),
                    ),
                ],
                values.binding(),
            );
            if args.bool("live") {
                it = it.live(true);
            }
            it
        }
    })
}

impl ComponentBook for ParameterForm {
    fn book() -> Meta {
        Meta::new("Actions and inputs/ParameterForm", render)
            .summary("A form made from a schema of parameters: groups, lists of rows, options that depend on another field, and a value or an expression in each field.")
            .arg(ArgType::bool("live", false).description("Each change goes back at once, with no Apply or Discard: for a form inside a dialog."))
            .story("Example", Args::new())
            .story("Live, inside a dialog", Args::new().with("live", true))
    }
}
