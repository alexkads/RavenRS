//! The RavenRS component gallery: every built-in component, grouped by
//! category, with live state. Runs natively and in the browser.

use raven::prelude::*;

/// Categories shown in the sidebar, in order.
const PAGES: &[(IconName, &str)] = &[
    (IconName::LayoutDashboard, "Overview"),
    (IconName::MousePointer, "Buttons"),
    (IconName::SlidersHorizontal, "Forms"),
    (IconName::PanelLeft, "Navigation"),
    (IconName::Layers, "Overlays"),
    (IconName::Table, "Data display"),
    (IconName::Bell, "Feedback"),
    (IconName::ChartBar, "Charts"),
    (IconName::Sparkles, "Icons"),
];

/// All gallery state, created once; pages are rebuilt from it.
#[derive(Debug)]
pub struct Gallery {
    page: State<usize>,
    clicks: State<u32>,
    name: State<String>,
    password: State<String>,
    plan: State<usize>,
    notifications: State<bool>,
    autosave: State<bool>,
    terms: State<bool>,
    newsletter: State<bool>,
    density: State<usize>,
    volume: State<f32>,
    seats: State<i32>,
    range: State<usize>,
    tab: State<usize>,
    popover: State<bool>,
    dialog: State<bool>,
    last_action: State<String>,
}

impl Default for Gallery {
    fn default() -> Self {
        Self::new()
    }
}

impl Gallery {
    pub fn new() -> Self {
        Self {
            page: State::new(0),
            clicks: State::new(0),
            name: State::new(String::new()),
            password: State::new(String::new()),
            plan: State::new(1),
            notifications: State::new(true),
            autosave: State::new(false),
            terms: State::new(true),
            newsletter: State::new(false),
            density: State::new(1),
            volume: State::new(0.62),
            seats: State::new(5),
            range: State::new(1),
            tab: State::new(0),
            popover: State::new(false),
            dialog: State::new(false),
            last_action: State::new("Nothing yet".into()),
        }
    }
}

/// A titled card holding one demo.
fn demo<V: View + 'static>(title: &str, description: &str, content: V) -> impl View + use<V> {
    Card::new(
        VStack::new((
            VStack::new((
                Text::new(title).style(TextStyle::Headline),
                Text::new(description).style(TextStyle::Callout).foreground(Semantic::InkMuted),
            ))
            .spacing(2.0),
            content,
        ))
        .spacing(18.0)
        .align(Align::Stretch),
    )
    .corner_radius(18.0)
}

fn page_header(title: &str, description: &str) -> impl View + use<> {
    VStack::new((
        Text::new(title).style(TextStyle::Display),
        Text::new(description).foreground(Semantic::InkMuted),
    ))
    .spacing(6.0)
}

impl Gallery {
    fn sidebar(&self) -> impl View + use<> {
        let current = self.page.get();
        let mut items: Vec<AnyView> = vec![
            Box::new(
                HStack::new((
                    Icon::new(IconName::Sparkles).size(18.0).color(Semantic::Accent),
                    Text::new("RavenRS").style(TextStyle::Headline),
                    Spacer::new(),
                    Badge::new("v0.1"),
                ))
                .spacing(10.0)
                .align(Align::Center)
                .padding((4.0, 10.0)),
            ),
            Box::new(SidebarSection::new("Components")),
        ];
        for (i, (icon, label)) in PAGES.iter().enumerate() {
            let page = self.page.clone();
            items.push(Box::new(
                SidebarItem::new(*icon, *label).selected(i == current).on_click(move || page.set(i)),
            ));
        }
        items.push(Box::new(SidebarSection::new("Resources")));
        items.push(Box::new(SidebarItem::new(IconName::FileText, "Architecture")));
        items.push(Box::new(SidebarItem::new(IconName::Palette, "Design system")));
        Sidebar::new(items)
    }

