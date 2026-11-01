//! One small example per component, for the component pages of the site. The
//! pages show these functions as they are, so the code a reader copies compiles.
//!
//! `scripts/gen-component-pages.py` reads the markers: `// sample: Name, height`
//! before a function and `// end` after it.

use ravenrs::layout::{Axis, LayoutStyle};
use ravenrs::prelude::*;

macro_rules! samples {
    ($($name:literal => $function:ident),* $(,)?) => {
        /// The name of every component with an example.
        pub const NAMES: &[&str] = &[$($name),*];

        /// The example of a component.
        pub fn sample(name: &str) -> Option<AnyView> {
            match name {
                $($name => Some(Box::new($function())),)*
                _ => None,
            }
        }
    };
}

samples! {
    "Button" => button, "IconButton" => icon_button, "ToggleButton" => toggle_button,
    "ToggleGroup" => toggle_group, "SegmentedControl" => segmented_control, "Toggle" => toggle,
    "Checkbox" => checkbox, "RadioGroup" => radio_group, "Slider" => slider, "Stepper" => stepper,
    "ColorSwatch" => color_swatch, "ThemeSwitcher" => theme_switcher,
    "TextField" => text_field, "Select" => select, "Combobox" => combobox, "DatePicker" => date_picker,
    "Calendar" => calendar, "EventCalendar" => event_calendar, "Scheduler" => scheduler,
    "RecurrenceEditor" => recurrence_editor, "InputOtp" => input_otp, "FormField" => form_field, "Label" => label,
    "Ribbon" => ribbon, "Menubar" => menubar, "Tabs" => tabs, "Sidebar" => sidebar,
    "Breadcrumb" => breadcrumb, "Pagination" => pagination, "Accordion" => accordion,
    "Disclosure" => disclosure,
    "Dialog" => dialog, "Sheet" => sheet, "Popover" => popover, "HoverCard" => hover_card,
    "Tooltip" => tooltip, "DropdownMenu" => dropdown_menu, "ContextMenu" => context_menu,
    "SplashScreen" => splash_screen, "CommandPalette" => command_palette, "Toaster" => toaster,
    "DataGrid" => data_grid, "Table" => table, "List" => list, "Stat" => stat,
    "Chart" => chart, "BarChart" => bar_chart, "AreaChart" => area_chart, "Barcode" => barcode,
    "VStack" => v_stack, "HStack" => h_stack, "Grid" => grid, "Spacer" => spacer,
    "ScrollView" => scroll_view, "Splitter" => splitter, "Card" => card, "GlassCard" => glass_card,
    "Section" => section, "Divider" => divider, "Footer" => footer, "AspectRatio" => aspect_ratio, "Collapse" => collapse,
    "Text" => text, "Alert" => alert, "Badge" => badge, "Avatar" => avatar, "Kbd" => kbd,
    "ProgressBar" => progress_bar, "Spinner" => spinner, "Skeleton" => skeleton,
    "Icon" => icon, "Image" => image, "Carousel" => carousel, "Canvas" => canvas,
}

/// A colored picture to show in the image examples.
fn picture(hue: f32) -> ImageData {
    ImageData::from_fn(160, 90, move |x, y| {
        let (u, v) = (x as f32 / 160.0, y as f32 / 90.0);
        Color::rgb(0.2 + 0.6 * u * hue, 0.3 + 0.4 * v, 0.9 - 0.5 * u)
    })
    .unwrap_or_else(|| unreachable!("the size is not zero"))
}

// sample: Button, 150
fn button() -> impl View {
    let clicks = State::new(0);
    let add = clicks.clone();
    VStack::new((
        HStack::new((
            Button::new("Save").on_click(move || add.update(|n| *n += 1)),
            Button::new("Cancel").style(ButtonStyle::Secondary),
            Button::new("More").style(ButtonStyle::Ghost),
            Button::new("Delete").style(ButtonStyle::Danger).icon(IconName::Trash2),
        ))
        .spacing(8.0)
        .wrap(),
        Text::bound(&clicks, |n| format!("Saved {n} times")).foreground(Semantic::InkMuted),
    ))
    .spacing(12.0)
}
// end

// sample: IconButton, 90
fn icon_button() -> impl View {
    HStack::new((
        IconButton::new(IconName::Pencil).label("Edit"),
        IconButton::new(IconName::Copy).label("Duplicate"),
        IconButton::new(IconName::Settings).label("Settings").style(ButtonStyle::Secondary),
        IconButton::new(IconName::Plus).label("New").style(ButtonStyle::Primary),
    ))
    .spacing(8.0)
}
// end

