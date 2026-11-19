//! The stories of `StatusBar`, from the component gallery's example, by scripts/rcb-scaffold.py.
//! Reviewed for #756: it is no longer written over.

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

/// The gallery's example of `StatusBar`, with its settings taken from the story.
#[allow(unused_variables, unused_mut, clippy::redundant_clone, clippy::let_and_return, clippy::clone_on_copy)]
fn render(args: &Args) -> AnyView {
    Box::new({
        let arg_center_tone = match args.choice("center_tone") {
            "Neutral" => Tone::Neutral,
            "Accent" => Tone::Accent,
            "Success" => Tone::Success,
            "Danger" => Tone::Danger,
            _ => Tone::Warning,
        };

        {
            let mut it = StatusBar::new()
                .left((StatusBar::action("Version 27.1"),))
                .center((
                    StatusBar::text("Version 27.2 is available").truncate().grow(1.0),
                    StatusBar::action("What's new"),
                    StatusBar::action("Update").style(ButtonStyle::Primary),
                ))
                .right((StatusBar::text("12 items"),));
            it = it.center_tone(arg_center_tone.clone());
            it
        }
    })
}

impl ComponentBook for StatusBar {
    fn book() -> Meta {
        Meta::new("Forms/StatusBar", render)
            .summary("One fixed line at the foot of an app window.")
            .arg(ArgType::choice(
                "center_tone",
                &["Neutral", "Accent", "Success", "Warning", "Danger"],
                "Warning",
            ))
            .story("Example", Args::new())
            .story("Center tone: Neutral", Args::new().with("center_tone", ArgValue::choice("Neutral")))
            .story("Center tone: Accent", Args::new().with("center_tone", ArgValue::choice("Accent")))
            .story("Center tone: Success", Args::new().with("center_tone", ArgValue::choice("Success")))
            .story("Center tone: Danger", Args::new().with("center_tone", ArgValue::choice("Danger")))
    }
}
