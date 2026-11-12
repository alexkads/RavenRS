//! The two pages of overlays.

use ravenrs::prelude::*;

use crate::{Gallery, columns, demo, page_header, samples};

impl Gallery {
    pub(crate) fn overlays(&self, width: f32) -> impl View + use<> {
        let (open_popover, open_dialog) = (self.popover.clone(), self.dialog.clone());
        let log = |label: &'static str, state: &State<String>| {
            let s = state.clone();
            move || s.set(format!("You chose “{label}”."))
        };
        let close = self.dialog.clone();
        let confirm = self.dialog.clone();
        let done = self.last_action.clone();
        let more = self.more_overlays(width);
        VStack::new((
            page_header("Overlays", "Floating layers above the UI. Click outside or press Escape to close them."),
            more,
            Grid::new(
                columns(width, 2),
                (
                    demo(
                        "Dropdown menu",
                        "Actions with icons, shortcuts and a destructive item.",
                        VStack::new((
                            DropdownMenu::new("Actions")
                                .icon(IconName::Ellipsis)
                                .item(MenuItem::new("Edit").icon(IconName::Pencil).shortcut("⌘E").on_select(log("Edit", &self.last_action)))
                                .item(MenuItem::new("Duplicate").icon(IconName::Copy).shortcut("⌘D").on_select(log("Duplicate", &self.last_action)))
                                .item(MenuItem::new("Share").icon(IconName::Share2).on_select(log("Share", &self.last_action)))
                                .item(MenuItem::new("Export")
                                    .icon(IconName::Download)
                                    .item(MenuItem::new("As PDF").on_select(log("As PDF", &self.last_action)))
                                    .item(MenuItem::new("As CSV").on_select(log("As CSV", &self.last_action))))
                                .item(MenuItem::new("Pinned").checked(true))
                                .item(MenuItem::new("Label").element(Badge::new("Family").tone(Tone::Accent)))
                                .separator()
                                .item(MenuItem::new("Delete").icon(IconName::Trash2).destructive().on_select(log("Delete", &self.last_action))),
                            Text::bound(&self.last_action, Clone::clone).style(TextStyle::Callout).foreground(Semantic::InkMuted),
                        ))
                        .spacing(12.0),
                    ),
                    demo(
                        "Popover",
                        "Rich content anchored to a control.",
                        Popover::new(
                            self.popover.binding(),
                            Button::new("Show details")
                                .style(ButtonStyle::Secondary)
                                .icon(IconName::Info)
                                .on_click(move || open_popover.update(|o| *o = !*o)),
                            VStack::new((
                                Text::new("Storage").style(TextStyle::Headline),
                                Text::new("You are using 6.2 GB of 10 GB.").foreground(Semantic::InkMuted),
                                ProgressBar::new(0.62),
                            ))
                            .spacing(12.0)
                            .width(260.0),
                        ),
                    ),
                    demo(
                        "Tooltip",
                        "Hover any of these.",
                        HStack::new((
                            Tooltip::new(Badge::new("Beta").tone(Tone::Accent), "Available to early adopters"),
                            Tooltip::new(Avatar::new("Ana Lima"), "Ana Lima, Design"),
                            Tooltip::new(Kbd::new("⌘K"), "Open the command palette"),
                            Tooltip::new(Button::new("Save").style(ButtonStyle::Secondary), "Save the file").shortcut("⌘S"),
                            Button::new("Open").style(ButtonStyle::Secondary).tooltip("Open a file"),
                        ))
                        .spacing(16.0)
                        .align(Align::Center)
                        .wrap(),
                    ),
                    demo(
                        "Dialog",
                        "A modal over a dimmed backdrop.",
                        VStack::new((
                            Button::new("Delete project")
                                .style(ButtonStyle::Danger)
                                .icon(IconName::Trash2)
                                .on_click(move || open_dialog.set(true)),
                            Dialog::new(self.dialog.binding(), "Delete “RavenRS”?")
                                .description("The project, its 214 files and its history will be removed. This cannot be undone.")
                                .action(Button::new("Cancel").style(ButtonStyle::Secondary).on_click(move || close.set(false)))
                                .action(Button::new("Delete").style(ButtonStyle::Danger).on_click(move || {
                                    confirm.set(false);
                                    done.set("The project was deleted (not really).".into());
                                })),
                        )),
                    ),
                ),
            ),
        ))
        .spacing(24.0)
        .align(Align::Stretch)
    }

    pub(crate) fn more_overlays(&self, width: f32) -> impl View + use<> {
        let (sheet, toasts, palette) = (self.sheet.clone(), self.toasts.clone(), self.palette.clone());
        let (t1, t2) = (toasts.clone(), toasts.clone());
        Grid::new(
            columns(width, 2),
            (
                demo(
                    "Context menu",
                    "Right-click the area below.",
                    ContextMenu::new(
                        Text::new("Right-click anywhere in this box")
                            .foreground(Semantic::InkMuted)
                            .centered()
                            .height(90.0)
                            .background(Semantic::Control)
                            .corner_radius(12.0),
                    )
                    .item(MenuItem::new("Copy").icon(IconName::Copy).shortcut("⌘C"))
                    .item(MenuItem::new("Paste").icon(IconName::ClipboardPaste).shortcut("⌘V"))
                    .separator()
                    .item(MenuItem::new("Delete").icon(IconName::Trash2).destructive()),
                ),
                demo(
                    "Hover card",
                    "Rich previews that stay open while you read them.",
                    HoverCard::new(
                        Text::new("@ana.lima").foreground(Semantic::Accent),
                        HStack::new((
                            Avatar::new("Ana Lima").size(44.0),
                            VStack::new((
                                Text::new("Ana Lima").style(TextStyle::Headline),
                                Text::new("Design lead at Northwind. Building calm interfaces.")
                                    .style(TextStyle::Callout)
                                    .foreground(Semantic::InkMuted),
                                Text::new("Joined October 2024")
                                    .style(TextStyle::Caption)
                                    .foreground(Semantic::InkMuted),
                            ))
                            .spacing(4.0)
                            .width(220.0),
                        ))
                        .spacing(12.0),
                    ),
                ),
                demo(
                    "Sheet",
                    "A side panel over a dimmed backdrop.",
                    VStack::new((
                        Button::new("Open filters")
                            .style(ButtonStyle::Secondary)
                            .icon(IconName::Filter)
                            .on_click(move || sheet.set(true)),
                        Sheet::new(self.sheet.binding(), "Filters")
                            .description("Narrow down the accounts shown in the dashboard.")
                            .content(
                                VStack::new((
                                    Label::new("Plan"),
                                    SegmentedControl::new(
                                        State::new(1).binding(),
                                        ["All", "Team", "Business"],
                                    ),
                                    Label::new("Only active accounts"),
                                    Toggle::new(State::new(true).binding()).label("Active"),
                                ))
                                .spacing(12.0),
                            ),
                    )),
                ),
                demo(
                    "Splash screen",
                    "Covers the window while the app loads, then fades out. This one pretends to load for three seconds.",
                    samples::sample("SplashScreen"),
                ),
                demo(
                    "Lottie",
                    "Plays Lottie files with our own engine, drawn as curves on the GPU, sharp at any size.",
                    samples::sample("Lottie"),
                ),
                demo(
                    "Toasts and command palette",
                    "Toasts disappear after 4 seconds. Press ⌘K or Ctrl+K anywhere for the palette.",
                    HStack::new((
                        Button::new("Show toast")
                            .style(ButtonStyle::Secondary)
                            .icon(IconName::BellRing)
                            .on_click(move || {
                                t1.show(
                                    "Report exported",
                                    "sales-q3.pdf is ready to download.",
                                    Tone::Success,
                                )
                            }),
                        Button::new("Show warning").style(ButtonStyle::Ghost).on_click(move || {
                            t2.show("Storage almost full", "92% of your quota is used.", Tone::Warning)
                        }),
                        Button::new("Open palette")
                            .style(ButtonStyle::Ghost)
                            .icon(IconName::Command)
                            .on_click(move || palette.set(true)),
                    ))
                    .spacing(8.0)
                    .wrap(),
                ),
            ),
        )
    }
}