    fn toolbar(&self, narrow: bool) -> AnyView {
        if narrow {
            let pages: Vec<&str> = PAGES.iter().map(|(_, label)| *label).collect();
            return Box::new(
                VStack::new((Select::new(self.page.binding(), pages).width(240.0), ThemeSwitcher))
                    .spacing(10.0)
                    .padding((12.0, 16.0)),
            );
        }
        let (_, title) = PAGES[self.page.get().min(PAGES.len() - 1)];
        Box::new(
            HStack::new((
                Breadcrumb::new(["Components", title]),
                Spacer::new(),
                Tooltip::new(IconButton::new(IconName::Search), "Search (⌘K)"),
                Tooltip::new(IconButton::new(IconName::Bell), "Notifications"),
                ThemeSwitcher,
            ))
            .spacing(8.0)
            .align(Align::Center)
            .padding((14.0, 32.0)),
        )
    }

    fn page(&self, width: f32) -> AnyView {
        match self.page.get() {
            0 => Box::new(self.overview(width)),
            1 => Box::new(self.buttons(width)),
            2 => Box::new(self.forms(width)),
            3 => Box::new(self.navigation(width)),
            4 => Box::new(self.overlays(width)),
            5 => Box::new(self.data_display(width)),
            6 => Box::new(self.feedback(width)),
            7 => Box::new(self.charts(width)),
            _ => Box::new(self.icons(width)),
        }
    }

    fn overview(&self, width: f32) -> impl View + use<> {
        VStack::new((
            page_header(
                "Overview",
                "A dashboard assembled only from RavenRS components. Every pixel is drawn on the GPU.",
            ),
            Grid::new(
                columns(width, 4),
                vec![
                    stat_card(Stat::new("Revenue", "R$ 128.450").delta("+12.4%", true)),
                    stat_card(Stat::new("Active users", "8,214").delta("+3.1%", true)),
                    stat_card(Stat::new("Churn", "1.8%").delta("-0.4%", false)),
                    stat_card(Stat::new("Uptime", "99.98%").delta("+0.02%", true)),
                ],
            ),
            Grid::new(
                columns(width, 2),
                (
                    demo(
                        "Revenue",
                        "Last 30 days",
                        AreaChart::new([
                            12.0, 14.0, 13.0, 17.0, 16.0, 21.0, 19.0, 24.0, 23.0, 27.0, 26.0, 31.0,
                        ])
                        .height(160.0),
                    ),
                    demo(
                        "Sign-ups per weekday",
                        "This week",
                        BarChart::new([
                            ("Mon", 42.0),
                            ("Tue", 58.0),
                            ("Wed", 51.0),
                            ("Thu", 73.0),
                            ("Fri", 66.0),
                            ("Sat", 29.0),
                            ("Sun", 24.0),
                        ])
                        .height(140.0),
                    ),
                ),
            ),
            demo(
                "Top accounts",
                "Sorted by monthly recurring revenue",
                Table::new([
                    Column::new("Account").weight(2.0),
                    Column::new("Plan"),
                    Column::new("Seats").numeric(),
                    Column::new("MRR").numeric(),
                ])
                .row(["Northwind Labs", "Enterprise", "240", "R$ 48.000"])
                .row(["Aurora Studio", "Business", "86", "R$ 17.200"])
                .row(["Kestrel Health", "Business", "64", "R$ 12.800"])
                .row(["Lumen Data", "Team", "18", "R$ 3.600"]),
            ),
        ))
        .spacing(24.0)
        .align(Align::Stretch)
    }

