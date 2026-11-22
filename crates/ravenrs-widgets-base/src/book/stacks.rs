//! The stories of `HStack` and `VStack`, the stacks every screen is laid out with: how far apart
//! their children sit and how they line up are what a design measures first.

use ravenrs_layout::{Align, Justify};
use ravenrs_style::Semantic;

use crate::book::{ArgType, ArgValue, Args, ComponentBook, Meta};
use crate::{AnyView, Badge, Button, ButtonStyle, HStack, Text, VStack};

const ALIGNS: &[&str] = &["Start", "Center", "End", "Stretch"];
const JUSTIFIES: &[&str] = &["Start", "Center", "End", "SpaceBetween"];

fn align(name: &str) -> Align {
    match name {
        "Center" => Align::Center,
        "End" => Align::End,
        "Stretch" => Align::Stretch,
        _ => Align::Start,
    }
}

fn justify(name: &str) -> Justify {
    match name {
        "Center" => Justify::Center,
        "End" => Justify::End,
        "SpaceBetween" => Justify::SpaceBetween,
        _ => Justify::Start,
    }
}

fn children() -> (Button, Button, Badge) {
    (Button::new("Save"), Button::new("Cancel").style(ButtonStyle::Secondary), Badge::new("Draft").outline())
}

fn hstack(args: &Args) -> AnyView {
    let mut stack = HStack::new(children())
        .spacing(args.number("spacing") as f32)
        .align(align(args.choice("align")))
        .justify(justify(args.choice("justify")))
        .width(args.number("width") as f32);
    if args.bool("wrap") {
        stack = stack.wrap();
    }
    Box::new(stack)
}

fn vstack(args: &Args) -> AnyView {
    Box::new(
        VStack::new((
            Text::new("Quarterly report"),
            Text::new("A grouped list of what changed.").foreground(Semantic::InkMuted),
            Button::new("Open").style(ButtonStyle::Secondary),
        ))
        .spacing(args.number("spacing") as f32)
        .align(align(args.choice("align")))
        .width(args.number("width") as f32),
    )
}

impl ComponentBook for HStack<(Button, Button, Badge)> {
    fn book() -> Meta {
        Meta::new("Layout/HStack", hstack)
            .summary("Lays its children out in a row, a set space apart.")
            .arg(
                ArgType::number("spacing", 16.0, 0.0, 48.0, 4.0)
                    .description("space-4 by default; the 4px grid."),
            )
            .arg(ArgType::choice("align", ALIGNS, "Start").description("Across the row."))
            .arg(ArgType::choice("justify", JUSTIFIES, "Start").description("Along the row."))
            .arg(ArgType::number("width", 520.0, 200.0, 760.0, 10.0))
            .arg(ArgType::bool("wrap", false).description("Children that do not fit go to the next line."))
            .story("Row", Args::new())
            .story("Tight", Args::new().with("spacing", 8.0))
            .story("Space between", Args::new().with("justify", ArgValue::choice("SpaceBetween")))
            .story(
                "Centered",
                Args::new()
                    .with("align", ArgValue::choice("Center"))
                    .with("justify", ArgValue::choice("Center")),
            )
    }
}

impl ComponentBook for VStack<(Text, Text, Button)> {
    fn book() -> Meta {
        Meta::new("Layout/VStack", vstack)
            .summary("Lays its children out in a column, a set space apart.")
            .arg(
                ArgType::number("spacing", 16.0, 0.0, 48.0, 4.0)
                    .description("space-4 by default; the 4px grid."),
            )
            .arg(ArgType::choice("align", ALIGNS, "Start").description("Across the column."))
            .arg(ArgType::number("width", 360.0, 160.0, 760.0, 10.0))
            .story("Column", Args::new())
            .story("Tight", Args::new().with("spacing", 4.0))
            .story("Stretched", Args::new().with("align", ArgValue::choice("Stretch")))
    }
}
