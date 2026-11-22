//! The stories of `CountUp`, from the component gallery's example, by scripts/rcb-scaffold.py.
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

/// The gallery's example of `CountUp`, with its settings taken from the story.
#[allow(unused_variables, unused_mut, clippy::redundant_clone, clippy::let_and_return, clippy::clone_on_copy)]
fn render(args: &Args) -> AnyView {
    Box::new({
        let arg_seconds = args.number("seconds");
        let arg_start_at_value = args.bool("start_at_value");

        HStack::new((
            VStack::new((Text::new("ROWS").style(TextStyle::Caption).foreground(Semantic::InkMuted), {
                let mut it = CountUp::new(100_000.0);
                it = it.seconds(arg_seconds);
                if arg_start_at_value {
                    it = it.start_at_value();
                }
                it
            }))
            .spacing(4.0),
            VStack::new((
                Text::new("COMPONENTS").style(TextStyle::Caption).foreground(Semantic::InkMuted),
                CountUp::new(160.0).format(|v| format!("{v:.0}+")),
            ))
            .spacing(4.0),
        ))
        .spacing(48.0)
    })
}

impl ComponentBook for CountUp {
    fn book() -> Meta {
        Meta::new("Layout/CountUp", render)
            .summary("A number that counts up when it first shows on screen.")
            .arg(ArgType::number("seconds", 1.5, 0.0, 6.0, 0.1))
            .arg(ArgType::bool("start_at_value", false))
            .story("Example", Args::new())
            .story("At its value", Args::new().with("start_at_value", true))
    }
}
