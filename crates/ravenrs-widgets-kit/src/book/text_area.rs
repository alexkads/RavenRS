//! The stories of `TextArea`, from the component gallery's example, by scripts/rcb-scaffold.py.
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

/// The gallery's example of `TextArea`, with its settings taken from the story.
#[allow(unused_variables, unused_mut, clippy::redundant_clone, clippy::let_and_return, clippy::clone_on_copy)]
fn render(args: &Args) -> AnyView {
    Box::new({
        let arg_placeholder = args.text("placeholder");
        let arg_disabled = args.bool("disabled");
        let arg_read_only = args.bool("read_only");

        let notes = State::new(String::from("Two lines\nto start with."));
        // Grows from 2 to 6 lines as you type; `.rows(4)` would fix the height.
        {
            // Grows with what is typed, or keeps a fixed number of lines.
            let mut it = TextArea::new(notes.binding());
            it = if args.bool("fixed") { it.rows(args.number("rows") as usize) } else { it.grows(2, 6) };
            it = it.placeholder(arg_placeholder.clone());
            it = it.disabled(arg_disabled);
            it = it.read_only(arg_read_only);
            it
        }
    })
}

impl ComponentBook for TextArea {
    fn book() -> Meta {
        Meta::new("Actions and inputs/TextArea", render)
            .summary("Text on several lines, scrolling past its height or growing with the text.")
            .arg(
                ArgType::bool("fixed", false)
                    .description("A fixed height in lines; off, it grows from 2 to 6 lines as you type."),
            )
            .arg(
                ArgType::number("rows", 4.0, 1.0, 20.0, 1.0)
                    .description("The lines of a fixed height; 4 by default."),
            )
            .arg(ArgType::text("placeholder", "Notes"))
            .arg(ArgType::bool("disabled", false))
            .arg(ArgType::bool("read_only", false))
            .story("Example", Args::new())
            .story("Fixed height", Args::new().with("fixed", true))
            .story("Disabled", Args::new().with("disabled", true))
            .story("Read only", Args::new().with("read_only", true))
    }
}
