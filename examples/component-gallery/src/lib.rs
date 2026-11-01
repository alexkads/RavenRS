//! The RavenRS component gallery: every built-in component, grouped by
//! category, with live state. Runs natively and in the browser.

use ravenrs::layout::{Axis, LayoutStyle};
use ravenrs::prelude::*;

pub mod samples;

fn first_of_plan() -> Date {
    Date::new(2026, 11, 1).unwrap_or_else(|| Date::from_days(0))
}

fn plan_events() -> Vec<SchedulerEvent> {
    let at = |day: i64| TimePoint::from_date(first_of_plan()).plus_days(day);
    vec![
        SchedulerEvent::new(1, 1, at(1), at(5), "Vacation").tone(Tone::Success),
        SchedulerEvent::new(2, 1, at(3), at(8), "Workshop"),
        SchedulerEvent::new(3, 2, at(2), at(6), "On call").tone(Tone::Warning),
        SchedulerEvent::new(4, 3, at(4), at(5), "Review").tone(Tone::Danger).resizable(false),
    ]
}

/// Categories shown in the sidebar, in order.
const PAGES: &[(IconName, &str)] = &[
    (IconName::LayoutDashboard, "Overview"),
    (IconName::MousePointer, "Buttons"),
    (IconName::SlidersHorizontal, "Forms"),
    (IconName::PanelLeft, "Navigation"),
    (IconName::Layers, "Overlays"),
    (IconName::Table, "Data display"),
    (IconName::Database, "Data grid"),
    (IconName::Image, "Images"),
    (IconName::Calendar, "Pickers & forms"),
    (IconName::Columns3, "Layout & navigation"),
    (IconName::Rows3, "Ribbon"),
    (IconName::Bell, "Feedback"),
    (IconName::Zap, "Motion"),
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
    images: Vec<ImageData>,
    grid: GridData,
    grid_selection: State<std::collections::BTreeSet<usize>>,
    export_info: State<String>,
    date: State<Option<Date>>,
    plan_axis: State<TimeAxis>,
    plan_events: State<Vec<SchedulerEvent>>,
    plan_rule: State<RecurrenceRule>,
    agenda_month: State<Date>,
    country: State<Option<usize>>,
    otp: State<String>,
    email: State<String>,
    company: State<String>,
    submitted: State<bool>,
    sheet: State<bool>,
    sidebar: SidebarState,
    sidebar_collapse: State<usize>,
    sidebar_variant: State<usize>,
    sidebar_side: State<usize>,
    sidebar_picked: State<String>,
    palette: State<bool>,
    toasts: Toasts,
    text_styles: State<std::collections::BTreeSet<usize>>,
    align: State<usize>,
    pinned: State<bool>,
    page_number: State<usize>,
    motion_side: State<bool>,
    motion_dialog: State<bool>,
    motion_segment: State<usize>,
    motion_tab: State<usize>,
    motion_toggle: State<bool>,
    motion_check: State<bool>,
    motion_progress: State<f32>,
    ribbon_bold: State<bool>,
    ribbon_italic: State<bool>,
    ribbon_ruler: State<bool>,
    ribbon_grid: State<bool>,
    ribbon_table: State<bool>,
    ribbon_zoom: State<u32>,
    ribbon_search: State<String>,
    swatch: State<usize>,
}

impl Default for Gallery {
    fn default() -> Self {
        Self::new()
    }
}

impl Gallery {
    /// Creates every page's state, the sample data and the generated images.
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
            swatch: State::new(0),
            volume: State::new(0.62),
            seats: State::new(5),
            range: State::new(1),
            tab: State::new(0),
            popover: State::new(false),
            dialog: State::new(false),
            last_action: State::new("Nothing yet".into()),
            images: art::all(),
            grid: GridData::new(sample_rows(100_000)),
            grid_selection: State::new(Default::default()),
            export_info: State::new(String::new()),
            date: State::new(Date::new(2026, 10, 31)),
            plan_axis: State::new(TimeAxis::days(first_of_plan(), 30)),
            plan_events: State::new(plan_events()),
            plan_rule: State::new(RecurrenceRule::new(Frequency::Weekly).on_weekdays([1, 4])),
            agenda_month: State::new(first_of_plan()),
            country: State::new(None),
            otp: State::new(String::new()),
            email: State::new(String::new()),
            company: State::new(String::new()),
            submitted: State::new(false),
            sheet: State::new(false),
            sidebar: SidebarState::new(true),
            sidebar_collapse: State::new(0),
            sidebar_variant: State::new(0),
            sidebar_side: State::new(0),
            sidebar_picked: State::new("Overview".into()),
            palette: State::new(false),
            toasts: Toasts::new(),
            text_styles: State::new(Default::default()),
            align: State::new(0),
            pinned: State::new(false),
            page_number: State::new(4),
            motion_side: State::new(false),
            motion_dialog: State::new(false),
            motion_segment: State::new(0),
            motion_tab: State::new(0),
            motion_toggle: State::new(true),
            motion_check: State::new(false),
            motion_progress: State::new(0.1),
            ribbon_bold: State::new(true),
            ribbon_italic: State::new(false),
            ribbon_ruler: State::new(true),
            ribbon_grid: State::new(false),
            ribbon_table: State::new(true),
            ribbon_zoom: State::new(100),
            ribbon_search: State::new(String::new()),
        }
    }
}

/// A titled card holding one demo.
/// A puck that crosses a rail on a spring when `0` flips.
struct MotionTrack(State<bool>);

impl View for MotionTrack {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let puck = cx.container(
            LayoutStyle { width: Some(44.0), height: Some(28.0), ..LayoutStyle::leaf() },
            &[],
            Paint::Quad(QuadPaint {
                corner_radius: theme.radii.md,
                ..QuadPaint::new(theme.color(Semantic::Accent))
            }),
        );
        cx.motion(puck, Motion::new().layout(Transition::Spring(Spring::from_response(0.45, 0.8))));
        let rail = cx.container(
            LayoutStyle {
                padding: Insets::uniform(4.0),
                justify: if self.0.get() { Justify::End } else { Justify::Start },
                align_self: Some(Align::Stretch),
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &[puck],
            Paint::Quad(QuadPaint {
                corner_radius: theme.radii.lg,
                ..QuadPaint::new(theme.color(Semantic::Control))
            }),
        );
        cx.clip(rail);
        rail
    }
}

/// Turns the theme's reduced-motion token on and off.
struct ReducedMotionSwitch;

