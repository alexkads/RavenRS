//! The stories of `TitleBar`, from the component gallery's example, by scripts/rcb-scaffold.py.
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

/// The gallery's example of `TitleBar`, with its settings taken from the story.
#[allow(unused_variables, unused_mut, clippy::redundant_clone, clippy::let_and_return, clippy::clone_on_copy)]
fn render(args: &Args) -> AnyView {
    Box::new({
        let arg_active = args.bool("active");
        let arg_maximized = args.bool("maximized");

        let maximized = State::new(args.bool("maximized_state"));
        view_fn(move || {
            let toggle = maximized.clone();
            {
                let mut it = TitleBar::new("Report.pdf")
                    .leading(Icon::new(IconName::File).size(16.0))
                    .action(IconButton::new(IconName::Search).label("Search").size(ButtonSize::Mini))
                    .maximized(maximized.get())
                    .on_minimize(|| {})
                    .on_maximize(move || toggle.update(|on| *on = !*on))
                    .on_close(|| {});
                it = it.active(arg_active);
                it = it.maximized(arg_maximized);
                it
            }
        })
    })
}

impl ComponentBook for TitleBar {
    fn book() -> Meta {
        Meta::new("Actions and inputs/TitleBar", render)
            .summary("Title, actions and window buttons drawn by the app; does not move the window.")
            .arg(ArgType::bool("active", true))
            .arg(ArgType::bool("maximized", false))
            .arg(ArgType::bool("maximized_state", false).description("How the example starts."))
            .story("Example", Args::new())
            .story("Maximized state", Args::new().with("maximized_state", true))
    }
}
