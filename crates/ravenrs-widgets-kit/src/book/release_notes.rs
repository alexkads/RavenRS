//! The stories of `ReleaseNotes`, from the component gallery's example, by scripts/rcb-scaffold.py.
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
use std::collections::{BTreeMap, BTreeSet, HashMap};
#[allow(unused_imports)]
use std::rc::Rc;

/// The gallery's example of `ReleaseNotes`, with its settings taken from the story.
#[allow(unused_variables, unused_mut, clippy::redundant_clone, clippy::let_and_return, clippy::clone_on_copy)]
fn render(args: &Args) -> AnyView {
    Box::new({
        let arg_height = args.number("height") as f32;

        let selected = State::new(args.number("selected") as _);
        {
            let mut it = ReleaseNotes::new(
                vec![
                    ReleaseNote::new("Version 27.2", "Status bars and release notes")
                        .date("Nov 1, 2026")
                        .badge("New", Tone::Success)
                        .changes(["A line at the foot of the window", "What each version brought"])
                        .section(
                            "Why update",
                            "Tell people about new versions without getting in their way.",
                        ),
                    ReleaseNote::new("Version 27.1", "The first of the year")
                        .badge("Installed", Tone::Neutral),
                ],
                selected.binding(),
            );
            it = it.height(arg_height);
            it
        }
    })
}

impl ComponentBook for ReleaseNotes {
    fn book() -> Meta {
        Meta::new("Forms/ReleaseNotes", render)
            .summary("An app's versions and what each one brought.")
            .arg(ArgType::number("height", 460.0, 0.0, 1840.0, 1.0))
            .arg(ArgType::number("selected", 0.0, 0.0, 5.0, 1.0).description("How the example starts."))
            .story("Example", Args::new())
    }
}
