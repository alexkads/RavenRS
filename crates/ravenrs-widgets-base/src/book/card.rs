//! The stories of `Card`.

use ravenrs_style::{Semantic, TextStyle};

use super::{ArgType, Args, ComponentBook, Meta};
use crate::{AnyView, Card, Text, VStack};

fn card(args: &Args) -> AnyView {
    let content = VStack::new((
        Text::new(args.text("title")).style(TextStyle::Headline),
        Text::new(args.text("text")).foreground(Semantic::InkMuted),
    ))
    .spacing(4.0);
    let mut card = Card::new(content).selected(args.bool("selected")).padding(args.number("padding") as f32);
    if args.bool("no_shadow") {
        card = card.no_shadow();
    }
    Box::new(card)
}

impl ComponentBook for Card<AnyView> {
    fn book() -> Meta {
        Meta::new("Layout/Card", card)
            .summary("A grouped surface lifted off the page by its fill and a soft shadow.")
            .arg(ArgType::text("title", "Quarterly report"))
            .arg(ArgType::text("text", "A grouped surface, lifted off the page."))
            .arg(
                ArgType::number("padding", 24.0, 0.0, 48.0, 4.0)
                    .description("Inside space, in points; space-6 by default."),
            )
            .arg(ArgType::bool("selected", false).description("The chosen card of a group."))
            .arg(ArgType::bool("no_shadow", false).description("Flat, for a card inside another surface."))
            .story("Default", Args::new())
            .story("Selected", Args::new().with("selected", true))
            .story("Flat", Args::new().with("no_shadow", true))
    }
}
