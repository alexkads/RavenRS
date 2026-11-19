//! The stories of `ToggleButton`, from the component gallery's example, by scripts/rcb-scaffold.py.
//! Reviewed for #755: it is no longer written over.

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

/// The gallery's example of `ToggleButton`, with its settings taken from the story.
#[allow(unused_variables, unused_mut, clippy::redundant_clone, clippy::let_and_return, clippy::clone_on_copy)]
fn render(args: &Args) -> AnyView {
    Box::new({
        let arg_pill = args.bool("pill");

        let bold = State::new(args.bool("bold"));
        let italic = State::new(args.bool("italic"));
        let open = State::new(args.bool("open"));
        HStack::new((
            {
                let mut it = ToggleButton::new(bold.binding(), "Bold");
                if arg_pill {
                    it = it.pill();
                }
                it
            },
            ToggleButton::new(italic.binding(), "Italic"),
            ToggleButton::new(open.binding(), "Open").count(12).pill(),
        ))
        .spacing(8.0)
    })
}

impl ComponentBook for ToggleButton {
    fn book() -> Meta {
        Meta::new("Actions and inputs/ToggleButton", render)
            .summary("Button that stays pressed, like bold or mute.")
            .arg(ArgType::bool("pill", false))
            .arg(ArgType::bool("bold", true).description("How the example starts."))
            .arg(ArgType::bool("italic", false).description("How the example starts."))
            .arg(ArgType::bool("open", true).description("How the example starts."))
            .story("Example", Args::new())
            .story("Italic", Args::new().with("italic", true))
    }
}
