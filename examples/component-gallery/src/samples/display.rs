//! Text, status and feedback.

use ravenrs::prelude::*;

use super::picture;

samples! {
    "Text" => text, "CodeBlock" => code_block, "RichText" => rich_text, "ReleaseNotes" => release_notes,
    "Alert" => alert, "Badge" => badge, "Avatar" => avatar, "Kbd" => kbd, "ProgressBar" => progress_bar,
    "Spinner" => spinner, "Skeleton" => skeleton, "Dot" => dot, "OverlayBadge" => overlay_badge,
    "IconTile" => icon_tile, "EmptyState" => empty_state, "HeroBanner" => hero_banner,
    "MultiProgress" => multi_progress, "ShimmerText" => shimmer_text, "DaySeparator" => day_separator,
    "Icon" => icon,
}

// sample: RichText, 170
fn rich_text() -> impl View {
    let opened = State::new(String::from("No link opened yet"));
    let told = opened.clone();
    VStack::new((
        RichText::new(
            "*Ready:* _your order_ ~left~ is at `Desk 4`.\n* Details at https://example.com/orders\n> Open until six",
        )
        .on_link(move |address| told.set(format!("Asked to open {address}"))),
        Text::bound(&opened, Clone::clone).style(TextStyle::Caption).foreground(Semantic::InkMuted),
    ))
    .spacing(8.0)
    .align(Align::Stretch)
    .width(360.0)
}
// end

// sample: CodeBlock, 70
fn code_block() -> impl View {
    CodeBlock::new("cargo add ravenrs")
}
// end

// sample: ReleaseNotes, 480
fn release_notes() -> impl View {
    let selected = State::new(0);
    ReleaseNotes::new(
        vec![
            ReleaseNote::new("Version 27.2", "Status bars and release notes")
                .date("Nov 1, 2026")
                .badge("New", Tone::Success)
                .changes(["A line at the foot of the window", "What each version brought"])
                .section("Why update", "Tell people about new versions without getting in their way."),
            ReleaseNote::new("Version 27.1", "The first of the year").badge("Installed", Tone::Neutral),
        ],
        selected.binding(),
    )
}
// end

// sample: Text, 300
fn text() -> impl View {
    VStack::new((
        Text::new("Display").style(TextStyle::Display),
        Text::new("Headline").style(TextStyle::Headline),
        Text::new("Muted body text").foreground(Semantic::InkMuted),
        Text::new("In the middle").align(TextAlign::Center),
        Text::new("At the end").align(TextAlign::End),
    ))
    .spacing(8.0)
    .align(Align::Stretch)
    .width(360.0)
}
// end

// sample: Alert, 260
fn alert() -> impl View {
    VStack::new((
        Alert::new("Deployment finished", "The site is live.").tone(Tone::Success),
        Alert::new("Payment failed", "Update your card.").tone(Tone::Danger),
        Alert::message("Moving deletes the originals.").tone(Tone::Warning).compact(),
    ))
    .spacing(12.0)
    .align(Align::Stretch)
}
// end

// sample: Badge, 80
fn badge() -> impl View {
    HStack::new((
        Badge::new("Neutral"),
        Badge::new("New").tone(Tone::Accent),
        Badge::new("Paid").tone(Tone::Success).icon(IconName::Check),
        Badge::new("Overdue").tone(Tone::Danger),
        Badge::new("Draft").outline(),
        // ravenrs-lint: allow(raw_color) a tag keeps the color its owner picked for it, in every theme
        Badge::new("Family").color(Color::hex(0x8b5cf6)).height(26.0),
    ))
    .spacing(8.0)
    .wrap()
}
// end

// sample: Avatar, 90
fn avatar() -> impl View {
    HStack::new((
        Avatar::new("Ana Lima"),
        Avatar::new("Bruno Reis").border(2.0, Semantic::Accent),
        Avatar::new("Carla Dias").size(44.0).image(Image::new(picture(1.0))),
    ))
    .spacing(8.0)
    .align(Align::Center)
}
// end

