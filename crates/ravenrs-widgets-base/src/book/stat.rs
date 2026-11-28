//! The stories of `Stat`, from the component gallery's example, by scripts/rcb-scaffold.py.
//! Reviewed for #759: it is no longer written over.

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

/// A `Stat` with the label, the value, the change and the caption the story gives (#921: a
/// screen sets its own words, as the boards of the Raven CI do).
fn render(args: &Args) -> AnyView {
    let mut it = Stat::new(args.text("label"), args.text("value"));
    let delta = args.text("delta");
    if !delta.is_empty() {
        let (up, good) = match args.choice("delta_direction") {
            "Down" => (false, false),
            "Up, bad" => (true, false),
            "Down, good" => (false, true),
            _ => (true, true),
        };
        it = it.delta_toned(delta, up, good);
    }
    let caption = args.text("caption");
    if !caption.is_empty() {
        it = it.caption(caption);
    }
    if args.bool("large") {
        it = it.large();
    }
    it = it.value_tone(match args.choice("value_tone") {
        "Accent" => Tone::Accent,
        "Success" => Tone::Success,
        "Warning" => Tone::Warning,
        "Danger" => Tone::Danger,
        _ => Tone::Neutral,
    });
    Box::new(it)
}

impl ComponentBook for Stat {
    fn book() -> Meta {
        Meta::new("Data/Stat", render)
            .summary("Headline number with a label and a change indicator.")
            .arg(ArgType::text("label", "Revenue"))
            .arg(ArgType::text("value", "$128,450"))
            .arg(ArgType::text("delta", "+12.4%").description("Empty shows no change."))
            .arg(ArgType::choice("delta_direction", &["Up", "Down", "Up, bad", "Down, good"], "Up"))
            .arg(ArgType::text("caption", "").description("A muted line under the value; empty shows none."))
            .arg(ArgType::bool("large", false))
            .arg(
                ArgType::choice(
                    "value_tone",
                    &["Neutral", "Accent", "Success", "Warning", "Danger"],
                    "Neutral",
                )
                .description("The value in a tone, for a total whose color says what it is."),
            )
            .story("Example", Args::new())
            .story("Large", Args::new().with("large", true))
            .story(
                "Label and value",
                Args::new().with("label", "IN THE QUEUE").with("value", "2").with("delta", ""),
            )
            .story(
                "With a caption",
                Args::new()
                    .with("label", "IN THE QUEUE")
                    .with("value", "2")
                    .with("delta", "")
                    .with("caption", "The oldest waits 40 s"),
            )
            .story(
                "Delta: Down",
                Args::new().with("delta", "-3.1%").with("delta_direction", ArgValue::choice("Down")),
            )
            .story(
                "Delta: Up, bad",
                Args::new()
                    .with("label", "Churn")
                    .with("value", "1.8%")
                    .with("delta", "+0.4%")
                    .with("delta_direction", ArgValue::choice("Up, bad")),
            )
            .story(
                "Delta: Down, good",
                Args::new()
                    .with("label", "Churn")
                    .with("value", "1.8%")
                    .with("delta", "-0.4%")
                    .with("delta_direction", ArgValue::choice("Down, good")),
            )
            .story(
                "Value tone: Accent",
                Args::new().with("delta", "").with("value_tone", ArgValue::choice("Accent")),
            )
            .story(
                "Value tone: Success",
                Args::new().with("delta", "").with("value_tone", ArgValue::choice("Success")),
            )
            .story(
                "Value tone: Warning",
                Args::new().with("delta", "").with("value_tone", ArgValue::choice("Warning")),
            )
            .story(
                "Value tone: Danger",
                Args::new().with("delta", "").with("value_tone", ArgValue::choice("Danger")),
            )
    }
}
