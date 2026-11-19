//! The stories of `MaskedField`, from the component gallery's example, by scripts/rcb-scaffold.py.
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

/// The gallery's example of `MaskedField`, with its settings taken from the story.
#[allow(unused_variables, unused_mut, clippy::redundant_clone, clippy::let_and_return, clippy::clone_on_copy)]
fn render(args: &Args) -> AnyView {
    Box::new({
        let arg_label = args.text("label");
        let arg_disabled = args.bool("disabled");
        let arg_keep_literals = args.bool("keep_literals");
        let arg_placeholder = args.text("placeholder");
        let arg_read_only = args.bool("read_only");

        let phone = State::new(args.text("phone"));
        {
            let mut it = MaskedField::new(phone.binding(), "(00) 00000-0000");
            it = it.label(arg_label.clone());
            it = it.disabled(arg_disabled);
            if arg_keep_literals {
                it = it.keep_literals();
            }
            if !arg_placeholder.is_empty() {
                it = it.placeholder(arg_placeholder.clone());
            }
            it = it.read_only(arg_read_only);
            it
        }
    })
}

impl ComponentBook for MaskedField {
    fn book() -> Meta {
        Meta::new("Actions and inputs/MaskedField", render)
            .summary("Text that follows a pattern, like a phone number.")
            .arg(ArgType::text("label", "Phone"))
            .arg(ArgType::bool("disabled", false))
            .arg(ArgType::bool("keep_literals", false))
            .arg(ArgType::text("placeholder", "").description("Empty leaves it as the component has it."))
            .arg(ArgType::bool("read_only", false))
            .arg(ArgType::text("phone", "11987654321").description("How the example starts."))
            .story("Example", Args::new())
            .story("Disabled", Args::new().with("disabled", true))
            .story("Read only", Args::new().with("read_only", true))
    }
}
