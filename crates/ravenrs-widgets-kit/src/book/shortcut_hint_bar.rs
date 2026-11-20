//! The stories of `ShortcutHintBar`, from the component gallery's example, by scripts/rcb-scaffold.py.
//! Reviewed for #761: it is no longer written over.

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

/// The gallery's example of `ShortcutHintBar`, with its settings taken from the story.
#[allow(unused_variables, unused_mut, clippy::redundant_clone, clippy::let_and_return, clippy::clone_on_copy)]
fn render(args: &Args) -> AnyView {
    Box::new({
        let arg_leading = args.text("leading");

        {
            let mut it =
                ShortcutHintBar::new().hint(["←", "→"], "move").hint(["1", "5"], "rate").hint(["P"], "pick");
            it = it.leading(arg_leading.clone());
            it
        }
    })
}

impl ComponentBook for ShortcutHintBar {
    fn book() -> Meta {
        Meta::new("Display and feedback/ShortcutHintBar", render)
            .summary("Line of key hints under what they act on.")
            .arg(ArgType::text("leading", "3 of 40"))
            .story("Example", Args::new())
    }
}