    fn buttons(&self, _width: f32) -> impl View + use<> {
        let clicks = self.clicks.clone();
        let inc = move || clicks.update(|n| *n += 1);
        let (a, b, c, d) = (inc.clone(), inc.clone(), inc.clone(), inc);
        VStack::new((
            page_header(
                "Buttons",
                "Actions in four styles and three sizes, with icons, focus rings and keyboard support.",
            ),
            demo(
                "Styles",
                "One primary action per view; the rest are secondary or ghost.",
                VStack::new((
                    HStack::new((
                        Button::new("Get started").on_click(a),
                        Button::new("Secondary").style(ButtonStyle::Secondary).on_click(b),
                        Button::new("Ghost").style(ButtonStyle::Ghost).on_click(c),
                        Button::new("Delete").style(ButtonStyle::Danger).icon(IconName::Trash2).on_click(d),
                    ))
                    .spacing(12.0)
                    .wrap(),
                    Text::bound(&self.clicks, |n| format!("Clicked {n} times. Try Tab and Enter too."))
                        .style(TextStyle::Callout)
                        .foreground(Semantic::InkMuted),
                ))
                .spacing(14.0),
            ),
            demo(
                "Sizes and icons",
                "Small for dense toolbars, large for primary calls to action.",
                HStack::new((
                    Button::new("Small").size(ButtonSize::Small).icon(IconName::Plus),
                    Button::new("Medium").icon(IconName::Download),
                    Button::new("Large").size(ButtonSize::Large).trailing_icon(IconName::ArrowRight),
                    Button::new("Disabled").style(ButtonStyle::Secondary).disabled(true),
                ))
                .spacing(12.0)
                .align(Align::Center)
                .wrap(),
            ),
            demo(
                "Icon buttons",
                "Square ghost buttons for toolbars. Hover them for a tooltip.",
                HStack::new((
                    Tooltip::new(IconButton::new(IconName::Pencil), "Edit"),
                    Tooltip::new(IconButton::new(IconName::Copy), "Duplicate"),
                    Tooltip::new(IconButton::new(IconName::Share2), "Share"),
                    Tooltip::new(
                        IconButton::new(IconName::Settings).style(ButtonStyle::Secondary),
                        "Settings",
                    ),
                    Tooltip::new(IconButton::new(IconName::Plus).style(ButtonStyle::Primary), "New item"),
                ))
                .spacing(6.0)
                .wrap(),
            ),
        ))
        .spacing(24.0)
        .align(Align::Stretch)
    }