// sample: ToggleButton, 90
fn toggle_button() -> impl View {
    let bold = State::new(true);
    let italic = State::new(false);
    HStack::new((ToggleButton::new(bold.binding(), "Bold"), ToggleButton::new(italic.binding(), "Italic")))
        .spacing(8.0)
}
// end

// sample: ToggleGroup, 90
fn toggle_group() -> impl View {
    let align = State::new(0);
    ToggleGroup::single(
        align.binding(),
        [
            (Some(IconName::AlignLeft), ""),
            (Some(IconName::AlignCenter), ""),
            (Some(IconName::AlignRight), ""),
        ],
    )
}
// end

// sample: SegmentedControl, 90
fn segmented_control() -> impl View {
    let period = State::new(1);
    SegmentedControl::new(period.binding(), ["Day", "Week", "Month"]).label("Period")
}
// end

// sample: Toggle, 110
fn toggle() -> impl View {
    let push = State::new(true);
    let email = State::new(false);
    VStack::new((
        Toggle::new(push.binding()).label("Push notifications"),
        Toggle::new(email.binding()).label("Email digest"),
    ))
    .spacing(12.0)
}
// end

// sample: Checkbox, 110
fn checkbox() -> impl View {
    let terms = State::new(true);
    let news = State::new(false);
    VStack::new((
        Checkbox::new("I accept the terms", terms.binding()),
        Checkbox::new("Send me product news", news.binding()),
    ))
    .spacing(10.0)
}
// end

// sample: RadioGroup, 150
fn radio_group() -> impl View {
    let plan = State::new(1);
    RadioGroup::new(plan.binding(), ["Starter", "Team", "Enterprise"]).label("Plan")
}
// end

// sample: Slider, 110
fn slider() -> impl View {
    let volume = State::new(0.6_f32);
    let shown = volume.clone();
    VStack::new((
        Slider::new(volume.binding()).label("Volume"),
        Text::dynamic(move || format!("{:.0}%", shown.get() * 100.0)).foreground(Semantic::InkMuted),
    ))
    .spacing(12.0)
    .align(Align::Stretch)
}
// end

// sample: Stepper, 90
fn stepper() -> impl View {
    let seats = State::new(3);
    Stepper::new(seats.binding()).range(1, 50).label("Seats")
}
// end

// sample: ColorSwatch, 90
fn color_swatch() -> impl View {
    let current = State::new(0);
    view_fn(move || {
        let theme = Theme::raven_light();
        let options =
            [(Semantic::Accent, "Accent"), (Semantic::Success, "Success"), (Semantic::Danger, "Danger")];
        let swatches: Vec<AnyView> = options
            .iter()
            .enumerate()
            .map(|(i, (role, name))| {
                let pick = current.clone();
                Box::new(
                    ColorSwatch::new(theme.color(*role), *name)
                        .selected(current.get() == i)
                        .on_click(move || pick.set(i)),
                ) as AnyView
            })
            .collect();
        HStack::new(swatches)
    })
}
// end

// sample: ThemeSwitcher, 90
fn theme_switcher() -> impl View {
    ThemeSwitcher
}
// end

// sample: TextField, 130
fn text_field() -> impl View {
    let name = State::new(String::new());
    let shown = name.clone();
    VStack::new((
        TextField::new(name.binding()).placeholder("Full name").icon(IconName::User),
        Text::dynamic(move || format!("Hello, {}", shown.get())).foreground(Semantic::InkMuted),
    ))
    .spacing(12.0)
    .align(Align::Stretch)
}
// end

// sample: Select, 280
fn select() -> impl View {
    let range = State::new(0);
    Select::new(range.binding(), ["Last 7 days", "Last 30 days", "This year"]).label("Date range")
}
// end

// sample: Combobox, 320
fn combobox() -> impl View {
    let country = State::new(None);
    Combobox::new(country.binding(), ["Argentina", "Brazil", "Chile", "Portugal", "Spain"])
        .placeholder("Choose a country")
        .label("Country")
}
// end

// sample: DatePicker, 420
fn date_picker() -> impl View {
    let today = Date::new(2026, 11, 1).unwrap_or_else(|| unreachable!("a real date"));
    let date = State::new(None);
    DatePicker::new(date.binding(), today).today(today).label("Date")
}
// end

