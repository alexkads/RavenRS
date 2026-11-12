//! The navigation page.

use ravenrs::prelude::*;

use crate::{Gallery, demo, page_header, sample_demos};

impl Gallery {
    pub(crate) fn sidebar_demo(&self) -> impl View + use<> {
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
            .spacing(12.0)
            .align(Align::Center)
            .padding((0.0, 12.0)),
        )
        .footer(
            HStack::new((Avatar::new("Ana Lima"), Text::new("Ana Lima").style(TextStyle::Callout)))
                .spacing(12.0)
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

    pub(crate) fn navigation(&self, _width: f32) -> impl View + use<> {
        VStack::new((
            page_header("Navigation", "Tabs, breadcrumbs, sidebars and disclosures."),
            sample_demos(&[
                ("DocumentTabs", "Tabs for open documents that close, move, rename, carry a color and say what was not saved."),
                ("FileBrowser", "Folders and files from the app's data, with a breadcrumb, sorting and selection."),
                ("YearView", "The twelve months of a year, with marks on the days and a click on any of them."),
            ]),
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
}