    fn forms(&self, width: f32) -> impl View + use<> {
        let volume = self.volume.clone();
        VStack::new((
            page_header(
                "Forms",
                "Inputs bound to state. Every control works with the mouse and the keyboard.",
            ),
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
                            ),
                            SegmentedControl::new(
                                self.density.binding(),
                                ["Compact", "Default", "Comfortable"],
                            ),
                        ))
                        .spacing(14.0),
                    ),
                    demo(
                        "Switches and checkboxes",
                        "Binary settings. The whole row is clickable.",
                        VStack::new((
                            Toggle::new(self.notifications.binding()).label("Push notifications"),
                            Toggle::new(self.autosave.binding()).label("Autosave drafts"),
                            Checkbox::new("I accept the terms", self.terms.binding()),
                            Checkbox::new("Send me product news", self.newsletter.binding()),
                        ))
                        .spacing(14.0),
                    ),
                    demo(
                        "Radio group",
                        "One choice among a few visible options.",
                        RadioGroup::new(
                            self.plan.binding(),
                            ["Starter: free", "Team: R$ 49 / seat", "Enterprise: talk to us"],
                        ),
                    ),
                    demo(
                        "Slider",
                        "Drag the thumb, or focus it and use the arrow keys.",
                        VStack::new((
                            Slider::new(self.volume.binding()),
                            Text::dynamic(move || format!("Volume: {:.0}%", volume.get() * 100.0))
                                .style(TextStyle::Callout)
                                .foreground(Semantic::InkMuted),
                        ))
                        .spacing(10.0)
                        .align(Align::Stretch),
                    ),
                    demo(
                        "Stepper",
                        "Small integer quantities with bounds.",
                        HStack::new((
                            Stepper::new(self.seats.binding()).range(1, 50),
                            Text::bound(&self.seats, |s| format!("{s} seats")).foreground(Semantic::InkMuted),
                        ))
                        .spacing(12.0)
                        .align(Align::Center)
                        .wrap(),
                    ),
                ),
            ),
        ))
        .spacing(24.0)
        .align(Align::Stretch)
    }

    fn navigation(&self, _width: f32) -> impl View + use<> {
        VStack::new((
            page_header("Navigation", "Tabs, breadcrumbs, sidebars and disclosures."),
            demo(
                "Tabs",
                "Only the selected tab's content is built.",
                Tabs::new(self.tab.binding())
                    .tab(
                        "Activity",
                        List::new((
                            ListItem::new("Ana Lima merged #482")
                                .subtitle("raven-render: clip stack")
                                .avatar("Ana Lima")
                                .trailing("2m"),
                            ListItem::new("Bruno Reis opened #483")
                                .subtitle("Add DatePicker")
                                .avatar("Bruno Reis")
                                .trailing("1h"),
                        )),
                    )
                    .tab(
                        "Settings",
                        Alert::new("Settings", "Each tab keeps its own component state.").tone(Tone::Neutral),
                    )
                    .tab("Billing", Stat::new("Next invoice", "R$ 1.240").delta("+R$ 80", true)),
            ),
            demo(
                "Breadcrumb",
                "Where the user is.",
                Breadcrumb::new(["Workspace", "Projects", "RavenRS", "Settings"]),
            ),
            demo(
                "Accordion",
                "Disclosures remember whether they are open.",
                Accordion::new(vec![
                    Disclosure::new(
                        "What platforms does RavenRS support?",
                        Text::new("Windows, macOS and Linux natively, and browsers with WebGPU.")
                            .foreground(Semantic::InkMuted),
                    )
                    .open(true),
                    Disclosure::new(
                        "Can I build my own components?",
                        Text::new("Yes: compose with Component, or implement View for full control.")
                            .foreground(Semantic::InkMuted),
                    ),
                    Disclosure::new(
                        "Is it accessible?",
                        Text::new(
                            "Keyboard focus works today; screen readers through AccessKit are planned.",
                        )
                        .foreground(Semantic::InkMuted),
                    ),
                ]),
            ),
        ))
        .spacing(24.0)
        .align(Align::Stretch)
    }

    fn overlays(&self, width: f32) -> impl View + use<> {
        let (open_popover, open_dialog) = (self.popover.clone(), self.dialog.clone());
        let log = |label: &'static str, state: &State<String>| {
            let s = state.clone();
            move || s.set(format!("You chose “{label}”."))
        };
        let close = self.dialog.clone();
        let confirm = self.dialog.clone();
        let done = self.last_action.clone();
        VStack::new((
            page_header("Overlays", "Floating layers above the UI. Click outside or press Escape to close them."),
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
                            .spacing(10.0)
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

    fn data_display(&self, width: f32) -> impl View + use<> {
        VStack::new((
            page_header("Data display", "Lists, tables, badges, avatars and keyboard keys."),
            demo(
                "List",
                "Rows with avatars or icons, subtitles and trailing details.",
                List::new((
                    ListItem::new("Ana Lima")
                        .subtitle("ana@northwind.dev")
                        .avatar("Ana Lima")
                        .trailing("Owner"),
                    ListItem::new("Bruno Reis")
                        .subtitle("bruno@northwind.dev")
                        .avatar("Bruno Reis")
                        .trailing("Admin"),
                    ListItem::new("Carla Dias")
                        .subtitle("carla@northwind.dev")
                        .avatar("Carla Dias")
                        .trailing("Member"),
                    ListItem::new("Quarterly report.pdf")
                        .subtitle("2.4 MB · updated today")
                        .icon(IconName::FileText)
                        .on_click(|| {}),
                )),
            ),
            demo(
                "Table",
                "Weighted columns; numbers align right.",
                Table::new([
                    Column::new("Invoice").weight(1.4),
                    Column::new("Customer").weight(2.0),
                    Column::new("Status"),
                    Column::new("Amount").numeric(),
                ])
                .row(["INV-1042", "Northwind Labs", "Paid", "R$ 4.800"])
                .row(["INV-1043", "Aurora Studio", "Pending", "R$ 1.720"])
                .row(["INV-1044", "Kestrel Health", "Overdue", "R$ 2.560"]),
            ),
            Grid::new(
                columns(width, 2),
                (
                    demo(
                        "Badges",
                        "Status always pairs color with a word or an icon.",
                        HStack::new((
                            Badge::new("Neutral"),
                            Badge::new("New").tone(Tone::Accent),
                            Badge::new("Paid").tone(Tone::Success).icon(IconName::Check),
                            Badge::new("Pending").tone(Tone::Warning),
                            Badge::new("Overdue").tone(Tone::Danger),
                        ))
                        .spacing(8.0)
                        .wrap(),
                    ),
                    demo(
                        "Avatars and keys",
                        "Initials on a color derived from the name.",
                        HStack::new((
                            Avatar::new("Ana Lima"),
                            Avatar::new("Bruno Reis"),
                            Avatar::new("Carla Dias").size(44.0),
                            Spacer::new().min(12.0),
                            Kbd::new("⌘"),
                            Kbd::new("Shift"),
                            Kbd::new("K"),
                        ))
                        .spacing(8.0)
                        .align(Align::Center)
                        .wrap(),
                    ),
                ),
            ),
        ))
        .spacing(24.0)
        .align(Align::Stretch)
    }

    fn feedback(&self, width: f32) -> impl View + use<> {
        VStack::new((
            page_header("Feedback", "Alerts, progress and loading placeholders."),
            demo(
                "Alerts",
                "Tinted messages with an icon for each tone.",
                VStack::new((
                    Alert::new("New version available", "RavenRS 0.2 adds images and animations.")
                        .tone(Tone::Accent),
                    Alert::new("Deployment finished", "The site is live on GitHub Pages.")
                        .tone(Tone::Success),
                    Alert::new("Storage almost full", "You are using 92% of your quota.").tone(Tone::Warning),
                    Alert::new("Payment failed", "Update your card to keep your workspace active.")
                        .tone(Tone::Danger),
                ))
                .spacing(10.0)
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
                        .spacing(14.0),
                    ),
                ),
            ),
        ))
        .spacing(24.0)
        .align(Align::Stretch)
    }

    fn charts(&self, width: f32) -> impl View + use<> {
        VStack::new((
            page_header("Charts", "Drawn with the same GPU primitives as everything else, through Canvas."),
            demo(
                "Area chart",
                "Smoothed series with a gradient fill.",
                AreaChart::new([
                    8.0, 11.0, 9.0, 14.0, 13.0, 18.0, 17.0, 22.0, 19.0, 25.0, 24.0, 29.0, 27.0, 33.0,
                ])
                .height(200.0),
            ),
            Grid::new(
                columns(width, 2),
                (
                    demo(
                        "Bar chart",
                        "Monthly active users, in thousands.",
                        BarChart::new([
                            ("Jan", 31.0),
                            ("Feb", 36.0),
                            ("Mar", 42.0),
                            ("Apr", 40.0),
                            ("May", 51.0),
                            ("Jun", 58.0),
                        ]),
                    ),
                    demo(
                        "Error rate",
                        "A falling series in the danger tone.",
                        AreaChart::new([9.0, 8.0, 8.5, 6.0, 6.5, 4.0, 4.2, 3.0, 2.5, 2.0])
                            .tone(Semantic::Danger)
                            .height(180.0),
                    ),
                ),
            ),
        ))
        .spacing(24.0)
        .align(Align::Stretch)
    }

    fn icons(&self, width: f32) -> impl View + use<> {
        let tiles: Vec<AnyView> = IconName::ALL
            .iter()
            .map(|icon| -> AnyView {
                Box::new(
                    VStack::new((
                        Icon::new(*icon).size(22.0),
                        Text::new(icon.name()).style(TextStyle::Caption).foreground(Semantic::InkMuted),
                    ))
                    .spacing(10.0)
                    .align(Align::Center)
                    .padding((16.0, 6.0))
                    .background(Semantic::Surface)
                    .corner_radius(12.0)
                    .border(1.0, Semantic::Border),
                )
            })
            .collect();
        VStack::new((
            page_header(
                "Icons",
                "112 Lucide icons (ISC license) built in, drawn as font glyphs so they take any size and color.",
            ),
            Grid::new(columns(width, 6), tiles).spacing(10.0),
        ))
        .spacing(24.0)
        .align(Align::Stretch)
    }
}

