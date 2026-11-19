//! The stories of `Combobox`, from the component gallery's example, by scripts/rcb-scaffold.py.
//! Reviewed for #756: it is no longer written over.

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

/// The gallery's example of `Combobox`, with its settings taken from the story.
#[allow(unused_variables, unused_mut, clippy::redundant_clone, clippy::let_and_return, clippy::clone_on_copy)]
fn render(args: &Args) -> AnyView {
    Box::new({
        let arg_placeholder = args.text("placeholder");
        let arg_label = args.text("label");
        let arg_disabled = args.bool("disabled");
        let arg_fill = args.bool("fill");
        let arg_read_only = args.bool("read_only");
        let arg_width = args.number("width") as f32;

        let country = State::new(None);
        {
            let mut it =
                Combobox::new(country.binding(), ["Argentina", "Brazil", "Chile", "Portugal", "Spain"]);
            it = it.placeholder(arg_placeholder.clone());
            it = it.label(arg_label.clone());
            it = it.disabled(arg_disabled);
            if arg_fill {
                it = it.fill();
            }
            it = it.read_only(arg_read_only);
            it = it.width(arg_width);
            it
        }
    })
}

impl ComponentBook for Combobox {
    fn book() -> Meta {
        Meta::new("Forms/Combobox", render)
            .summary("Select with a search box that filters the options.")
            .arg(ArgType::text("placeholder", "Choose a country"))
            .arg(ArgType::text("label", "Country"))
            .arg(ArgType::bool("disabled", false))
            .arg(ArgType::bool("fill", false))
            .arg(ArgType::bool("read_only", false))
            .arg(ArgType::number("width", 240.0, 0.0, 960.0, 1.0))
            .story("Example", Args::new())
            .story("Disabled", Args::new().with("disabled", true))
            .story("Read only", Args::new().with("read_only", true))
    }
}
