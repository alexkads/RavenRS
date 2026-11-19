//! The stories of `SelectionBar`, from the component gallery's example, by scripts/rcb-scaffold.py.
//! Reviewed for #756: it is no longer written over.

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
use ravenrs_widgets_kit::*;
#[allow(unused_imports)]
use std::collections::{BTreeMap, BTreeSet, HashMap};
#[allow(unused_imports)]
use std::rc::Rc;

/// The gallery's example of `SelectionBar`, with its settings taken from the story.
#[allow(unused_variables, unused_mut, clippy::redundant_clone, clippy::let_and_return, clippy::clone_on_copy)]
fn render(args: &Args) -> AnyView {
    Box::new({
        let arg_hint = args.text("hint");

        {
            let mut it = SelectionBar::new(3)
                .on_exit(|| {})
                .action(ToolbarAction::new("Copy").icon(IconName::Copy).icon_only().on_click(|| {}))
                .action(ToolbarAction::new("Favorite").icon(IconName::Star).icon_only().on_click(|| {}));
            if !arg_hint.is_empty() {
                it = it.hint(arg_hint.clone());
            }
            it
        }
        .width(420.0)
    })
}

impl ComponentBook for SelectionBar {
    fn book() -> Meta {
        Meta::new("Forms/SelectionBar", render)
            .summary("How many are chosen, a way out and what to do with them, with Escape to leave.")
            .arg(ArgType::text("hint", "").description("Empty leaves it as the component has it."))
            .story("Example", Args::new())
    }
}