// sample: Calendar, 380
fn calendar() -> impl View {
    let today = Date::new(2026, 11, 1).unwrap_or_else(|| unreachable!("a real date"));
    let date = State::new(Some(today));
    Calendar::new(date.binding(), today).today(today)
}
// end

// sample: EventCalendar, 620
fn event_calendar() -> impl View {
    let today = Date::new(2026, 11, 16).unwrap_or_else(|| unreachable!("a real date"));
    let (month, day) = (State::new(today.first_of_month()), State::new(Some(today)));
    EventCalendar::new(month.binding(), day.binding()).today(today).events([
        CalendarEvent::new(1, today.add_days(-4), "Rent").done(true),
        CalendarEvent::new(2, today, "Review").tone(Tone::Warning),
        CalendarEvent::new(3, today.add_days(3), "Salary").tone(Tone::Success),
    ])
}
// end

// sample: Scheduler, 400
fn scheduler() -> impl View {
    let first = Date::new(2026, 11, 1).unwrap_or_else(|| unreachable!("a real date"));
    let at = |day: i64| TimePoint::from_date(first).plus_days(day);
    let axis = State::new(TimeAxis::days(first, 30));
    Scheduler::new(axis.binding())
        .resources([Resource::new(1, "Ana"), Resource::new(2, "Bruno"), Resource::new(3, "Carla")])
        .events([
            SchedulerEvent::new(1, 1, at(1), at(5), "Vacation").tone(Tone::Success),
            SchedulerEvent::new(2, 1, at(3), at(8), "Workshop"),
            SchedulerEvent::new(3, 2, at(2), at(6), "On call").tone(Tone::Warning),
        ])
        .on_change(|change| println!("{change:?}"))
}
// end

// sample: RecurrenceEditor, 280
fn recurrence_editor() -> impl View {
    let start = Date::new(2026, 11, 5).unwrap_or_else(|| unreachable!("a real date"));
    let rule = State::new(RecurrenceRule::new(Frequency::Weekly).on_weekdays([1, 4]));
    RecurrenceEditor::new(rule.binding(), start)
}
// end

// sample: InputOtp, 90
fn input_otp() -> impl View {
    let code = State::new(String::new());
    InputOtp::new(code.binding(), 6)
}
// end

// sample: FormField, 130
fn form_field() -> impl View {
    let email = State::new(String::new());
    FormField::new("Work email", TextField::new(email.binding()).placeholder("you@company.com"))
        .required()
        .hint("We send the invoice here.")
        .validate(email.binding(), validate::email)
}
// end

// sample: Label, 80
fn label() -> impl View {
    Label::new("Company name")
}
// end

// sample: Ribbon, 240
fn ribbon() -> impl View {
    Ribbon::new()
        .app_menu("File", vec![MenuItem::new("Save").icon(IconName::Save).shortcut("⌘S")])
        .quick_access(RibbonItem::button("Undo", IconName::Undo2))
        .page(
            RibbonPage::new("Home")
                .group(
                    RibbonGroup::new("Clipboard")
                        .item(RibbonItem::button("Paste", IconName::ClipboardPaste).large())
                        .item(RibbonItem::button("Copy", IconName::Copy).small()),
                )
                .group(
                    RibbonGroup::new("Output").item(RibbonItem::button("Print", IconName::Printer).large()),
                ),
        )
        .page(
            RibbonPage::new("Insert")
                .group(RibbonGroup::new("Items").item(RibbonItem::button("Image", IconName::Image).large())),
        )
}
// end

// sample: Menubar, 240
fn menubar() -> impl View {
    Menubar::new()
        .menu("File")
        .item(MenuItem::new("New").icon(IconName::FilePlus).shortcut("⌘N"))
        .separator()
        .item(MenuItem::new("Print").icon(IconName::Printer).shortcut("⌘P"))
        .menu("Edit")
        .item(MenuItem::new("Undo").icon(IconName::Undo2).shortcut("⌘Z"))
}
// end

// sample: Tabs, 170
fn tabs() -> impl View {
    let tab = State::new(0);
    Tabs::new(tab.binding())
        .tab("Activity", Text::new("Ana merged #482").foreground(Semantic::InkMuted))
        .tab("Settings", Text::new("Each tab builds when selected.").foreground(Semantic::InkMuted))
        .tab("Billing", Stat::new("Next invoice", "R$ 1.240"))
}
// end

