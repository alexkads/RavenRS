//! The stories of `Dialog`, from the component gallery's example, by scripts/rcb-scaffold.py.
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

/// The gallery's example of `Dialog`, with its settings taken from the story.
#[allow(unused_variables, unused_mut, clippy::redundant_clone, clippy::let_and_return, clippy::clone_on_copy)]
fn render(args: &Args) -> AnyView {
    Box::new({
        let arg_description = args.text("description");
        let arg_close_button = args.bool("close_button");
        let arg_close_label = args.text("close_label");
        let arg_close_on_backdrop = args.bool("close_on_backdrop");
        let arg_close_on_escape = args.bool("close_on_escape");
        let arg_width = args.number("width") as f32;

        let open = State::new(args.bool("open"));
        let (show, cancel, confirm) = (open.clone(), open.clone(), open.clone());
        VStack::new((
            Button::new("Delete project").style(ButtonStyle::Danger).on_click(move || show.set(true)),
            {
                let mut it = Dialog::new(open.binding(), "Delete the project?")
                    .action(
                        Button::new("Cancel")
                            .style(ButtonStyle::Secondary)
                            .on_click(move || cancel.set(false)),
                    )
                    .action(
                        Button::new("Delete").style(ButtonStyle::Danger).on_click(move || confirm.set(false)),
                    );
                it = it.description(arg_description.clone());
                it = it.close_button(arg_close_button);
                if !arg_close_label.is_empty() {
                    it = it.close_label(arg_close_label.clone());
                }
                it = it.close_on_backdrop(arg_close_on_backdrop);
                it = it.close_on_escape(arg_close_on_escape);
                it = it.width(arg_width);
                it
            },
        ))
    })
}

impl ComponentBook for Dialog {
    fn book() -> Meta {
        Meta::new("Overlays/Dialog", render)
            .summary("Modal window over a dimmed backdrop.")
            .arg(ArgType::text("description", "This cannot be undone."))
            .arg(ArgType::bool("close_button", true))
            .arg(ArgType::text("close_label", "").description("Empty leaves it as the component has it."))
            .arg(ArgType::bool("close_on_backdrop", true))
            .arg(ArgType::bool("close_on_escape", true))
            .arg(ArgType::number("width", 460.0, 0.0, 1840.0, 1.0))
            .arg(ArgType::bool("open", false).description("How the example starts."))
            .story("Example", Args::new())
            .story("Open", Args::new().with("open", true))
            .story("Open, no close button", Args::new().with("open", true).with("close_button", false))
    }
}