impl View for ReducedMotionSwitch {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = cx.theme_state();
        let on = cx.reduced_motion();
        cx.child(
            &Button::new(if on { "Reduced motion is on" } else { "Try reduced motion" })
                .style(ButtonStyle::Ghost)
                .size(ButtonSize::Small)
                .on_click(move || {
                    theme.update(|t| t.interaction.reduced_motion = !t.interaction.reduced_motion)
                }),
        )
    }
}

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
                    Badge::new("v27.1"),
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

    /// Cmd/Ctrl+K: jump to any page or switch theme.
    fn palette_view(&self) -> impl View + use<> {
        let mut palette = CommandPalette::new(self.palette.binding());
        for (i, (icon, label)) in PAGES.iter().enumerate() {
            let page = self.page.clone();
            palette = palette.command(
                Command::new(format!("Go to {label}"), move || page.set(i)).icon(*icon).group("Pages"),
            );
        }
        let toasts = self.toasts.clone();
        palette.command(
            Command::new("Show a toast", move || {
                toasts.show("Hello from the palette", "Commands can run any code.", Tone::Accent)
            })
            .icon(IconName::BellRing)
            .group("Actions"),
        )
    }

    fn toolbar(&self, narrow: bool) -> AnyView {
        if narrow {
            let pages: Vec<&str> = PAGES.iter().map(|(_, label)| *label).collect();
            return Box::new(
                VStack::new((
                    Select::new(self.page.binding(), pages).width(240.0).label("Page"),
                    ThemeSwitcher,
                ))
                .spacing(10.0)
                .padding((12.0, 16.0)),
            );
        }
        let (_, title) = PAGES[self.page.get().min(PAGES.len() - 1)];
        Box::new(
            HStack::new((
                Breadcrumb::new(["Components", title]),
                Spacer::new(),
                Tooltip::new(
                    IconButton::new(IconName::Search).on_click({
                        let p = self.palette.clone();
                        move || p.set(true)
                    }),
                    "Search (⌘K)",
                ),
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
            6 => Box::new(self.data_grid(width)),
            7 => Box::new(self.images(width)),
            8 => Box::new(self.pickers(width)),
            9 => Box::new(self.layout_nav(width)),
            10 => Box::new(self.ribbon_page(width)),
            11 => Box::new(self.feedback(width)),
            12 => Box::new(self.motion(width)),
            13 => Box::new(self.charts(width)),
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
                            )
                            .label("Date range"),
                            SegmentedControl::new(
                                self.density.binding(),
                                ["Compact", "Default", "Comfortable"],
                            )
                            .label("Density"),
                        ))
                        .spacing(14.0),
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
                        .spacing(14.0),
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
                        ))
                        .spacing(10.0)
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
        ))
        .spacing(24.0)
        .align(Align::Stretch)
    }

    fn sidebar_demo(&self) -> impl View + use<> {
        let picked = self.sidebar_picked.get();
        let row = |icon: IconName, label: &'static str| {
            let chosen = self.sidebar_picked.clone();
            SidebarItem::new(icon, label).selected(picked == label).on_click(move || chosen.set(label.into()))
        };
        let sub = |label: &'static str| {
            let chosen = self.sidebar_picked.clone();
            SidebarItem::text(label)
                .compact()
                .selected(picked == label)
                .on_click(move || chosen.set(label.into()))
        };
        let toasts = self.toasts.clone();
        let invite = move || toasts.show("Invite", "An action beside the row.", Tone::Accent);
        let toasts = self.toasts.clone();
        let add = move || toasts.show("New project", "An action on the group.", Tone::Accent);
        let collapse = match self.sidebar_collapse.get() {
            0 => SidebarCollapse::Icons,
            1 => SidebarCollapse::Hidden,
            _ => SidebarCollapse::Fixed,
        };
        let variant = match self.sidebar_variant.get() {
            0 => SidebarVariant::Standard,
            1 => SidebarVariant::Floating,
            _ => SidebarVariant::Inset,
        };
        let side = if self.sidebar_side.get() == 0 { Side::Left } else { Side::Right };
        let sidebar = Sidebar::new((
            SidebarGroup::new(
                "Workspace",
                (
                    row(IconName::LayoutDashboard, "Overview"),
                    row(IconName::Users, "Customers").badge(Badge::new("12").tone(Tone::Accent)).action(
                        IconButton::new(IconName::UserPlus)
                            .label("Invite a customer")
                            .size(ButtonSize::Small)
                            .on_click(invite),
                    ),
                    row(IconName::Settings, "Settings")
                        .item(sub("General"))
                        .item(sub("Billing"))
                        .item(sub("Members").count("4")),
                    row(IconName::Lock, "Audit log").disabled(true),
                ),
            )
            .action(
                IconButton::new(IconName::Plus).label("New project").size(ButtonSize::Small).on_click(add),
            ),
            SidebarGroup::new(
                "Projects",
                (row(IconName::FileText, "Reports"), row(IconName::Palette, "Brand")),
            )
            .collapsible(),
            SidebarGroup::new("Loading", (SidebarSkeleton::new(3),)).collapsible().open(false),
        ))
        .state(self.sidebar.clone())
        .collapsible(collapse)
        .variant(variant)
        .side(side)
        .narrow_below(768.0)
        .header(
            HStack::new((
                Icon::new(IconName::Sparkles).color(Semantic::Accent),
                Text::new("Acme Inc.").style(TextStyle::Headline),
            ))
            .spacing(10.0)
            .align(Align::Center)
            .padding((0.0, 10.0)),
        )
        .footer(
            HStack::new((Avatar::new("Ana Lima"), Text::new("Ana Lima").style(TextStyle::Callout)))
                .spacing(10.0)
                .align(Align::Center),
        );
        let page = VStack::new((
            SidebarTrigger::new(self.sidebar.clone()),
            Text::new(picked).style(TextStyle::Title),
            Text::new("The page takes whatever room the sidebar leaves.").foreground(Semantic::InkMuted),
        ))
        .spacing(12.0)
        .padding(16.0)
        .grow(1.0);
        let page: AnyView =
            if variant == SidebarVariant::Inset { Box::new(SidebarInset::new(page)) } else { Box::new(page) };
        let parts: Vec<AnyView> =
            if side == Side::Left { vec![Box::new(sidebar), page] } else { vec![page, Box::new(sidebar)] };
        VStack::new((
            HStack::new((
                SegmentedControl::new(self.sidebar_collapse.binding(), ["Icons", "Hidden", "Fixed"]),
                SegmentedControl::new(self.sidebar_variant.binding(), ["Standard", "Floating", "Inset"]),
                SegmentedControl::new(self.sidebar_side.binding(), ["Left", "Right"]),
            ))
            .spacing(12.0)
            .wrap(),
            HStack::new(parts).height(460.0).align(Align::Stretch),
        ))
        .spacing(12.0)
        .align(Align::Stretch)
    }

    fn navigation(&self, _width: f32) -> impl View + use<> {
        VStack::new((
            page_header("Navigation", "Tabs, breadcrumbs, sidebars and disclosures."),
            demo(
                "Sidebar",
                "Closes to icons or off the screen with the button or Cmd/Ctrl+B, with groups, submenus, actions and badges. Below 768 px it opens as a sheet.",
                self.sidebar_demo(),
            ),
            demo(
                "Tabs",
                "Only the selected tab's content is built.",
                Tabs::new(self.tab.binding())
                    .tab(
                        "Activity",
                        List::new((
                            ListItem::new("Ana Lima merged #482")
                                .subtitle("ravenrs-render: clip stack")
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

    fn more_overlays(&self, width: f32) -> impl View + use<> {
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
                                .spacing(10.0),
                            ),
                    )),
                ),
                demo(
                    "Splash screen",
                    "Covers the window while the app loads, then fades out. This one pretends to load for three seconds.",
                    samples::sample("SplashScreen"),
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
                        Button::new("Warning").style(ButtonStyle::Ghost).on_click(move || {
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

    fn pickers(&self, width: f32) -> impl View + use<> {
        let otp_toasts = self.toasts.clone();
        let submit = self.submitted.clone();
        let (email, company) = (self.email.clone(), self.company.clone());
        let form_toasts = self.toasts.clone();
        let today = Date::new(2026, 10, 31).unwrap_or_else(|| unreachable!());
        let show = self.submitted.get();
        VStack::new((
            page_header("Pickers & forms", "Dates, searchable selects, one-time codes and validated forms."),
            Grid::new(
                columns(width, 2),
                (
                    demo(
                        "Calendar",
                        "Pick a day; the ring marks today.",
                        Calendar::new(self.date.binding(), today).today(today),
                    ),
                    demo(
                        "Event calendar",
                        "A month with what is on each day.",
                        EventCalendar::new(self.agenda_month.binding(), self.date.binding()).today(today).events([
                            CalendarEvent::new(1, first_of_plan().add_days(3), "Rent").done(true),
                            CalendarEvent::new(2, first_of_plan().add_days(9), "Review").tone(Tone::Warning),
                            CalendarEvent::new(3, first_of_plan().add_days(14), "Salary").tone(Tone::Success),
                        ]),
                    ),
                    demo(
                        "Scheduler",
                        "Drag events along the time and between rows, pull their ends, draw on an empty stretch, zoom.",
                        {
                            let (moved, drawn) = (self.plan_events.clone(), self.plan_events.clone());
                            VStack::new((Scheduler::new(self.plan_axis.binding())
                                .resources([
                                    Resource::new(1, "Ana").subtitle("Design").group("Team"),
                                    Resource::new(2, "Bruno").subtitle("Engineering").group("Team"),
                                    Resource::new(3, "Meeting room").group("Rooms"),
                                ])
                                .events(self.plan_events.get())
                                .on_change(move |change| {
                                    moved.update(|events| {
                                        if let Some(event) = events.iter_mut().find(|e| Some(e.id) == change.id) {
                                            event.resource = change.resource;
                                            event.start = change.start;
                                            event.end = change.end;
                                        }
                                    });
                                })
                                .on_create(move |change| {
                                    drawn.update(|events| {
                                        let id = events.len() as u64 + 1;
                                        events.push(SchedulerEvent::new(
                                            id,
                                            change.resource,
                                            change.start,
                                            change.end,
                                            "New event",
                                        ));
                                    });
                                }),))
                            .align(Align::Stretch)
                            .height(380.0)
                        },
                    ),
                    demo(
                        "Recurrence editor",
                        "How often, on which days, until when; it says the result in words.",
                        RecurrenceEditor::new(self.plan_rule.binding(), first_of_plan()),
                    ),
                    demo(
                        "Date picker and combobox",
                        "A calendar in a popover, and a select you can type into.",
                        VStack::new((
                            DatePicker::new(self.date.binding(), today).today(today).label("Date"),
                            Combobox::new(
                                self.country.binding(),
                                [
                                    "Argentina",
                                    "Brazil",
                                    "Canada",
                                    "Chile",
                                    "Germany",
                                    "Japan",
                                    "Mexico",
                                    "Portugal",
                                    "Spain",
                                    "United States",
                                ],
                            )
                            .placeholder("Choose a country")
                            .label("Country"),
                            Text::bound(&self.date, |d| {
                                d.map_or("No date selected".into(), |d| format!("Selected: {d}"))
                            })
                            .style(TextStyle::Callout)
                            .foreground(Semantic::InkMuted),
                        ))
                        .spacing(12.0),
                    ),
                    demo(
                        "One-time code",
                        "Digits only; completes automatically.",
                        InputOtp::new(self.otp.binding(), 6).on_complete(move |code| {
                            otp_toasts.show("Code accepted", format!("Verified {code}."), Tone::Success)
                        }),
                    ),
                    demo(
                        "Form with validation",
                        "Errors appear as you type, or for every field after submitting.",
                        VStack::new((
                            FormField::new(
                                "Company",
                                TextField::new(self.company.binding()).placeholder("Northwind Labs"),
                            )
                            .required()
                            .validate(self.company.binding(), validate::min_length(3))
                            .show_errors(show),
                            FormField::new(
                                "Work email",
                                TextField::new(self.email.binding())
                                    .placeholder("you@company.com")
                                    .icon(IconName::Mail),
                            )
                            .required()
                            .hint("We send the invoice here.")
                            .validate(self.email.binding(), validate::email)
                            .show_errors(show),
                            Button::new("Create account").on_click(move || {
                                submit.set(true);
                                let ok = validate::required(&company.get()).is_ok()
                                    && validate::min_length(3)(&company.get()).is_ok()
                                    && validate::required(&email.get()).is_ok()
                                    && validate::email(&email.get()).is_ok();
                                if ok {
                                    form_toasts.show("Account created", "Welcome aboard.", Tone::Success);
                                }
                            }),
                        ))
                        .spacing(14.0)
                        .align(Align::Stretch),
                    ),
                ),
            ),
        ))
        .spacing(24.0)
        .align(Align::Stretch)
    }

    fn layout_nav(&self, width: f32) -> impl View + use<> {
        let slides: Vec<AnyView> = (0..4)
            .map(|i| -> AnyView { Box::new(Image::new(self.images[i].clone()).stretch().height(240.0)) })
            .collect();
        VStack::new((
            page_header(
                "Layout & navigation",
                "Resizable panels, carousels, menus, toggles, pagination and loaders.",
            ),
            demo(
                "Footer",
                "Left, center and right areas under a hairline. Below 640 pixels of window width they stack, as the second one always does.",
                VStack::new((
                    samples::sample("Footer"),
                    Footer::new()
                        .divider(false)
                        .stacked()
                        .left((Footer::text("© 2026 Raven Finance"),))
                        .center((Footer::link("Privacy"), Footer::link("Terms")))
                        .right((Footer::text("v27.1"),)),
                ))
                .align(Align::Stretch),
            ),
            demo(
                "StatusBar",
                "One fixed line at the foot of a window. The sides keep their width, the center gives way, and a notice can tint only the center.",
                VStack::new((
                    samples::sample("StatusBar"),
                    StatusBar::new()
                        .left((StatusBar::action("Version 27.1"),))
                        .center((
                            StatusBar::text("Version 27.2 is available").truncate().grow(1.0),
                            StatusBar::action("What's new"),
                            StatusBar::action("Update").style(ButtonStyle::Primary),
                        ))
                        .center_tone(Tone::Warning)
                        .right((StatusBar::text("12 items"),)),
                ))
                .align(Align::Stretch),
            ),
            demo(
                "Menubar",
                "Desktop-style menus.",
                Menubar::new()
                    .menu("File")
                    .item(MenuItem::new("New").icon(IconName::FilePlus).shortcut("⌘N"))
                    .item(MenuItem::new("Open…").icon(IconName::FolderOpen).shortcut("⌘O"))
                    .separator()
                    .item(MenuItem::new("Print").icon(IconName::Printer).shortcut("⌘P"))
                    .menu("Edit")
                    .item(MenuItem::new("Undo").icon(IconName::Undo2).shortcut("⌘Z"))
                    .item(MenuItem::new("Redo").icon(IconName::Redo2).shortcut("⇧⌘Z"))
                    .menu("View")
                    .item(MenuItem::new("Zoom in").icon(IconName::ZoomIn))
                    .item(MenuItem::new("Zoom out").icon(IconName::ZoomOut)),
            ),
            demo(
                "Splitter",
                "Drag the handle between the panes.",
                Splitter::horizontal(
                    VStack::new((
                        Text::new("Explorer").style(TextStyle::Headline),
                        Text::new("src/main.rs").foreground(Semantic::InkMuted),
                    ))
                    .padding(16.0),
                    VStack::new((
                        Text::new("Editor").style(TextStyle::Headline),
                        Text::new("fn main() { … }").foreground(Semantic::InkMuted),
                    ))
                    .padding(16.0),
                )
                .ratio(0.32)
                .height(200.0),
            ),
            demo(
                "Carousel",
                "Arrows, dots and autoplay every 5 seconds.",
                Carousel::new(slides).height(240.0).autoplay(5.0),
            ),
            Grid::new(
                columns(width, 2),
                (
                    demo(
                        "Toggle buttons and groups",
                        "Single choice for alignment, several for text styles.",
                        VStack::new((
                            ToggleGroup::single(
                                self.align.binding(),
                                [
                                    (Some(IconName::AlignLeft), ""),
                                    (Some(IconName::AlignCenter), ""),
                                    (Some(IconName::AlignRight), ""),
                                    (Some(IconName::AlignJustify), ""),
                                ],
                            ),
                            ToggleGroup::multiple(
                                self.text_styles.binding(),
                                [
                                    (Some(IconName::Bold), ""),
                                    (Some(IconName::Italic), ""),
                                    (Some(IconName::Underline), ""),
                                    (Some(IconName::Strikethrough), ""),
                                ],
                            ),
                            ToggleButton::new(self.pinned.binding(), "Pin to top").icon(IconName::Pin),
                        ))
                        .spacing(12.0),
                    ),
                    demo(
                        "Pagination, spinner and aspect ratio",
                        "Page through results; loaders respect reduced motion.",
                        VStack::new((
                            Pagination::new(self.page_number.binding(), 24),
                            HStack::new((
                                Spinner::new(),
                                Spinner::new().size(28.0),
                                Text::new("Loading reports…").foreground(Semantic::InkMuted),
                            ))
                            .spacing(14.0)
                            .align(Align::Center),
                            AspectRatio::new(
                                16.0 / 9.0,
                                Image::new(self.images[2].clone()).stretch().corner_radius(10.0),
                            ),
                        ))
                        .spacing(16.0)
                        .align(Align::Stretch),
                    ),
                ),
            ),
        ))
        .spacing(24.0)
        .align(Align::Stretch)
    }

    fn ribbon_page(&self, _width: f32) -> impl View + use<> {
        let zoom = self.ribbon_zoom.clone();
        let (zin, zout) = (zoom.clone(), zoom.clone());
        let toasts = self.toasts.clone();
        let (t1, t2) = (toasts.clone(), toasts.clone());
        let align = self.align.clone();
        let align_toggle = |i: usize, label: &'static str, icon: IconName| {
            let (read, write) = (align.clone(), align.clone());
            RibbonItem::toggle(label, icon, Binding::new(move || read.get() == i, move |_| write.set(i)))
                .icon_only()
        };
        let ribbon = Ribbon::new()
            .app_menu(
                "File",
                vec![
                    MenuItem::new("New report").icon(IconName::FilePlus).shortcut("⌘N"),
                    MenuItem::new("Open…").icon(IconName::FolderOpen).shortcut("⌘O"),
                    MenuItem::new("Save").icon(IconName::Save).shortcut("⌘S"),
                    MenuItem::new("Export to PDF")
                        .icon(IconName::Download)
                        .on_select(move || t1.show("Exported", "invoice-1042.pdf", Tone::Success)),
                    MenuItem::new("Print").icon(IconName::Printer).shortcut("⌘P"),
                ],
            )
            .quick_access(RibbonItem::button("Save", IconName::Save))
            .quick_access(RibbonItem::button("Undo", IconName::Undo2))
            .quick_access(RibbonItem::button("Redo", IconName::Redo2))
            .search(self.ribbon_search.binding())
            .page(
                RibbonPage::new("Home")
                    .group(
                        RibbonGroup::new("Clipboard")
                            .item(
                                RibbonItem::button("Paste", IconName::ClipboardPaste)
                                    .menu_item(MenuItem::new("Paste").icon(IconName::ClipboardPaste))
                                    .menu_item(MenuItem::new("Paste values only").icon(IconName::Type)),
                            )
                            .item(RibbonItem::button("Cut", IconName::Scissors).small())
                            .item(RibbonItem::button("Copy", IconName::Copy).small())
                            .item(RibbonItem::button("Format painter", IconName::Paintbrush).small()),
                    )
                    .group(
                        RibbonGroup::new("Font")
                            .item(
                                RibbonItem::toggle("Bold", IconName::Bold, self.ribbon_bold.binding())
                                    .icon_only(),
                            )
                            .item(
                                RibbonItem::toggle("Italic", IconName::Italic, self.ribbon_italic.binding())
                                    .icon_only(),
                            )
                            .item(RibbonItem::button("Underline", IconName::Underline).icon_only())
                            .item(RibbonItem::button("Strikethrough", IconName::Strikethrough).icon_only())
                            .item(RibbonItem::button("Highlight", IconName::Highlighter).icon_only())
                            .item(RibbonItem::button("Font color", IconName::Baseline).icon_only())
                            .dialog_launcher(move || {
                                t2.show("Font", "The font dialog would open here.", Tone::Neutral)
                            }),
                    )
                    .group(
                        RibbonGroup::new("Alignment")
                            .item(align_toggle(0, "Align left", IconName::AlignLeft))
                            .item(align_toggle(1, "Center", IconName::AlignCenter))
                            .item(align_toggle(2, "Align right", IconName::AlignRight))
                            .item(align_toggle(3, "Justify", IconName::AlignJustify))
                            .item(RibbonItem::button("Bulleted list", IconName::List).icon_only())
                            .item(RibbonItem::button("Numbered list", IconName::ListOrdered).icon_only()),
                    )
                    .group(
                        RibbonGroup::new("Editing")
                            .item(RibbonItem::button("Find", IconName::Search).small())
                            .item(RibbonItem::button("Replace", IconName::ArrowUpDown).small())
                            .item(RibbonItem::button("Select all", IconName::TextCursor).small()),
                    ),
            )
            .page(
                RibbonPage::new("Insert")
                    .group(
                        RibbonGroup::new("Controls")
                            .item(RibbonItem::button("Label", IconName::Type))
                            .item(RibbonItem::button("Table", IconName::Table2))
                            .item(RibbonItem::button("Picture", IconName::ImagePlus))
                            .item(RibbonItem::button("Chart", IconName::ChartColumn)),
                    )
                    .group(
                        RibbonGroup::new("Elements")
                            .item(RibbonItem::button("Shapes", IconName::Shapes).small())
                            .item(RibbonItem::button("Page number", IconName::Pilcrow).small())
                            .item(RibbonItem::button("Link", IconName::Link2).small()),
                    ),
            )
            .page(
                RibbonPage::new("Data")
                    .group(
                        RibbonGroup::new("Data source")
                            .item(RibbonItem::button("Add data source", IconName::Database))
                            .item(RibbonItem::button("Query builder", IconName::Filter)),
                    )
                    .group(
                        RibbonGroup::new("Calculations")
                            .item(RibbonItem::button("Calculated field", IconName::Calculator).small())
                            .item(RibbonItem::button("Summary", IconName::Sigma).small())
                            .item(RibbonItem::button("Parameters", IconName::SlidersHorizontal).small()),
                    ),
            )
            .page(
                RibbonPage::new("View")
                    .group(
                        RibbonGroup::new("Show")
                            .item(
                                RibbonItem::toggle("Ruler", IconName::Ruler, self.ribbon_ruler.binding())
                                    .large(),
                            )
                            .item(
                                RibbonItem::toggle(
                                    "Gridlines",
                                    IconName::Grid2x2,
                                    self.ribbon_grid.binding(),
                                )
                                .large(),
                            )
                            .item(
                                RibbonItem::toggle(
                                    "Table selected",
                                    IconName::Table,
                                    self.ribbon_table.binding(),
                                )
                                .large(),
                            ),
                    )
                    .group(
                        RibbonGroup::new("Zoom")
                            .item(
                                RibbonItem::button("Zoom in", IconName::ZoomIn)
                                    .on_click(move || zin.update(|z| *z = (*z + 10).min(400))),
                            )
                            .item(
                                RibbonItem::button("Zoom out", IconName::ZoomOut)
                                    .on_click(move || zout.update(|z| *z = z.saturating_sub(10).max(10))),
                            ),
                    ),
            )
            .page(
                RibbonPage::new("Layout")
                    .contextual("Table tools", Tone::Warning, self.ribbon_table.get())
                    .group(
                        RibbonGroup::new("Rows & columns")
                            .item(RibbonItem::button("Insert row", IconName::Rows3))
                            .item(RibbonItem::button("Insert column", IconName::Columns3))
                            .item(RibbonItem::button("Merge cells", IconName::Merge).small())
                            .item(RibbonItem::button("Split cells", IconName::Split).small()),
                    ),
            );
        VStack::new((
            page_header(
                "Ribbon",
                "Office-style command surface: File menu, quick access toolbar, pages, groups with large and small commands, toggles, drop-downs, dialog launchers, a contextual \"Table tools\" tab and search. Double-click a tab to collapse it.",
            ),
            Card::new(
                VStack::new((
                    ribbon,
                    HStack::new((
                        VStack::new((
                            Text::new("FIELD LIST").style(TextStyle::Caption).foreground(Semantic::InkMuted),
                            List::new((
                                ListItem::new("InvoiceID").icon(IconName::Pilcrow),
                                ListItem::new("Customer").icon(IconName::User),
                                ListItem::new("Amount").icon(IconName::DollarSign),
                                ListItem::new("Issued").icon(IconName::CalendarDays),
                            )),
                        ))
                        .spacing(8.0)
                        .width(210.0)
                        .padding(14.0),
                        VStack::new((
                            Text::bound(&self.ribbon_zoom, |z| format!("Invoice report · {z}%"))
                                .style(TextStyle::Caption)
                                .foreground(Semantic::InkMuted),
                            VStack::new((
                                Text::new("REPORT HEADER").style(TextStyle::Caption).foreground(Semantic::InkMuted),
                                Text::new("Invoice #1042").style(TextStyle::Title),
                                Divider::horizontal(),
                                Text::new("DETAIL").style(TextStyle::Caption).foreground(Semantic::InkMuted),
                                HStack::new((Text::new("[Customer]").grow(1.0), Text::new("[Amount]"))).spacing(12.0),
                                Divider::horizontal(),
                                Text::new("PAGE FOOTER").style(TextStyle::Caption).foreground(Semantic::InkMuted),
                                Text::new("Page [PageNumber] of [PageCount]").foreground(Semantic::InkMuted),
                            ))
                            .spacing(10.0)
                            .padding(24.0)
                            .background(Semantic::Surface)
                            .border(1.0, Semantic::Border)
                            .corner_radius(4.0),
                        ))
                        .spacing(10.0)
                        .padding(16.0)
                        .grow(1.0),
                    ))
                    .align(Align::Stretch),
                ))
                .spacing(0.0)
                .align(Align::Stretch),
            )
            .padding(0.0)
            .corner_radius(14.0),
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

    fn data_grid(&self, width: f32) -> impl View + use<> {
        let info = self.export_info.clone();
        let _ = width;
        VStack::new((
            page_header(
                "Data grid",
                "100,000 rows, virtualized. Sort (Shift-click for several columns), filter with >, <, = or a..b, search, group, resize columns, select with Ctrl/Shift, edit with a double-click, export to CSV.",
            ),
            DataGrid::new(
                self.grid.clone(),
                [
                    GridColumn::number("ID").width(90.0).groupable(false),
                    GridColumn::text("Customer").width(200.0).editable(),
                    GridColumn::text("Country").width(140.0),
                    GridColumn::text("Plan").width(130.0).template(|v| -> AnyView {
                        let text = if let Value::Text(t) = v { t.clone() } else { String::new() };
                        let tone = match text.as_str() {
                            "Enterprise" => Tone::Accent,
                            "Business" => Tone::Success,
                            "Team" => Tone::Warning,
                            _ => Tone::Neutral,
                        };
                        Box::new(Badge::new(text).tone(tone))
                    }),
                    GridColumn::number("Seats").width(100.0).summary(Summary::Sum).editable(),
                    GridColumn::currency("MRR", "R$ ", 2).width(150.0).summary(Summary::Sum),
                    GridColumn::number("Health").width(110.0).summary(Summary::Average).number_format(NumberFormat {
                        decimals: 0,
                        suffix: "%".into(),
                        ..NumberFormat::default()
                    }),
                    GridColumn::boolean("Active").width(100.0),
                    GridColumn::text("Account manager").width(170.0).hidden(),
                ],
            )
            .selection_mode(SelectionMode::Multiple)
            .selection(self.grid_selection.binding())
            .sort_by(5, SortDirection::Descending)
            .height(620.0)
            .on_export(move |csv| {
                let lines = csv.lines().count().saturating_sub(1);
                info.set(format!("Exported {lines} rows as CSV ({} KB).", csv.len() / 1024));
            }),
            Text::bound(&self.export_info, Clone::clone).style(TextStyle::Callout).foreground(Semantic::InkMuted),
        ))
        .spacing(18.0)
        .align(Align::Stretch)
    }

    fn images(&self, width: f32) -> impl View + use<> {
        let [aurora, dunes, ocean, bloom] = [0, 1, 2, 3].map(|i| self.images[i].clone());
        let card = |image: ImageData, title: &'static str, caption: &'static str| -> AnyView {
            Box::new(
                Card::new(
                    VStack::new((
                        Image::new(image).stretch().height(170.0).corner_radius(12.0),
                        Text::new(title).style(TextStyle::Headline),
                        Text::new(caption).style(TextStyle::Callout).foreground(Semantic::InkMuted),
                    ))
                    .spacing(10.0)
                    .align(Align::Stretch),
                )
                .padding(12.0)
                .corner_radius(18.0),
            )
        };
        let mode = |mode: ContentMode, label: &'static str, image: ImageData| -> AnyView {
            Box::new(
                VStack::new((
                    Image::new(image)
                        .content_mode(mode)
                        .size(180.0, 130.0)
                        .corner_radius(10.0)
                        .background(Semantic::Control)
                        .corner_radius(10.0),
                    Text::new(label).style(TextStyle::Callout).foreground(Semantic::InkMuted),
                ))
                .spacing(8.0),
            )
        };
        VStack::new((
            page_header(
                "Images",
                "Textures with mipmaps, rounded corners and cover, fit or fill. PNG and JPEG decode with Image::from_bytes; these are generated in Rust.",
            ),
            Grid::new(
                columns(width, 4),
                vec![
                    card(aurora.clone(), "Aurora", "Sine bands over a night gradient"),
                    card(dunes.clone(), "Dunes", "Layered curves at sunset"),
                    card(ocean.clone(), "Ocean", "Interfering waves"),
                    card(bloom.clone(), "Bloom", "A radial color field"),
                ],
            ),
            demo(
                "Content modes",
                "The same wide image in a 180 × 130 frame.",
                HStack::new((
                    mode(ContentMode::Cover, "Cover: fills and crops", dunes.clone()),
                    mode(ContentMode::Fit, "Fit: shows everything", dunes.clone()),
                    mode(ContentMode::Fill, "Fill: stretches", dunes),
                ))
                .spacing(20.0)
                .wrap(),
            ),
            demo(
                "Shapes and opacity",
                "Any corner radius, and opacity for disabled or decorative images.",
                HStack::new((
                    Image::new(bloom.clone()).size(64.0, 64.0).corner_radius(32.0),
                    Image::new(ocean.clone()).size(64.0, 64.0).corner_radius(16.0),
                    Image::new(aurora.clone()).size(64.0, 64.0),
                    Image::new(bloom).size(64.0, 64.0).corner_radius(32.0).opacity(0.4),
                ))
                .spacing(16.0)
                .align(Align::Center)
                .wrap(),
            ),
        ))
        .spacing(24.0)
        .align(Align::Stretch)
    }

    fn feedback(&self, width: f32) -> impl View + use<> {
        VStack::new((
            page_header("Feedback", "Alerts, progress and loading placeholders."),
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

    fn motion(&self, width: f32) -> impl View + use<> {
        let (sprint, progress) = (self.motion_side.clone(), self.motion_progress.clone());
        let flip = sprint.clone();
        let advance = progress.clone();
        let toasts = self.toasts.clone();
        let show = move || toasts.show("Saved", "Toasts slide in and make room.", Tone::Success);
        VStack::new((
            page_header(
                "Motion",
                "Springs that keep their speed when interrupted, colors mixed in OKLab, and exits that outlive the view.",
            ),
            Grid::new(columns(width, 2), (
                demo(
                    "Interruptible springs",
                    "Click again before it lands: it turns around with no jump in speed.",
                    VStack::new((
                        MotionTrack(sprint),
                        Button::new("Send to the other side")
                            .style(ButtonStyle::Secondary)
                            .on_click(move || flip.update(|s| *s = !*s)),
                    ))
                    .spacing(14.0)
                    .align(Align::Stretch),
                ),
                demo(
                    "Shared selection",
                    "One marker that travels between items, under the labels.",
                    VStack::new((
                        SegmentedControl::new(self.motion_segment.binding(), ["Day", "Week", "Month", "Year"]),
                        Tabs::new(self.motion_tab.binding())
                            .tab("Summary", Text::new("Summary").foreground(Semantic::InkMuted))
                            .tab("Activity log", Text::new("Activity").foreground(Semantic::InkMuted))
                            .tab("Billing", Text::new("Billing").foreground(Semantic::InkMuted)),
                    ))
                    .spacing(18.0)
                    .align(Align::Stretch),
                ),
                demo(
                    "Press and toggle",
                    "Colors ease in OKLab, buttons give under the pointer, and the thumb stretches while held.",
                    VStack::new((
                        HStack::new((
                            Button::new("Primary"),
                            Button::new("Secondary").style(ButtonStyle::Secondary),
                            Button::new("Ghost").style(ButtonStyle::Ghost),
                        ))
                        .spacing(10.0),
                        Toggle::new(self.motion_toggle.binding()).label("Wi-Fi"),
                        Checkbox::new("Send a copy to me", self.motion_check.binding()),
                    ))
                    .spacing(14.0),
                ),
                demo(
                    "Height to fit",
                    "The hard one on the web: content of unknown height opens smoothly.",
                    Accordion::new(vec![
                        Disclosure::new(
                            "What is animated?",
                            Text::new("Fill, border, shadow, radius, text color, position, size and opacity.")
                                .foreground(Semantic::InkMuted),
                        ),
                        Disclosure::new(
                            "What does it cost?",
                            Text::new("Paint only: views are not rebuilt and layout does not run again.")
                                .foreground(Semantic::InkMuted),
                        ),
                    ]),
                ),
                demo(
                    "Enter and exit",
                    "A closed view stays on screen as a picture until its exit ends. It takes no input.",
                    HStack::new((
                        Button::new("Open dialog").on_click({
                            let open = self.motion_dialog.clone();
                            move || open.set(true)
                        }),
                        Button::new("Show toast").style(ButtonStyle::Secondary).on_click(show),
                    ))
                    .spacing(10.0),
                ),
                demo(
                    "Values and loaders",
                    "Progress eases to each new value. Loaders paint without rebuilding the UI.",
                    VStack::new((
                        ProgressBar::new(*progress.peek()),
                        HStack::new((
                            Button::new("Advance")
                                .style(ButtonStyle::Secondary)
                                .size(ButtonSize::Small)
                                .on_click(move || advance.update(|p| *p = if *p >= 1.0 { 0.1 } else { *p + 0.3 })),
                            Spinner::new(),
                            Skeleton::line(14.0).grow(1.0),
                        ))
                        .spacing(14.0)
                        .align(Align::Center),
                        ReducedMotionSwitch,
                    ))
                    .spacing(14.0)
                    .align(Align::Stretch),
                ),
            ))
            .spacing(16.0),
            Dialog::new(self.motion_dialog.binding(), "Enter and exit")
                .description("The card springs in. Close it and it fades out as a picture of itself."),
        ))
        .spacing(20.0)
        .align(Align::Stretch)
    }

    fn charts(&self, width: f32) -> impl View + use<> {
        VStack::new((
            page_header("Charts", "The same layout as reports: shapes in a picture at the screen's density, labels as real text."),
            demo(
                "Area chart",
                "A smoothed series over a filled area.",
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
            demo(
                "Line chart",
                "Several series, with a legend to tell them apart.",
                Chart::new(ChartKind::Line)
                    .categories(["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug"])
                    .series("2025", [12.0, 18.0, 15.0, 21.0, 19.0, 24.0, 23.0, 27.0])
                    .series("2026", [14.0, 21.0, 26.0, 24.0, 31.0, 35.0, 33.0, 40.0])
                    .height(260.0),
            ),
            Grid::new(
                columns(width, 2),
                (
                    demo(
                        "Pie chart",
                        "Slices labeled by their share.",
                        Chart::new(ChartKind::Pie)
                            .categories(["Direct", "Search", "Referral", "Social"])
                            .series("Visits", [46.0, 28.0, 16.0, 10.0])
                            .height(260.0),
                    ),
                    demo(
                        "Doughnut chart",
                        "The same slices around a hole.",
                        Chart::new(ChartKind::Doughnut)
                            .categories(["Desktop", "Mobile", "Tablet"])
                            .series("Sessions", [58.0, 34.0, 8.0])
                            .height(260.0),
                    ),
                    demo(
                        "Scatter chart",
                        "Dots at numeric positions.",
                        Chart::new(ChartKind::Scatter)
                            .categories(["1", "2", "3", "4", "5", "6", "7", "8"])
                            .positions([1.0, 2.0, 3.5, 4.0, 5.5, 6.0, 7.5, 9.0])
                            .series("Latency", [12.0, 19.0, 17.0, 26.0, 24.0, 33.0, 30.0, 41.0])
                            .height(260.0),
                    ),
                    demo(
                        "Sparkline",
                        "A small line to sit next to a number.",
                        HStack::new((
                            Stat::new("Revenue", "R$ 128k").delta("+12.4%", true),
                            Chart::new(ChartKind::Sparkline)
                                .series("Revenue", [4.0, 6.0, 5.0, 8.0, 7.0, 10.0, 9.0, 13.0])
                                .size(140.0, 44.0),
                        ))
                        .spacing(24.0)
                        .align(Align::Center),
                    ),
                ),
            ),
            demo(
                "QR Code and barcodes",
                "Every module lands on whole pixels, and they stay dark on light in every theme.",
                HStack::new((
                    Barcode::qr("https://ravenrs.dev").alt("Link to the RavenRS site"),
                    VStack::new((
                        Barcode::new(Symbology::Code128, "RAVEN-2711"),
                        Barcode::new(Symbology::Ean13, "789123456789"),
                    ))
                    .spacing(16.0),
                ))
                .spacing(24.0),
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
                &format!("{} Lucide icons (ISC license) built in, drawn as font glyphs so they take any size and color.", IconName::ALL.len()),
            ),
            Grid::new(columns(width, 6), tiles).spacing(10.0),
        ))
        .spacing(24.0)
        .align(Align::Stretch)
    }
}

/// Deterministic sample data for the data grid demo.
fn sample_rows(n: usize) -> Vec<Vec<Value>> {
    const FIRST: &[&str] = &[
        "Northwind",
        "Aurora",
        "Kestrel",
        "Lumen",
        "Vertex",
        "Harbor",
        "Nimbus",
        "Cobalt",
        "Juniper",
        "Atlas",
        "Solace",
        "Quartz",
        "Meridian",
        "Tidal",
        "Orchid",
        "Summit",
    ];
    const SECOND: &[&str] =
        &["Labs", "Studio", "Health", "Data", "Systems", "Foods", "Logistics", "Bank", "Energy", "Media"];
    const COUNTRIES: &[&str] = &[
        "Brazil",
        "Portugal",
        "Argentina",
        "Chile",
        "Mexico",
        "United States",
        "Germany",
        "Japan",
        "Canada",
        "Spain",
    ];
    const PLANS: &[&str] = &["Starter", "Team", "Business", "Enterprise"];
    const MANAGERS: &[&str] = &["Ana Lima", "Bruno Reis", "Carla Dias", "Diego Alves", "Elisa Rocha"];
    let mut seed: u64 = 0x5eed_cafe;
    let mut next = move || {
        seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
        (seed >> 33) as usize
    };
    (0..n)
        .map(|i| {
            let plan = next() % PLANS.len();
            let seats = match plan {
                0 => 1 + next() % 5,
                1 => 5 + next() % 30,
                2 => 20 + next() % 150,
                _ => 100 + next() % 900,
            };
            let price = [0.0, 49.0, 89.0, 139.0][plan];
            vec![
                Value::from(i + 1),
                Value::from(format!("{} {}", FIRST[next() % FIRST.len()], SECOND[next() % SECOND.len()])),
                Value::from(COUNTRIES[next() % COUNTRIES.len()]),
                Value::from(PLANS[plan]),
                Value::from(seats),
                Value::from(seats as f64 * price),
                Value::from((40 + next() % 61) as f64),
                Value::from(next() % 10 != 0),
                Value::from(MANAGERS[next() % MANAGERS.len()]),
            ]
        })
        .collect()
}

/// Procedural images, so the gallery ships no third-party pictures.
mod art {
    use ravenrs::prelude::*;

    fn mix(a: Color, b: Color, t: f32) -> Color {
        a.lerp(b, t)
    }

    pub fn all() -> Vec<ImageData> {
        let (w, h) = (480, 300);
        let aurora = ImageData::from_fn(w, h, |x, y| {
            let (u, v) = (x as f32 / w as f32, y as f32 / h as f32);
            let sky = mix(Color::hex(0x0b1030), Color::hex(0x2a1b4f), v);
            let band = ((u * 9.0 + (v * 4.0).sin() * 1.5).sin() * 0.5 + 0.5)
                * (1.0 - (v - 0.35).abs() * 2.6).max(0.0);
            mix(sky, Color::hex(0x3cf2b0), band * 0.85)
        });
        let dunes = ImageData::from_fn(w, h, |x, y| {
            let (u, v) = (x as f32 / w as f32, y as f32 / h as f32);
            let mut c = mix(Color::hex(0xffb36b), Color::hex(0xff6f61), v);
            for (k, color) in [(0.55, 0xe2734a), (0.68, 0xc4553a), (0.82, 0x8f3b2e)] {
                let line = k + (u * 6.0 + k * 9.0).sin() * 0.05;
                if v > line {
                    c = mix(c, Color::hex(color), 0.9);
                }
            }
            c
        });
        let ocean = ImageData::from_fn(w, h, |x, y| {
            let (u, v) = (x as f32 / w as f32, y as f32 / h as f32);
            let wave = ((u * 14.0 + v * 6.0).sin() + (u * 5.0 - v * 11.0).sin()) * 0.25 + 0.5;
            mix(mix(Color::hex(0x0a3d62), Color::hex(0x38ada9), v), Color::hex(0xd6f5ff), wave * 0.35)
        });
        let bloom = ImageData::from_fn(256, 256, |x, y| {
            let (u, v) = (x as f32 / 255.0 - 0.5, y as f32 / 255.0 - 0.5);
            let r = (u * u + v * v).sqrt() * 2.0;
            let a = v.atan2(u);
            let petals = ((a * 5.0).sin() * 0.5 + 0.5) * (1.0 - r).max(0.0);
            mix(mix(Color::hex(0x4c66ee), Color::hex(0x161a2e), r.min(1.0)), Color::hex(0xff8fd8), petals)
        });
        [aurora, dunes, ocean, bloom].into_iter().flatten().collect()
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
            let shell: AnyView = if narrow {
                Box::new(main)
            } else {
                Box::new(HStack::new((self.sidebar(), main)).spacing(0.0).align(Align::Stretch).grow(1.0))
            };
            Box::new(
                VStack::new((shell, self.palette_view(), Toaster::new(self.toasts.clone())))
                    .spacing(0.0)
                    .align(Align::Stretch)
                    .grow(1.0),
            )
        })
    }
}

struct PageBody(AnyView);

impl View for PageBody {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let content = cx.child(self.0.as_ref());
        cx.container(
            ravenrs::layout::LayoutStyle {
                max_width: Some(1080.0),
                align: Align::Stretch,
                ..ravenrs::layout::LayoutStyle::stack(ravenrs::layout::Axis::Vertical)
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
        static APP: RefCell<Option<ravenrs::web::WebApp>> = const { RefCell::new(None) };
    }

    #[wasm_bindgen(start)]
    pub fn start() {
        console_error_panic_hook::set_once();
        wasm_bindgen_futures::spawn_local(async {
            let options = ravenrs::web::WebOptions::new("raven-canvas");
            // `?component=Button` shows that example alone, for the component pages.
            let sample = requested_component().and_then(|name| super::samples::SampleApp::new(&name));
            let started = match sample {
                Some(sample) => ravenrs::web::start(sample, options).await,
                None => ravenrs::web::start(super::Gallery::new(), options).await,
            };
            match started {
                Ok(app) => APP.with(|a| *a.borrow_mut() = Some(app)),
                Err(e) => web_sys::console::error_1(&e.to_string().into()),
            }
        });
    }

    fn requested_component() -> Option<String> {
        let search = web_sys::window()?.location().search().ok()?;
        let params = web_sys::UrlSearchParams::new_with_str(&search).ok()?;
        params.get("component")
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

    /// The lowest text to the right of `left`, which leaves out a fixed sidebar.
    fn first_glyph_y(ui: &Ui, left: f32) -> f32 {
        ui.scene()
            .primitives()
            .iter()
            .filter_map(|p| match p {
                ravenrs::core::Primitive::Glyphs(g) => g.glyphs.first().filter(|g| g.x >= left).map(|g| g.y),
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
            let left = if size.width > 800.0 { 248.0 } else { 0.0 };
            let before = first_glyph_y(&ui, left);
            let at = Point::new(size.width - 60.0, size.height - 100.0);
            assert!(
                ui.handle_event(&ravenrs::core::Event::Scroll {
                    position: at,
                    delta: Point::new(0.0, -200.0)
                }),
                "the page scrolls at {size:?}"
            );
            assert!(
                (first_glyph_y(&ui, left) - (before - 200.0)).abs() < 0.5,
                "content moved up by 200px at {size:?}"
            );
        }
    }
}