/// How many of `max` columns fit in `width` logical pixels (cards need ~240px).
fn columns(width: f32, max: u16) -> u16 {
    ((width / 240.0).floor() as u16).clamp(1, max)
}

fn stat_card(stat: Stat) -> AnyView {
    Box::new(Card::new(stat).corner_radius(18.0))
}

impl Component for Gallery {
    fn body(&self) -> impl View {
        responsive(move |size: Size| -> AnyView {
            let narrow = size.width < 820.0;
            let content_width =
                if narrow { size.width - 32.0 } else { (size.width - 248.0 - 64.0).min(1080.0) };
            let padding = if narrow { (20.0, 16.0) } else { (28.0, 32.0) };
            let main = VStack::new((
                self.toolbar(narrow),
                Divider::horizontal(),
                // Keyed by page, so each page opens scrolled to the top with fresh local state.
                Keyed::new(
                    self.page.get() as u64,
                    ScrollView::new((PageBody(self.page(content_width)),)).padding(padding).spacing(0.0),
                ),
            ))
            .spacing(0.0)
            .align(Align::Stretch)
            .grow(1.0);
            if narrow {
                Box::new(main)
            } else {
                Box::new(HStack::new((self.sidebar(), main)).spacing(0.0).align(Align::Stretch).grow(1.0))
            }
        })
    }
}