// sample: Sidebar, 440
fn sidebar() -> impl View {
    let state = SidebarState::new(true);
    HStack::new((
        Sidebar::new((
            SidebarGroup::new(
                "Workspace",
                (
                    SidebarItem::new(IconName::LayoutDashboard, "Overview").selected(true),
                    SidebarItem::new(IconName::Users, "Customers").badge(Badge::new("12")),
                    SidebarItem::new(IconName::Settings, "Settings")
                        .item(SidebarItem::text("General").compact())
                        .item(SidebarItem::text("Billing").compact()),
                ),
            ),
            SidebarGroup::new("Projects", (SidebarSkeleton::new(2),)).collapsible(),
        ))
        .collapsible(SidebarCollapse::Icons)
        .state(state.clone())
        .footer(
            HStack::new((Avatar::new("Ana Lima"), Text::new("Ana Lima").style(TextStyle::Callout)))
                .spacing(10.0)
                .align(Align::Center),
        ),
        VStack::new((SidebarTrigger::new(state), Text::new("Or press Cmd/Ctrl+B.")))
            .spacing(12.0)
            .padding(16.0)
            .grow(1.0),
    ))
    .height(420.0)
    .align(Align::Stretch)
}
// end

// sample: Breadcrumb, 80
fn breadcrumb() -> impl View {
    Breadcrumb::new(["Workspace", "Projects", "RavenRS", "Settings"])
}
// end

// sample: Pagination, 90
fn pagination() -> impl View {
    let page = State::new(0);
    Pagination::new(page.binding(), 24)
}
// end

// sample: Accordion, 230
fn accordion() -> impl View {
    Accordion::new(vec![
        Disclosure::new("What platforms does it support?", Text::new("Desktop and browsers with WebGPU."))
            .open(true),
        Disclosure::new("Can I build my own components?", Text::new("Yes, with the same public API.")),
    ])
}
// end

// sample: Disclosure, 130
fn disclosure() -> impl View {
    Disclosure::new("Advanced options", Text::new("Shown when open.").foreground(Semantic::InkMuted))
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
            .action(Button::new("Delete").style(ButtonStyle::Danger).on_click(move || confirm.set(false))),
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
}
// end

// sample: HoverCard, 220
fn hover_card() -> impl View {
    HoverCard::new(
        Text::new("@ana.lima").foreground(Semantic::Accent),
        HStack::new((Avatar::new("Ana Lima"), Text::new("Design lead at Northwind"))).spacing(10.0),
    )
}
// end

// sample: Tooltip, 130
fn tooltip() -> impl View {
    VStack::new((
        Spacer::new().min(40.0),
        Tooltip::new(Button::new("Hover me").style(ButtonStyle::Secondary), "It stays out of the way"),
    ))
}
// end

// sample: DropdownMenu, 260
fn dropdown_menu() -> impl View {
    DropdownMenu::new("Actions")
        .item(MenuItem::new("Edit").icon(IconName::Pencil).shortcut("⌘E"))
        .item(MenuItem::new("Share").icon(IconName::Share2))
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
        Toaster::new(toasts),
    ))
}
// end

// sample: DataGrid, 420
fn data_grid() -> impl View {
    let rows = (0..1000)
        .map(|i| {
            vec![
                Value::Number(f64::from(i)),
                Value::Text(format!("Customer {i}")),
                Value::Number(f64::from(i % 90) * 13.5),
            ]
        })
        .collect();
    DataGrid::new(
        GridData::new(rows),
        [
            GridColumn::number("ID").width(90.0),
            GridColumn::text("Customer").width(220.0).editable(),
            GridColumn::currency("MRR", "R$ ", 2).width(150.0).summary(Summary::Sum),
        ],
    )
    .height(380.0)
}
// end

// sample: Table, 190
fn table() -> impl View {
    Table::new([Column::new("Invoice"), Column::new("Customer").weight(2.0), Column::new("Amount").numeric()])
        .row(["INV-1042", "Northwind Labs", "R$ 4.800"])
        .row(["INV-1043", "Aurora Studio", "R$ 1.720"])
}
// end

// sample: List, 210
fn list() -> impl View {
    List::new((
        ListItem::new("Ana Lima").subtitle("ana@northwind.dev").avatar("Ana Lima").trailing("Owner"),
        ListItem::new("Report.pdf").subtitle("2.4 MB").icon(IconName::FileText).on_click(|| {}),
    ))
}
// end

