//! The stories of `HeroBanner`, from the component gallery's example, by scripts/rcb-scaffold.py.
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

/// The gallery's example of `HeroBanner`, with its settings taken from the story.
#[allow(unused_variables, unused_mut, clippy::redundant_clone, clippy::let_and_return, clippy::clone_on_copy)]
fn render(args: &Args) -> AnyView {
    Box::new({
        let arg_tone = match args.choice("tone") {
            "Neutral" => Tone::Neutral,
            "Accent" => Tone::Accent,
            "Warning" => Tone::Warning,
            "Danger" => Tone::Danger,
            _ => Tone::Success,
        };
        let arg_description = args.text("description");

        VStack::new((
            {
                let mut it = HeroBanner::new("You can remove the card").icon(IconName::CircleCheck);
                it = it.tone(arg_tone.clone());
                if !arg_description.is_empty() {
                    it = it.description(arg_description.clone());
                }
                it
            },
            HeroBanner::new("Do not remove the card").icon(IconName::CircleAlert).tone(Tone::Danger),
        ))
        .spacing(12.0)
        .align(Align::Stretch)
    })
}

impl ComponentBook for HeroBanner {
    fn book() -> Meta {
        Meta::new("Display and feedback/HeroBanner", render)
            .summary("Wide band in a strong color, readable from across the room.")
            .arg(ArgType::choice("tone", &["Neutral", "Accent", "Success", "Warning", "Danger"], "Success"))
            .arg(ArgType::text("description", "").description("Empty leaves it as the component has it."))
            .story("Example", Args::new())
            .story(
                "With description",
                Args::new().with("description", "Wait for the beep before you take it out."),
            )
            .story("Tone: Neutral", Args::new().with("tone", ArgValue::choice("Neutral")))
            .story("Tone: Accent", Args::new().with("tone", ArgValue::choice("Accent")))
            .story("Tone: Warning", Args::new().with("tone", ArgValue::choice("Warning")))
            .story("Tone: Danger", Args::new().with("tone", ArgValue::choice("Danger")))
    }
}