// sample: Kbd, 80
fn kbd() -> impl View {
    HStack::new((
        Kbd::new("⌘"),
        Kbd::new("Shift"),
        Kbd::new("K"),
        // The button gives the key its own color.
        Button::new("Open").size(ButtonSize::Small).shortcut("F8"),
    ))
    .spacing(8.0)
    .align(Align::Center)
}
// end

// sample: ProgressBar, 170
fn progress_bar() -> impl View {
    VStack::new((
        ProgressBar::new(0.25),
        ProgressBar::new(0.9).tone(Tone::Success),
        ProgressBar::new(0.4).label("Copying").percent().detail("12 of 30 photos").height(4.0),
    ))
    .spacing(16.0)
    .align(Align::Stretch)
}
// end

// sample: Spinner, 80
fn spinner() -> impl View {
    Spinner::new()
}
// end

// sample: Skeleton, 120
fn skeleton() -> impl View {
    HStack::new((
        Skeleton::circle(44.0),
        VStack::new((Skeleton::line(14.0), Skeleton::block(120.0, 12.0)))
            .spacing(8.0)
            .align(Align::Stretch)
            .grow(1.0),
    ))
    .spacing(16.0)
}
// end

// sample: Dot, 60
fn dot() -> impl View {
    HStack::new((
        Dot::new().tone(Tone::Warning).label("Not saved"),
        Text::new("Not saved").style(TextStyle::Callout),
        Dot::new().tone(Tone::Success).size(10.0).label("Online"),
        Text::new("Online").style(TextStyle::Callout),
    ))
    .spacing(8.0)
    .align(Align::Center)
}
// end

// sample: OverlayBadge, 90
fn overlay_badge() -> impl View {
    HStack::new((
        OverlayBadge::new(Avatar::new("Ana Lima"), Dot::new().tone(Tone::Success).size(10.0).label("Online"))
            .corner(FloatingCorner::BottomRight)
            .inset(5.0),
        OverlayBadge::new(
            Button::new("Open inbox").style(ButtonStyle::Secondary),
            Badge::new("3").tone(Tone::Danger),
        ),
    ))
    .spacing(32.0)
    .align(Align::Center)
}
// end

// sample: IconTile, 80
fn icon_tile() -> impl View {
    HStack::new((
        IconTile::new(IconName::CircleCheck).tone(Tone::Success),
        IconTile::new(IconName::TriangleAlert).tone(Tone::Warning),
        IconTile::new(IconName::Image).size(32.0),
    ))
    .spacing(12.0)
    .align(Align::Center)
}
// end

// sample: EmptyState, 260
fn empty_state() -> impl View {
    EmptyState::new("No photos yet")
        .icon(IconName::Image)
        .description("Import a folder to start. The originals stay where they are.")
        .action(Button::new("Import"))
        .action(Button::new("Learn more").style(ButtonStyle::Ghost))
}
// end

// sample: HeroBanner, 170
fn hero_banner() -> impl View {
    VStack::new((
        HeroBanner::new("You can remove the card").icon(IconName::CircleCheck).tone(Tone::Success),
        HeroBanner::new("Do not remove the card").icon(IconName::CircleAlert).tone(Tone::Danger),
    ))
    .spacing(12.0)
    .align(Align::Stretch)
}
// end

// sample: MultiProgress, 170
fn multi_progress() -> impl View {
    MultiProgress::new().label("Import").meter("Local copy", 84, 120).meter("Previews", 12, 84)
}
// end

// sample: ShimmerText, 60
fn shimmer_text() -> impl View {
    ShimmerText::new("Looking for updates")
}
// end

// sample: DaySeparator, 110
fn day_separator() -> impl View {
    let today = Date::new(2026, 11, 4).unwrap_or_else(|| Date::from_days(0));
    VStack::new((
        DaySeparator::of(today, today),
        DaySeparator::of(today.add_days(-1), today),
        DaySeparator::new("Last week"),
    ))
    .spacing(16.0)
    .align(Align::Stretch)
}
// end

// sample: Icon, 80
fn icon() -> impl View {
    HStack::new((
        Icon::new(IconName::Sparkles).size(24.0).color(Semantic::Accent),
        Icon::new(IconName::Bell).size(24.0),
        Icon::new(IconName::Check).size(24.0).color(Semantic::Success),
    ))
    .spacing(12.0)
}
// end
