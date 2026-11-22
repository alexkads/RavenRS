//! The stories of `TagBox`, from the component gallery's example, by scripts/rcb-scaffold.py.
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

/// The gallery's example of `TagBox`, with its settings taken from the story.
#[allow(unused_variables, unused_mut, clippy::redundant_clone, clippy::let_and_return, clippy::clone_on_copy)]
fn render(args: &Args) -> AnyView {
    Box::new({
        let arg_allow_new = args.bool("allow_new");
        let arg_label = args.text("label");
        let arg_disabled = args.bool("disabled");
        let arg_fill = args.bool("fill");
        let arg_placeholder = args.text("placeholder");
        let arg_read_only = args.bool("read_only");
        let arg_width = args.number("width") as f32;

        let tags = State::new(vec![String::from("urgent")]);
        {
            let mut it = TagBox::new(
                tags.binding(),
                vec![
                    LookupItem::new("urgent", "Urgent"),
                    LookupItem::new("review", "To review"),
                    LookupItem::new("paid", "Paid"),
                ],
            );
            it = it.allow_new(arg_allow_new);
            it = it.label(arg_label.clone());
            it = it.disabled(arg_disabled);
            if arg_fill {
                it = it.fill();
            }
            if !arg_placeholder.is_empty() {
                it = it.placeholder(arg_placeholder.clone());
            }
            it = it.read_only(arg_read_only);
            it = it.width(arg_width);
            it
        }
    })
}

impl ComponentBook for TagBox {
    fn book() -> Meta {
        Meta::new("Actions and inputs/TagBox", render)
            .summary("Several values from a list, shown as tags.")
            .arg(ArgType::bool("allow_new", true))
            .arg(ArgType::text("label", "Tags"))
            .arg(ArgType::bool("disabled", false))
            .arg(ArgType::bool("fill", false))
            .arg(ArgType::text("placeholder", "").description("Empty leaves it as the component has it."))
            .arg(ArgType::bool("read_only", false))
            .arg(ArgType::number("width", 280.0, 0.0, 1120.0, 1.0))
            .story("Example", Args::new())
            .story("Disabled", Args::new().with("disabled", true))
            .story("Read only", Args::new().with("read_only", true))
    }
}