// sample: Stat, 120
fn stat() -> impl View {
    HStack::new((
        Stat::new("Revenue", "R$ 128.450").delta("+12.4%", true),
        Stat::new("Churn", "1.8%").delta("-0.4%", false),
    ))
    .spacing(32.0)
}
// end

// sample: Chart, 280
fn chart() -> impl View {
    Chart::new(ChartKind::Line)
        .title("Sales")
        .categories(["Jan", "Feb", "Mar", "Apr", "May", "Jun"])
        .series("2025", [12.0, 18.0, 15.0, 21.0, 19.0, 24.0])
        .series("2026", [14.0, 21.0, 26.0, 24.0, 31.0, 35.0])
        .height(260.0)
}
// end

// sample: BarChart, 220
fn bar_chart() -> impl View {
    BarChart::new([("Mon", 42.0), ("Tue", 58.0), ("Wed", 51.0), ("Thu", 73.0), ("Fri", 66.0)]).height(180.0)
}
// end

// sample: AreaChart, 220
fn area_chart() -> impl View {
    AreaChart::new([12.0, 14.0, 13.0, 17.0, 16.0, 21.0, 19.0, 24.0, 23.0, 27.0]).height(180.0)
}
// end

// sample: Barcode, 190
fn barcode() -> impl View {
    HStack::new((
        Barcode::qr("https://ravenrs.dev").alt("Link to the RavenRS site"),
        Barcode::new(Symbology::Ean13, "789123456789"),
    ))
    .spacing(24.0)
}
// end

// sample: VStack, 150
fn v_stack() -> impl View {
    VStack::new((Text::new("First"), Text::new("Second"), Button::new("Third"))).spacing(12.0)
}
// end

// sample: HStack, 90
fn h_stack() -> impl View {
    HStack::new((Text::new("First"), Text::new("Second"), Button::new("Third")))
        .spacing(12.0)
        .align(Align::Center)
}
// end

// sample: Grid, 170
fn grid() -> impl View {
    let cell = |n: u32| Card::new(Text::new(n.to_string())).padding(16.0);
    Grid::new(3, (cell(1), cell(2), cell(3), cell(4), cell(5), cell(6))).spacing(8.0)
}
// end

// sample: Spacer, 90
fn spacer() -> impl View {
    HStack::new((Text::new("Left"), Spacer::new(), Button::new("Right"))).align(Align::Center)
}
// end

// sample: ScrollView, 220
fn scroll_view() -> impl View {
    let rows: Vec<AnyView> = (1..=30).map(|n| Box::new(Text::new(format!("Row {n}"))) as AnyView).collect();
    ScrollView::new(rows).spacing(8.0).height(180.0)
}
// end

// sample: Splitter, 230
fn splitter() -> impl View {
    Splitter::horizontal(Text::new("Explorer").padding(16.0), Text::new("Editor").padding(16.0))
        .ratio(0.32)
        .height(190.0)
}
// end

// sample: Card, 150
fn card() -> impl View {
    Card::new(
        VStack::new((
            Text::new("Quarterly report").style(TextStyle::Headline),
            Text::new("A grouped surface, lifted off the page.").foreground(Semantic::InkMuted),
        ))
        .spacing(4.0),
    )
}
// end

// sample: GlassCard, 150
fn glass_card() -> impl View {
    GlassCard::new(Text::new("Translucent, for Raven Glass compositions."))
}
// end

// sample: Section, 130
fn section() -> impl View {
    Section::new(
        "Notifications",
        (Text::new("Grouped content under a title.").foreground(Semantic::InkMuted),),
    )
}
// end

// sample: Divider, 110
fn divider() -> impl View {
    VStack::new((Text::new("Above"), Divider::horizontal(), Text::new("Below")))
        .spacing(12.0)
        .align(Align::Stretch)
}
// end

// sample: Footer, 170
fn footer() -> impl View {
    Footer::new()
        .left((Footer::text("© 2026 Raven Finance"),))
        .center((Footer::link("Privacy"), Footer::link("Terms"), Footer::link("Help")))
        .right((Footer::text("v27.1"), Badge::new("Online").tone(Tone::Success)))
}
// end

// sample: AspectRatio, 190
fn aspect_ratio() -> impl View {
    VStack::new((AspectRatio::new(16.0 / 9.0, Image::new(picture(1.0)).stretch()),))
        .width(320.0)
        .align(Align::Stretch)
}
// end

