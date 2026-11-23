//! The stories of `Badge`, `Alert`, `Avatar` and `ProgressBar`.

use ravenrs_layout::Align;

use super::button::{ICONS, icon};
use super::{ArgType, ArgValue, Args, ComponentBook, Meta};
use crate::{Alert, AnyView, Avatar, Badge, ProgressBar, Tone, VStack};

const TONES: &[&str] = &["Neutral", "Accent", "Success", "Warning", "Danger"];

fn tone(name: &str) -> Tone {
    match name {
        "Accent" => Tone::Accent,
        "Success" => Tone::Success,
        "Warning" => Tone::Warning,
        "Danger" => Tone::Danger,
        _ => Tone::Neutral,
    }
}

fn badge(args: &Args) -> AnyView {
    let mut badge = Badge::new(args.text("text")).tone(tone(args.choice("tone")));
    if let Some(icon) = icon(args.choice("icon")) {
        badge = badge.icon(icon);
    }
    if args.bool("outline") {
        badge = badge.outline();
    }
    Box::new(badge)
}

fn alert(args: &Args) -> AnyView {
    let mut alert = if args.text("title").is_empty() {
        Alert::message(args.text("message"))
    } else {
        Alert::new(args.text("title"), args.text("message"))
    }
    .tone(tone(args.choice("tone")));
    if args.bool("compact") {
        alert = alert.compact();
    }
    let action = args.text("action");
    if !action.is_empty() {
        let pressed = ravenrs_reactive::State::new(0);
        return Box::new(alert.action(action, move || pressed.update(|n| *n += 1)));
    }
    Box::new(alert)
}

fn avatar(args: &Args) -> AnyView {
    Box::new(Avatar::new(args.text("name")).size(args.number("size") as f32))
}

fn progress(args: &Args) -> AnyView {
    let mut bar = ProgressBar::new(args.number("value") as f32).tone(tone(args.choice("tone")));
    if !args.text("label").is_empty() {
        bar = bar.label(args.text("label"));
    }
    if args.bool("percent") {
        bar = bar.percent();
    }
    if !args.text("detail").is_empty() {
        bar = bar.detail(args.text("detail"));
    }
    bar = bar.detail_inline(args.bool("detail_inline"));
    Box::new(bar)
}

impl ComponentBook for Badge {
    fn book() -> Meta {
        Meta::new("Display and feedback/Badge", badge)
            .summary("Short label for a state or a count, in a tone that says what it means.")
            .arg(ArgType::text("text", "New"))
            .arg(ArgType::choice("tone", TONES, "Accent"))
            .arg(ArgType::choice("icon", ICONS, "none"))
            .arg(ArgType::bool("outline", false).description("Only the edge, for a draft or a quiet tag."))
            .story("Accent", Args::new())
            .story("Neutral", Args::new().with("tone", ArgValue::choice("Neutral")).with("text", "Neutral"))
            .story(
                "Success with icon",
                Args::new()
                    .with("tone", ArgValue::choice("Success"))
                    .with("text", "Paid")
                    .with("icon", ArgValue::choice("check")),
            )
            .story("Danger", Args::new().with("tone", ArgValue::choice("Danger")).with("text", "Overdue"))
            .story(
                "Outline",
                Args::new()
                    .with("tone", ArgValue::choice("Neutral"))
                    .with("text", "Draft")
                    .with("outline", true),
            )
            .story("Warning", Args::new().with("tone", ArgValue::choice("Warning")).with("text", "Pending"))
    }
}

impl ComponentBook for Alert {
    fn book() -> Meta {
        Meta::new("Display and feedback/Alert", alert)
            .summary("A message about the page, with a tone, an icon and an optional title.")
            .arg(
                ArgType::text("title", "Deployment finished")
                    .description("Leave it empty when it would only repeat where it is."),
            )
            .arg(ArgType::text("message", "The site is live."))
            .arg(ArgType::choice("tone", TONES, "Success"))
            .arg(ArgType::bool("compact", false))
            .arg(
                ArgType::text("action", "")
                    .description("A button at the end that says what to do; empty shows none."),
            )
            .story("Success", Args::new())
            .story(
                "Danger",
                Args::new()
                    .with("title", "Payment failed")
                    .with("message", "Update your card.")
                    .with("tone", ArgValue::choice("Danger")),
            )
            .story(
                "With an action",
                Args::new()
                    .with("title", "1 part not converted")
                    .with("message", "Line 13 runs free Groovy, which the server does not run.")
                    .with("tone", ArgValue::choice("Danger"))
                    .with("action", "Go to line 13"),
            )
            .story(
                "Message only",
                Args::new()
                    .with("title", "")
                    .with("message", "Moving deletes the originals.")
                    .with("tone", ArgValue::choice("Warning"))
                    .with("compact", true),
            )
            .story_with("All tones", Args::new(), all_alert_tones)
    }
}

impl ComponentBook for Avatar {
    fn book() -> Meta {
        Meta::new("Display and feedback/Avatar", avatar)
            .summary("A person's initials or picture in a circle.")
            .arg(ArgType::text("name", "Ana Lima").description("The initials come from it."))
            .arg(ArgType::number("size", 32.0, 20.0, 96.0, 4.0))
            .story("Default", Args::new())
            .story("Large", Args::new().with("name", "Carla Dias").with("size", 44.0))
    }
}

impl ComponentBook for ProgressBar {
    fn book() -> Meta {
        Meta::new("Display and feedback/ProgressBar", progress)
            .summary("How far a task is, with an optional label, percent and detail.")
            .arg(ArgType::number("value", 0.4, 0.0, 1.0, 0.01))
            .arg(ArgType::choice("tone", TONES, "Accent"))
            .arg(ArgType::text("label", ""))
            .arg(ArgType::bool("percent", false))
            .arg(ArgType::text("detail", ""))
            .arg(
                ArgType::bool("detail_inline", false)
                    .description("The detail on the line of the label, before the percent."),
            )
            .story("Plain", Args::new().with("value", 0.25))
            .story("Success", Args::new().with("value", 0.9).with("tone", ArgValue::choice("Success")))
            .story(
                "With label",
                Args::new().with("label", "Copying").with("percent", true).with("detail", "12 of 30 photos"),
            )
            .story(
                "Detail inline",
                Args::new()
                    .with("value", 0.6)
                    .with("label", "Stage 3 of 5 · Tests")
                    .with("percent", true)
                    .with("detail", "812 of 1,360 tests")
                    .with("detail_inline", true),
            )
            .story_with("All tones", Args::new(), all_progress_tones)
    }
}

fn all_alert_tones(_: &Args) -> AnyView {
    let rows: Vec<AnyView> = TONES
        .iter()
        .map(|name| Box::new(Alert::new(*name, "Shown in this tone.").tone(tone(name))) as AnyView)
        .collect();
    Box::new(VStack::new(rows).spacing(8.0).align(Align::Stretch))
}

fn all_progress_tones(_: &Args) -> AnyView {
    let rows: Vec<AnyView> =
        TONES.iter().map(|name| Box::new(ProgressBar::new(0.6).tone(tone(name))) as AnyView).collect();
    Box::new(VStack::new(rows).spacing(12.0).align(Align::Stretch))
}
