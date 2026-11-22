//! The stories of `Dot`, from the component gallery's example, by scripts/rcb-scaffold.py.
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

/// The gallery's example of `Dot`, with its settings taken from the story.
#[allow(unused_variables, unused_mut, clippy::redundant_clone, clippy::let_and_return, clippy::clone_on_copy)]
fn render(args: &Args) -> AnyView {
    Box::new({
        let arg_tone = match args.choice("tone") {
            "Neutral" => Tone::Neutral,
            "Accent" => Tone::Accent,
            "Success" => Tone::Success,
            "Danger" => Tone::Danger,
            _ => Tone::Warning,
        };
        let arg_label = args.text("label");

        HStack::new((
            {
                let mut it = Dot::new();
                it = it.tone(arg_tone.clone());
                it = it.label(arg_label.clone());
                it
            },
            Text::new("Not saved").style(TextStyle::Callout),
            Dot::new().tone(Tone::Success).size(10.0).label("Online"),
            Text::new("Online").style(TextStyle::Callout),
        ))
        .spacing(8.0)
        .align(Align::Center)
    })
}

impl ComponentBook for Dot {
    fn book() -> Meta {
        Meta::new("Display and feedback/Dot", render)
            .summary("Small round mark of state, such as not saved.")
            .arg(ArgType::choice("tone", &["Neutral", "Accent", "Success", "Warning", "Danger"], "Warning"))
            .arg(ArgType::text("label", "Not saved"))
            .story("Example", Args::new())
            .story("Tone: Neutral", Args::new().with("tone", ArgValue::choice("Neutral")))
            .story("Tone: Accent", Args::new().with("tone", ArgValue::choice("Accent")))
            .story("Tone: Success", Args::new().with("tone", ArgValue::choice("Success")))
            .story("Tone: Danger", Args::new().with("tone", ArgValue::choice("Danger")))
    }
}
