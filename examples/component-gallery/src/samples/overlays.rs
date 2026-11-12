//! What opens over the page.

use ravenrs::prelude::*;

samples! {
    "ConfirmDialog" => confirm_dialog, "Dialog" => dialog, "Sheet" => sheet, "Popover" => popover,
    "HoverCard" => hover_card, "Tooltip" => tooltip, "HelpTip" => help_tip, "DropdownMenu" => dropdown_menu,
    "ContextMenu" => context_menu, "SplashScreen" => splash_screen, "CommandPalette" => command_palette,
    "Toaster" => toaster, "FloatingPanel" => floating_panel, "SelectionBar" => selection_bar,
}

// sample: ConfirmDialog, 90
fn confirm_dialog() -> impl View {
    let confirm = ConfirmDialog::new();
    let asker = confirm.clone();
    VStack::new((
        Button::new("Delete").style(ButtonStyle::Danger).on_click(move || {
            asker.ask("Delete this record?", "This cannot be undone.", "Delete", || {});
        }),
        confirm,
    ))
}
// end

// sample: Dialog, 340
fn dialog() -> impl View {
    let open = State::new(false);
    let (show, cancel, confirm) = (open.clone(), open.clone(), open.clone());
    VStack::new((
        Button::new("Delete project").style(ButtonStyle::Danger).on_click(move || show.set(true)),
        Dialog::new(open.binding(), "Delete the project?")
            .description("This cannot be undone.")
            .action(Button::new("Cancel").style(ButtonStyle::Secondary).on_click(move || cancel.set(false)))
            .action(Button::new("Delete").style(ButtonStyle::Danger).on_click(move || confirm.set(false)))
            // A stray click outside must not close this one.
            .close_on_backdrop(false),
    ))
}
// end

// sample: Sheet, 380
fn sheet() -> impl View {
    let open = State::new(false);
    let show = open.clone();
    let active = State::new(true);
    VStack::new((
        Button::new("Open filters").style(ButtonStyle::Secondary).on_click(move || show.set(true)),
        Sheet::new(open.binding(), "Filters")
            .description("Narrow down the accounts shown.")
            .content(Toggle::new(active.binding()).label("Only active accounts")),
    ))
}
// end

// sample: SplashScreen, 380
fn splash_screen() -> impl View {
    /// Pretends to load for three seconds. A real app sets the same values as its work advances.
    struct Loading;

    impl View for Loading {
        fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
            const SECONDS: f64 = 3.0;
            let visible = cx.use_state(|| false);
            let started = cx.use_state(|| None::<f64>);
            let now = cx.now();
            let elapsed = match (visible.get(), started.get()) {
                (true, Some(start)) => now - start,
                (true, None) => {
                    started.set(Some(now));
                    0.0
                }
                (false, _) => {
                    started.set_if_changed(None);
                    0.0
                }
            };
            if elapsed >= SECONDS {
                visible.set(false);
            } else if visible.get() {
                cx.wake_at(now + 0.25);
            }
            let status = match elapsed {
                e if e < 1.0 => "Opening the database",
                e if e < 2.0 => "Loading your accounts",
                _ => "Almost there",
            };
            let show = visible.clone();
            let splash =
                SplashScreen::new(visible.binding(), "Raven Finance").icon(IconName::Wallet).status(status);
            // A spinner until there is something to measure, then a bar.
            let splash = if elapsed < 1.0 { splash } else { splash.progress((elapsed / SECONDS) as f32) };
            let page = VStack::new((
                Button::new("Show the splash screen")
                    .style(ButtonStyle::Secondary)
                    .on_click(move || show.set(true)),
                splash,
            ));
            cx.child(&page)
        }
    }

    Loading
}
// end

// sample: Popover, 240
fn popover() -> impl View {
    let open = State::new(false);
    let flip = open.clone();
    Popover::new(
        open.binding(),
        Button::new("Show details").style(ButtonStyle::Secondary).on_click(move || flip.update(|o| *o = !*o)),
        VStack::new((Text::new("Storage").style(TextStyle::Headline), ProgressBar::new(0.62)))
            .spacing(8.0)
            .width(220.0),
    )
    // `Placement::Above` and the others work too; `.surface(false)` drops the panel.
    .placement(Placement::Below)
}
// end

