//! The stories of `Section`, from the component gallery's example, by scripts/rcb-scaffold.py.
//! Reviewed for #760: it is no longer written over.

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

/// The gallery's example of `Section`, with its settings taken from the story.
#[allow(unused_variables, unused_mut, clippy::redundant_clone, clippy::let_and_return, clippy::clone_on_copy)]
fn render(args: &Args) -> AnyView {
    Box::new({
        let arg_description = args.text("description");

        {
            let mut it = Section::new(
                "Notifications",
                (Text::new("Grouped content under a title.").foreground(Semantic::InkMuted),),
            )
            .action(Button::new("Edit").style(ButtonStyle::Secondary).size(ButtonSize::Small));
            if !arg_description.is_empty() {
                it = it.description(arg_description.clone());
            }
            it
        }
    })
}

impl ComponentBook for Section<(AnyView,)> {
    fn book() -> Meta {
        Meta::new("Layout/Section", render)
            .summary("Titled group of content with an optional description.")
            .arg(ArgType::text("description", "").description("Empty leaves it as the component has it."))
            .story("Example", Args::new())
            .story("With description", Args::new().with("description", "Who can see and change it."))
    }
}
