//! The stories of `SplashScreen`. Written by hand: the gallery's example pretends to load inside
//! a view of its own, which the scaffold does not take apart.

use ravenrs_reactive::State;

use crate::book::{ArgType, Args, ComponentBook, Meta};
use crate::{AnyView, IconName, SplashScreen};

fn render(args: &Args) -> AnyView {
    let visible = State::new(true);
    let mut splash = SplashScreen::new(visible.binding(), args.text("name"))
        .icon(IconName::Wallet)
        .status(args.text("status"))
        .reveal_after(0.0);
    if args.bool("progress") {
        splash = splash.progress(args.number("value") as f32);
    }
    Box::new(splash)
}

impl ComponentBook for SplashScreen {
    fn book() -> Meta {
        Meta::new("Overlays/SplashScreen", render)
            .summary("The screen an app shows while it opens: its name, an icon and how far it got.")
            .arg(ArgType::text("name", "Raven Finance"))
            .arg(ArgType::text("status", "Opening the database").description("What it is doing now."))
            .arg(
                ArgType::bool("progress", false)
                    .description("A bar in place of the spinner, once there is something to measure."),
            )
            .arg(ArgType::number("value", 0.6, 0.0, 1.0, 0.05))
            .story("Spinner", Args::new())
            .story("Progress", Args::new().with("progress", true).with("status", "Loading your accounts"))
    }
}