// sample: HoverCard, 220
fn hover_card() -> impl View {
    HoverCard::new(
        Text::new("@ana.lima").foreground(Semantic::Accent),
        HStack::new((Avatar::new("Ana Lima"), Text::new("Design lead at Northwind"))).spacing(12.0),
    )
}
// end

// sample: Tooltip, 130
fn tooltip() -> impl View {
    VStack::new((
        Spacer::new().min(40.0),
        HStack::new((
            Tooltip::new(Button::new("Show tip").style(ButtonStyle::Secondary), "It stays out of the way"),
            Button::new("Save").style(ButtonStyle::Secondary).tooltip("Save the file"),
            Tooltip::new(Button::new("Open").style(ButtonStyle::Secondary), "Open a file").shortcut("⌘O"),
        ))
        .spacing(8.0),
    ))
}
// end

// sample: HelpTip, 130
fn help_tip() -> impl View {
    HStack::new((
        Text::new("Recurring"),
        HelpTip::new("Recurring", "It comes back on the same day of each month.")
            .how("Choose it when you save."),
    ))
    .spacing(8.0)
}
// end

// sample: DropdownMenu, 360
fn dropdown_menu() -> impl View {
    DropdownMenu::new("Actions")
        .item(MenuItem::new("Edit").icon(IconName::Pencil).shortcut("⌘E"))
        .item(MenuItem::new("Share").icon(IconName::Share2))
        .item(MenuItem::new("Export").item(MenuItem::new("As PDF")).item(MenuItem::new("As CSV")))
        .item(MenuItem::new("Show grid").checked(true))
        .item(MenuItem::new("Label").element(Badge::new("Family").tone(Tone::Accent)))
        .separator()
        .item(MenuItem::new("Delete").icon(IconName::Trash2).destructive())
}
// end

// sample: ContextMenu, 240
fn context_menu() -> impl View {
    ContextMenu::new(
        Text::new("Right-click in this box")
            .foreground(Semantic::InkMuted)
            .centered()
            .height(90.0)
            .background(Semantic::Control)
            .corner_radius(12.0),
    )
    .item(MenuItem::new("Copy").icon(IconName::Copy).shortcut("⌘C"))
    .item(MenuItem::new("Sort by").item(MenuItem::new("Name")).item(MenuItem::new("Date")))
    .item(MenuItem::new("Delete").icon(IconName::Trash2).destructive())
}
// end

// sample: CommandPalette, 400
fn command_palette() -> impl View {
    let open = State::new(false);
    let show = open.clone();
    VStack::new((
        Button::new("Open palette").icon(IconName::Command).on_click(move || show.set(true)),
        CommandPalette::new(open.binding())
            .command(Command::new("Go to reports", || {}).icon(IconName::FileText).group("Pages"))
            .command(Command::new("Open settings", || {}).icon(IconName::Settings).group("Pages")),
    ))
}
// end

// sample: Toaster, 240
fn toaster() -> impl View {
    let toasts = Toasts::new();
    let show = toasts.clone();
    VStack::new((
        Button::new("Show toast")
            .on_click(move || show.show("Report exported", "sales-q3.pdf is ready.", Tone::Success)),
        // `toasts.show_until_closed` keeps one up until it is closed.
        Toaster::new(toasts).position(ToastPosition::BottomRight),
    ))
}
// end

// sample: SelectionBar, 60
fn selection_bar() -> impl View {
    SelectionBar::new(3)
        .on_exit(|| {})
        .action(ToolbarAction::new("Copy").icon(IconName::Copy).icon_only().on_click(|| {}))
        .action(ToolbarAction::new("Favorite").icon(IconName::Star).icon_only().on_click(|| {}))
        .width(420.0)
}
// end

// sample: FloatingPanel, 80
fn floating_panel() -> impl View {
    /// Keeps whether it is open, here while the page rebuilds.
    struct Corner;

    impl View for Corner {
        fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
            let open = cx.use_state(|| false);
            // `.movable` and `.minimized` take bindings, for the app to remember them.
            cx.child(&FloatingPanel::new(open.binding(), "Assistant", || {
                Box::new(Text::new("The app stays usable behind this panel."))
            }))
        }
    }
    Corner
}
// end