struct PageBody(AnyView);

impl View for PageBody {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let content = cx.child(self.0.as_ref());
        cx.container(
            raven::layout::LayoutStyle {
                max_width: Some(1080.0),
                align: Align::Stretch,
                ..raven::layout::LayoutStyle::stack(raven::layout::Axis::Vertical)
            },
            &[content],
            Paint::None,
        )
    }
}

#[cfg(target_arch = "wasm32")]
mod web_entry {
    use std::cell::RefCell;

    use wasm_bindgen::prelude::*;

    thread_local! {
        static APP: RefCell<Option<raven::web::WebApp>> = const { RefCell::new(None) };
    }

    #[wasm_bindgen(start)]
    pub fn start() {
        console_error_panic_hook::set_once();
        wasm_bindgen_futures::spawn_local(async {
            match raven::web::start(super::Gallery::new(), raven::web::WebOptions::new("raven-canvas")).await
            {
                Ok(app) => APP.with(|a| *a.borrow_mut() = Some(app)),
                Err(e) => web_sys::console::error_1(&e.to_string().into()),
            }
        });
    }

    #[wasm_bindgen]
    pub fn stop() {
        APP.with(|a| a.borrow_mut().take());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_page_builds() {
        let gallery = Gallery::new();
        let page = gallery.page.clone();
        let mut ui = Ui::new(gallery, Theme::raven_dark());
        ui.set_viewport(Size::new(1280.0, 820.0), 1.0);
        for i in 0..PAGES.len() {
            page.set(i);
            ui.update();
            assert!(ui.element_count() > 40, "page {i} built only {} elements", ui.element_count());
        }
    }

    fn first_glyph_y(ui: &Ui) -> f32 {
        ui.scene()
            .primitives()
            .iter()
            .filter_map(|p| match p {
                raven::core::Primitive::Glyphs(g) => g.glyphs.first().map(|g| g.y),
                _ => None,
            })
            .fold(f32::MIN, f32::max)
    }

    #[test]
    fn pages_scroll_on_desktop_and_phone_layouts() {
        for size in [Size::new(1280.0, 720.0), Size::new(375.0, 812.0)] {
            let mut ui = Ui::new(Gallery::new(), Theme::raven_dark());
            ui.set_viewport(size, 1.0);
            ui.update();
            let before = first_glyph_y(&ui);
            let at = Point::new(size.width - 60.0, size.height - 100.0);
            assert!(
                ui.handle_event(&raven::core::Event::Scroll { position: at, delta: Point::new(0.0, -200.0) }),
                "the page scrolls at {size:?}"
            );
            assert!(
                (first_glyph_y(&ui) - (before - 200.0)).abs() < 0.5,
                "content moved up by 200px at {size:?}"
            );
        }
    }
}
