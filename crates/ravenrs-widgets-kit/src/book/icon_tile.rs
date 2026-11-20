//! The stories of `IconTile`, from the component gallery's example, by scripts/rcb-scaffold.py.
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

/// The gallery's example of `IconTile`, with its settings taken from the story.
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
        let arg_label = args.text("label");
        let arg_size = args.number("size") as f32;

        HStack::new((
            {
                let mut it = IconTile::new(IconName::CircleCheck);
                it = it.tone(arg_tone.clone());
                if !arg_label.is_empty() {
                    it = it.label(arg_label.clone());
                }
                it = it.size(arg_size);
                it
            },
            IconTile::new(IconName::TriangleAlert).tone(Tone::Warning),
            IconTile::new(IconName::Image).size(32.0),
        ))
        .spacing(12.0)
        .align(Align::Center)
    })
}

impl ComponentBook for IconTile {
    fn book() -> Meta {
        Meta::new("Display and feedback/IconTile", render)
            .summary("An icon in a square tinted with its color.")
            .arg(ArgType::choice("tone", &["Neutral", "Accent", "Success", "Warning", "Danger"], "Success"))
            .arg(ArgType::text("label", "").description("Empty leaves it as the component has it."))
            .arg(ArgType::number("size", 48.0, 0.0, 192.0, 1.0))
            .story("Example", Args::new())
            .story("Small", Args::new().with("size", 32.0))
            .story("Tone: Neutral", Args::new().with("tone", ArgValue::choice("Neutral")))
            .story("Tone: Accent", Args::new().with("tone", ArgValue::choice("Accent")))
            .story("Tone: Warning", Args::new().with("tone", ArgValue::choice("Warning")))
            .story("Tone: Danger", Args::new().with("tone", ArgValue::choice("Danger")))
    }
}
