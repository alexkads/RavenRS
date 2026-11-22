//! The stories of `CommandPalette`, from the component gallery's example, by scripts/rcb-scaffold.py.
//! Reviewed for #758: it is no longer written over.

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

/// The gallery's example of `CommandPalette`, with its settings taken from the story.
#[allow(unused_variables, unused_mut, clippy::redundant_clone, clippy::let_and_return, clippy::clone_on_copy)]
fn render(args: &Args) -> AnyView {
    Box::new({
        let arg_placeholder = args.text("placeholder");

        let open = State::new(args.bool("open"));
        let show = open.clone();
        VStack::new((Button::new("Open palette").icon(IconName::Command).on_click(move || show.set(true)), {
            let mut it = CommandPalette::new(open.binding())
                .command(Command::new("Go to reports", || {}).icon(IconName::FileText).group("Pages"))
                .command(Command::new("Open settings", || {}).icon(IconName::Settings).group("Pages"));
            if !arg_placeholder.is_empty() {
                it = it.placeholder(arg_placeholder.clone());
            }
            it
        }))
    })
}

impl ComponentBook for CommandPalette {
    fn book() -> Meta {
        Meta::new("Overlays/CommandPalette", render)
            .summary("Searchable list of commands, opened with Cmd/Ctrl+K.")
            .arg(ArgType::text("placeholder", "").description("Empty leaves it as the component has it."))
            .arg(ArgType::bool("open", false).description("How the example starts."))
            .story("Example", Args::new())
            .story("Open", Args::new().with("open", true))
    }
}
