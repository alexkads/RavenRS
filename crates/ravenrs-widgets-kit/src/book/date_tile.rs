//! The stories of `DateTile`: a day in a small block, in its tones and sizes.

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

/// The gallery's example of `DateTile`, with its settings taken from the story.
#[allow(unused_variables, unused_mut, clippy::redundant_clone, clippy::let_and_return, clippy::clone_on_copy)]
fn render(args: &Args) -> AnyView {
    let day = Date::new(2026, 11, 14).unwrap_or_else(|| unreachable!("a real date"));
    let tone = match args.choice("tone") {
        "Accent" => Tone::Accent,
        "Success" => Tone::Success,
        "Warning" => Tone::Warning,
        "Danger" => Tone::Danger,
        _ => Tone::Neutral,
    };
    let size = args.number("size") as f32;
    let mut tile = DateTile::new(day).tone(tone).size(size);
    if args.bool("outlined") {
        tile = tile.outlined();
    }
    Box::new(
        HStack::new((
            tile,
            DateTile::new(day.add_days(1)).tone(Tone::Accent),
            DateTile::new(day.add_days(7)).size(72.0),
        ))
        .spacing(12.0)
        .align(Align::Center),
    )
}

impl ComponentBook for DateTile {
    fn book() -> Meta {
        Meta::new("Display and feedback/DateTile", render)
            .summary("A day in a small block: the day over the month, and the whole date for screen readers.")
            .arg(ArgType::choice("tone", &["Neutral", "Accent", "Success", "Warning", "Danger"], "Neutral"))
            .arg(ArgType::number("size", 56.0, 32.0, 128.0, 4.0))
            .arg(ArgType::bool("outlined", false).description("Only the outline, over a colored band."))
            .story("Example", Args::new())
            .story("Tones", Args::new().with("tone", ArgValue::choice("Accent")))
            .story("Tone: Success", Args::new().with("tone", ArgValue::choice("Success")))
            .story("Tone: Warning", Args::new().with("tone", ArgValue::choice("Warning")))
            .story("Tone: Danger", Args::new().with("tone", ArgValue::choice("Danger")))
            .story("Sizes", Args::new().with("size", 72.0))
            .story("Outlined", Args::new().with("outlined", true))
    }
}