// sample: Collapse, 170
fn collapse() -> impl View {
    let open = State::new(true);
    let flip = open.clone();
    view_fn(move || {
        let toggle = flip.clone();
        VStack::new((
            Button::new("Toggle").style(ButtonStyle::Secondary).on_click(move || toggle.update(|o| *o = !*o)),
            Collapse::new(open.get(), Text::new("The height animates, whatever the content.")),
        ))
        .spacing(12.0)
    })
}
// end

// sample: Text, 170
fn text() -> impl View {
    VStack::new((
        Text::new("Display").style(TextStyle::Display),
        Text::new("Headline").style(TextStyle::Headline),
        Text::new("Muted body text").foreground(Semantic::InkMuted),
    ))
    .spacing(6.0)
}
// end

// sample: Alert, 220
fn alert() -> impl View {
    VStack::new((
        Alert::new("Deployment finished", "The site is live.").tone(Tone::Success),
        Alert::new("Payment failed", "Update your card.").tone(Tone::Danger),
    ))
    .spacing(10.0)
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
    ))
    .spacing(8.0)
    .wrap()
}
// end

// sample: Avatar, 90
fn avatar() -> impl View {
    HStack::new((Avatar::new("Ana Lima"), Avatar::new("Bruno Reis"), Avatar::new("Carla Dias").size(44.0)))
        .spacing(8.0)
        .align(Align::Center)
}
// end

// sample: Kbd, 80
fn kbd() -> impl View {
    HStack::new((Kbd::new("⌘"), Kbd::new("Shift"), Kbd::new("K"))).spacing(6.0)
}
// end

// sample: ProgressBar, 110
fn progress_bar() -> impl View {
    VStack::new((ProgressBar::new(0.25), ProgressBar::new(0.9).tone(Tone::Success)))
        .spacing(14.0)
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
    .spacing(14.0)
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

// sample: Image, 150
fn image() -> impl View {
    HStack::new((
        Image::new(picture(1.0)).size(160.0, 90.0).corner_radius(12.0),
        Image::new(picture(0.3)).size(90.0, 90.0).corner_radius(45.0),
    ))
    .spacing(12.0)
}
// end

// sample: Carousel, 280
fn carousel() -> impl View {
    let slides: Vec<AnyView> = [1.0, 0.6, 0.2]
        .into_iter()
        .map(|hue| Box::new(Image::new(picture(hue)).stretch().height(220.0)) as AnyView)
        .collect();
    Carousel::new(slides).height(220.0)
}
// end

// sample: Canvas, 170
fn canvas() -> impl View {
    Canvas::new(|frame, scene| {
        for i in 0..8 {
            let x = frame.x() + i as f32 * 28.0;
            let height = 20.0 + i as f32 * 12.0;
            let bar = Rect::new(x, frame.max_y() - height, 22.0, height);
            scene.push_quad(Quad::new(bar, Color::rgb(0.3, 0.4, 0.93)).radius(4.0));
        }
    })
    .size(240.0, 130.0)
}
// end

/// Shows one example alone, for the frame on a component's page.
pub struct SampleApp {
    content: AnyView,
}

impl std::fmt::Debug for SampleApp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SampleApp").finish_non_exhaustive()
    }
}

impl SampleApp {
    /// The example of a component, if it has one.
    pub fn new(name: &str) -> Option<Self> {
        sample(name).map(|content| Self { content })
    }
}

impl View for SampleApp {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let content = cx.child(self.content.as_ref());
        cx.container(
            LayoutStyle {
                grow: 1.0,
                padding: Insets::uniform(24.0),
                align: Align::Stretch,
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &[content],
            Paint::Quad(QuadPaint::new(theme.color(Semantic::Canvas))),
        )
    }
}

/// Components without an example would make the site's list a promise it does not keep.
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_unique_and_every_example_builds() {
        let unique: std::collections::BTreeSet<&&str> = NAMES.iter().collect();
        assert_eq!(unique.len(), NAMES.len());
        for name in NAMES {
            let app = SampleApp::new(name).unwrap_or_else(|| panic!("{name}"));
            let mut ui = Ui::new(app, Theme::raven_dark());
            ui.set_viewport(Size::new(800.0, 500.0), 1.0);
            ui.update();
        }
        assert!(SampleApp::new("Nope").is_none());
    }
}
