//! The feedback page.

use ravenrs::prelude::*;

use crate::{Gallery, columns, demo, page_header, sample_demos, samples};

impl Gallery {
    pub(crate) fn feedback(&self, width: f32) -> impl View + use<> {
        VStack::new((
            page_header("Feedback", "Alerts, progress and loading placeholders."),
            sample_demos(&[
                ("RichText", "Bold, italic, code and links in plain text, with the marks chat apps use. Links take the keyboard, and the app decides how to open them."),
                ("AttachmentStrip", "The files about to go with a message: icon or thumbnail, name, size, how to send each one and a button to take it out."),
                ("AudioPlayerButton", "Plays a recording and stops it, with progress and a transcript to open. The app can deliver the audio after the press."),
                ("SelectionBar", "What to do with several things chosen: how many, a way out, and actions that go to a menu when they do not fit. Escape leaves."),
                ("Dot", "A small mark of state, named for screen readers."),
                ("OverlayBadge", "A small view on a corner of another."),
                ("Rating", "Stars to give with the pointer, the arrows or a digit, or just to read."),
                ("IconTile", "An icon in a square tinted with its color."),
                ("EmptyState", "What an area says when it has nothing, and what to do about it."),
                ("HeroBanner", "A strong band, readable from across the room."),
                ("MultiProgress", "Work with more than one part."),
                ("ShimmerText", "A light passing over the text, while motion is allowed."),
                ("DaySeparator", "Dividing a list by day."),
                ("ShortcutSheet", "The keys of a screen, in a dialog."),
                ("ShortcutHintBar", "A line of hints under what they act on."),
                ("KeyBar", "The keys of a screen always in sight, as on a point of sale. They work wherever the focus is."),
                ("ScanField", "Takes what a barcode scanner reads: it keeps the focus and tells a scanner from fingers."),
            ]),
            demo(
                "Timeline",
                "Blocks along time on named tracks, under a ruler with markers and a playhead. The app holds the data and gets back what people do.",
                samples::sample("Timeline"),
            ),
            demo(
                "ToolRail",
                "A narrow bar of tools with their names under their icons, of which one is open.",
                samples::sample("ToolRail"),
            ),
            demo(
                "MoviePlayer",
                "Training films with no pixels: what the app drew, recorded from a script. It has chapters, marks to click and steps that wait, and `MovieEditor` cuts and dresses them.",
                samples::sample("MoviePlayer"),
            ),
            demo(
                "MovieAssistant",
                "Someone types a question and the moment of the film that answers it opens. It reads the words the films already hold, on this machine, with no model, and accents do not matter.",
                samples::sample("MovieAssistant"),
            ),
            demo(
                "Chat",
                "A conversation over a field to write in, with examples and cards to act on. It does not know who answers, so the same one serves a search, a person or a local model.",
                samples::sample("Chat"),
            ),
            demo(
                "ModelPicker",
                "Local AI models to download, try, use and remove. It shows what it is given, so the app decides which ones fit the machine.",
                samples::sample("ModelPicker"),
            ),
            demo(
                "FloatingPanel",
                "The sphere in the corner of this window: it opens a panel over the page, which stays usable behind it. Drag it, or press an arrow key on it, to send it to another corner. Escape closes the panel.",
                samples::sample("FloatingPanel"),
            ),
            demo(
                "ReleaseNotes",
                "An app's versions: the list, what the chosen one brought, and buttons that walk through them. The second block shows a command to copy.",
                VStack::new((
                    samples::sample("ReleaseNotes"),
                    samples::sample("CodeBlock"),
                ))
                .spacing(16.0)
                .align(Align::Stretch),
            ),
            demo(
                "Alerts",
                "Tinted messages with an icon for each tone.",
                VStack::new((
                    Alert::new(
                        "New version available",
                        "RavenRS 27.1 adds motion and report export to PDF and Excel.",
                    )
                    .tone(Tone::Accent),
                    Alert::new("Deployment finished", "The site is live on GitHub Pages.")
                        .tone(Tone::Success),
                    Alert::new("Storage almost full", "You are using 92% of your quota.").tone(Tone::Warning),
                    Alert::new("Payment failed", "Update your card to keep your workspace active.")
                        .tone(Tone::Danger),
                ))
                .spacing(12.0)
                .align(Align::Stretch),
            ),
            Grid::new(
                columns(width, 2),
                (
                    demo(
                        "Progress",
                        "Determinate progress bars.",
                        VStack::new((
                            ProgressBar::new(0.25),
                            ProgressBar::new(0.62),
                            ProgressBar::new(0.9).tone(Tone::Success),
                            ProgressBar::new(0.97).tone(Tone::Danger),
                            ProgressBar::new(0.4).label("Copying").percent().detail("12 of 30 photos").height(4.0),
                            Alert::message("Moving deletes the originals.").tone(Tone::Warning).compact(),
                            HStack::new((
                                Badge::new("Draft").outline(),
                                // ravenrs-lint: allow(raw_color) a tag keeps the color its owner picked for it, in every theme
                                Badge::new("Family").color(Color::hex(0x8b5cf6)).height(26.0),
                                Avatar::new("Ana Lima").border(2.0, Semantic::Accent),
                            ))
                            .spacing(8.0)
                            .align(Align::Center),
                        ))
                        .spacing(16.0)
                        .align(Align::Stretch),
                    ),
                    demo(
                        "Skeleton",
                        "Placeholders while content loads.",
                        HStack::new((
                            Skeleton::circle(44.0),
                            VStack::new((
                                Skeleton::line(14.0),
                                Skeleton::line(12.0),
                                Skeleton::block(120.0, 12.0),
                            ))
                            .spacing(8.0)
                            .align(Align::Stretch)
                            .grow(1.0),
                        ))
                        .spacing(16.0),
                    ),
                ),
            ),
        ))
        .spacing(24.0)
        .align(Align::Stretch)
    }
}
