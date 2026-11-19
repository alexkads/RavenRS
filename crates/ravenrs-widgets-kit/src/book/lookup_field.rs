//! The stories of `LookupField`, from the component gallery's example, by scripts/rcb-scaffold.py.
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

/// The gallery's example of `LookupField`, with its settings taken from the story.
#[allow(unused_variables, unused_mut, clippy::redundant_clone, clippy::let_and_return, clippy::clone_on_copy)]
fn render(args: &Args) -> AnyView {
    Box::new({
        let arg_placeholder = args.text("placeholder");
        let arg_width = args.number("width") as f32;
        let arg_label = args.text("label");
        let arg_disabled = args.bool("disabled");
        let arg_read_only = args.bool("read_only");

        let city = State::new(None);
        {
            let mut it = LookupField::remote(city.binding(), |request, responder| {
                // Ask a database or a server here; a clone of the responder can answer later.
                let cities = ["Belo Horizonte", "Recife", "Salvador", "São Paulo"];
                let needle = request.query.to_lowercase();
                let found = cities.iter().filter(|c| c.to_lowercase().contains(&needle));
                responder.respond(found.map(|c| LookupItem::new(*c, *c)).collect());
            });
            it = it.placeholder(arg_placeholder.clone());
            it = it.width(arg_width);
            it = it.label(arg_label.clone());
            it = it.disabled(arg_disabled);
            it = it.read_only(arg_read_only);
            it
        }
    })
}

impl ComponentBook for LookupField {
    fn book() -> Meta {
        Meta::new("Actions and inputs/LookupField", render)
            .summary("Picks a record from a list searched on demand.")
            .arg(ArgType::text("placeholder", "Choose a city"))
            .arg(ArgType::number("width", 220.0, 0.0, 880.0, 1.0))
            .arg(ArgType::text("label", "City"))
            .arg(ArgType::bool("disabled", false))
            .arg(ArgType::bool("read_only", false))
            .story("Example", Args::new())
            .story("Disabled", Args::new().with("disabled", true))
            .story("Read only", Args::new().with("read_only", true))
    }
}
