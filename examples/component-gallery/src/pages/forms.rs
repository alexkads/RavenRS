//! The forms page.

use ravenrs::prelude::*;

use crate::{Gallery, columns, demo, page_header, sample_demos, samples};

impl Gallery {
    pub(crate) fn forms(&self, width: f32) -> impl View + use<> {
        let volume = self.volume.clone();
        VStack::new((
            page_header(
                "Forms",
                "Inputs bound to state. Every control works with the mouse and the keyboard.",
            ),
            sample_demos(&[
                ("ValueSlider", "A value in small, which opens a slider or can be typed."),
                ("DropZone", "A dashed area around any content, which takes dropped files."),
                ("InlineRename", "A name that turns into a field, by a double click or F2."),
                ("FavoriteToggle", "A heart or a star, filled when on."),
                ("SwatchGrid", "Colors to pick, with the arrow keys or a click that can carry a modifier."),
                ("NinePointAnchor", "A point of reference on a box, moved by the arrow keys."),
                ("ColorPair", "Two colors, with a button that swaps them."),
            ]),
            Grid::new(
                columns(width, 2),
                (
                    demo(
                        "Text fields",
                        "Type, edit with the arrows and Backspace, press Enter to submit.",
                        VStack::new((
                            TextField::new(self.name.binding()).placeholder("Full name").icon(IconName::User),
                            TextField::new(self.password.binding())
                                .placeholder("Password")
                                .icon(IconName::Lock)
                                .secure(),
                            Text::bound(&self.name, |n| {
                                if n.is_empty() { "Hello, stranger.".into() } else { format!("Hello, {n}.") }
                            })
                            .style(TextStyle::Callout)
                            .foreground(Semantic::InkMuted),
                        ))
                        .spacing(12.0)
                        .align(Align::Stretch),
                    ),
                    demo(
                        "Select and segmented control",
                        "Pick one option from a list or a compact row.",
                        VStack::new((
                            Select::new(
                                self.range.binding(),
                                ["Last 7 days", "Last 30 days", "Last 90 days", "This year"],
                            )
                            .label("Date range"),
                            SegmentedControl::new(
                                self.density.binding(),
                                ["Compact", "Default", "Comfortable"],
                            )
                            .label("Density"),
                        ))
                        .spacing(16.0),
                    ),
                    demo(
                        "Switches and checkboxes",
                        "Binary settings. The whole row is clickable.",
                        VStack::new((
                            Toggle::new(self.notifications.binding()).label("Push notifications"),
                            Toggle::new(self.autosave.binding()).label("Autosave drafts"),
                            Checkbox::new("I accept the terms", self.terms.binding()).leading(40.0),
                            Checkbox::new("Send me product news", self.newsletter.binding()).leading(40.0),
                        ))
                        .spacing(16.0),
                    ),
                    demo(
                        "Radio group",
                        "One choice among a few visible options.",
                        RadioGroup::new(
                            self.plan.binding(),
                            ["Starter: free", "Team: R$ 49 / seat", "Enterprise: talk to us"],
                        )
                        .label("Plan"),
                    ),
                    demo(
                        "Slider",
                        "Drag the thumb, or focus it and use the arrow keys.",
                        VStack::new((
                            Slider::new(self.volume.binding()).label("Volume"),
                            Text::dynamic(move || format!("Volume: {:.0}%", volume.get() * 100.0))
                                .style(TextStyle::Callout)
                                .foreground(Semantic::InkMuted),
                            Slider::new(self.exposure.binding())
                                .range(-5.0, 5.0)
                                .step(0.1)
                                .neutral(0.0)
                                .label("Exposure")
                                .show_value(),
                            Slider::new(self.warmth.binding())
                                // ravenrs-lint: allow(raw_color) the track runs from cold blue to warm yellow, which is what the value means
                                .track_colors([Color::hex(0x3b82f6), Color::hex(0xfacc15)])
                                .label("Temperature"),
                        ))
                        .spacing(12.0)
                        .align(Align::Stretch),
                    ),
                    demo(
                        "Long text, numbers and masks",
                        "Notes on several lines, numbers formatted when you leave, a phone number that follows its pattern.",
                        VStack::new((
                            samples::sample("TextArea"),
                            samples::sample("NumberField"),
                            samples::sample("MaskedField"),
                        ))
                        .spacing(12.0)
                        .align(Align::Stretch),
                    ),
                    demo(
                        "Times and records",
                        "Type a time or pick from the list; search a record loaded on demand.",
                        VStack::new((
                            samples::sample("DateTimePicker"),
                            samples::sample("LookupField"),
                        ))
                        .spacing(12.0),
                    ),
                    demo(
                        "Form layout and filter builder",
                        "Groups in columns that collapse when there is no room; a filter made of lines.",
                        VStack::new((samples::sample("FormLayout"), samples::sample("FilterBuilder")))
                            .spacing(16.0)
                            .align(Align::Stretch),
                    ),
                    demo(
                        "Commands",
                        "A toolbar folds into a menu when it runs out of room; a button with alternatives; a question first.",
                        VStack::new((
                            samples::sample("Toolbar"),
                            HStack::new((samples::sample("SplitButton"), samples::sample("ConfirmDialog")))
                                .spacing(12.0)
                                .align(Align::Center),
                        ))
                        .spacing(12.0)
                        .align(Align::Stretch),
                    ),
                    demo(
                        "Stepper",
                        "Small integer quantities with bounds.",
                        HStack::new((
                            Stepper::new(self.seats.binding()).range(1, 50).label("Seats"),
                            Text::bound(&self.seats, |s| format!("{s} seats")).foreground(Semantic::InkMuted),
                        ))
                        .spacing(12.0)
                        .align(Align::Center)
                        .wrap(),
                    ),
                    demo("Color swatches", "A palette of named colors, with the current one ringed.", {
                        let colors = [Semantic::Accent, Semantic::Success, Semantic::Warning, Semantic::Danger];
                        let names = ["Accent", "Success", "Warning", "Danger"];
                        let theme = Theme::raven_light();
                        let swatches: Vec<AnyView> = colors
                            .iter()
                            .zip(names)
                            .enumerate()
                            .map(|(i, (role, name))| {
                                let pick = self.swatch.clone();
                                Box::new(
                                    ColorSwatch::new(theme.color(*role), name)
                                        .selected(self.swatch.get() == i)
                                        .on_click(move || pick.set(i)),
                                ) as AnyView
                            })
                            .collect();
                        HStack::new(swatches).wrap()
                    }),
                    demo("Clickable card", "The whole card is a button. Long text can end with an ellipsis.", {
                        let clicks = self.clicks.clone();
                        Card::new(
                            VStack::new((
                                Text::new("Quarterly report").style(TextStyle::Headline),
                                Text::new("A summary far too long to fit on a single line of this small card, so it is cut.")
                                    .style(TextStyle::Callout)
                                    .foreground(Semantic::InkMuted)
                                    .truncate(),
                            ))
                            .spacing(4.0)
                            .align(Align::Stretch),
                        )
                        .corner_radius(16.0)
                        .padding(16.0)
                        .no_shadow()
                        .on_click(move || clicks.update(|c| *c += 1))
                    }),
                ),
            ),
            Grid::new(
                columns(width, 2),
                (
                    demo(
                        "Fields with more inside",
                        "Views before and after the text, a clear button, sizes, a box that grows and a select by value with search.",
                        VStack::new((
                            TextField::new(self.invoice.binding())
                                .placeholder("Price")
                                .size(ButtonSize::Small)
                                .prefix(Text::new("R$").foreground(Semantic::InkMuted))
                                .suffix(Badge::new("per month"))
                                .clearable(),
                            TextArea::new(self.message.binding()).placeholder("Type a message").grows(2, 5),
                            Select::of(self.currency.binding(), [("BRL", "Real"), ("EUR", "Euro"), ("USD", "Dollar")])
                                .label("Currency")
                                .size(ButtonSize::Small)
                                .searchable(),
                            Toggle::new(self.autosave.binding()).label("Small switch").small(),
                        ))
                        .spacing(12.0)
                        .align(Align::Stretch),
                    ),
                    demo(
                        "Trees and tags",
                        "Items inside items, with the keyboard; several values as tags, adding new ones.",
                        VStack::new((samples::sample("TreeView"), samples::sample("TagBox")))
                            .spacing(16.0)
                            .align(Align::Stretch),
                    ),
                    demo(
                        "Colors and periods",
                        "A palette, a shade square and a hue bar; a period in two clicks, or ready-made.",
                        VStack::new((samples::sample("ColorPicker"), samples::sample("DateRangePicker")))
                            .spacing(12.0),
                    ),
                    demo(
                        "Steps",
                        "A step can refuse to be left, saying why; steps already done can be reopened.",
                        samples::sample("Wizard"),
                    ),
                    demo(
                        "Where the steps stand",
                        "The same marks for an app that lays out its own steps, with one that has a problem.",
                        VStack::new((samples::sample("StepIndicator"), samples::sample("StepBadge")))
                            .spacing(16.0)
                            .align(Align::Stretch),
                    ),
                    demo(
                        "Files",
                        "From the system's chooser or dropped on the place, refused with a reason when they do not fit.",
                        VStack::new((samples::sample("FileUpload"), samples::sample("ImageField")))
                            .spacing(16.0)
                            .align(Align::Stretch),
                    ),
                ),
            ),
        ))
        .spacing(24.0)
        .align(Align::Stretch)
    }
}
