//! The overview and the buttons.

use ravenrs::prelude::*;

use crate::{Gallery, columns, demo, page_header, sample_demos, stat_card};

impl Gallery {
    pub(crate) fn overview(&self, width: f32) -> impl View + use<> {
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

    pub(crate) fn buttons(&self, _width: f32) -> impl View + use<> {
        let clicks = self.clicks.clone();
        let inc = move || clicks.update(|n| *n += 1);
        let (a, b, c, d) = (inc.clone(), inc.clone(), inc.clone(), inc);
        VStack::new((
            page_header(
                "Buttons",
                "Actions in four styles and three sizes, with icons, focus rings and keyboard support.",
            ),
            sample_demos(&[("ButtonGroup", "Buttons glued side by side, as one control.")]),
            demo(
                "Styles",
                "One primary action per view; the rest are secondary or ghost.",
                VStack::new((
                    HStack::new((
                        Button::new("Get started").on_click(a),
                        Button::new("Save draft").style(ButtonStyle::Secondary).on_click(b),
                        Button::new("Cancel").style(ButtonStyle::Ghost).on_click(c),
                        Button::new("Delete").style(ButtonStyle::Danger).icon(IconName::Trash2).on_click(d),
                    ))
                    .spacing(12.0)
                    .wrap(),
                    Text::bound(&self.clicks, |n| format!("Clicked {n} times. Try Tab and Enter too."))
                        .style(TextStyle::Callout)
                        .foreground(Semantic::InkMuted),
                ))
                .spacing(16.0),
            ),
            demo(
                "Sizes and icons",
                "Small for dense toolbars, large for primary calls to action.",
                HStack::new((
                    Button::new("Add").style(ButtonStyle::Secondary).size(ButtonSize::Small).icon(IconName::Plus),
                    Button::new("Download").style(ButtonStyle::Secondary).icon(IconName::Download),
                    // ravenrs-lint: allow(many_primary_buttons) this card shows the large size, made for the primary call to action; the page's other primary sits in its own card
                    Button::new("Continue").size(ButtonSize::Large).trailing_icon(IconName::ArrowRight),
                    Button::new("Publish").style(ButtonStyle::Secondary).disabled(true),
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
                .spacing(8.0)
                .wrap(),
            ),
            demo(
                "States",
                "Loading holds the width and ignores clicks; selected stays lit; a shortcut takes the label's color.",
                HStack::new((
                    Button::new("Upload").style(ButtonStyle::Secondary).loading(true),
                    Button::new("Show grid").style(ButtonStyle::Secondary).selected(true),
                    IconButton::new(IconName::Bold).label("Bold").selected(true),
                    Button::new("Open").style(ButtonStyle::Ghost).shortcut("F8"),
                ))
                .spacing(12.0)
                .align(Align::Center)
                .wrap(),
            ),
        ))
        .spacing(24.0)
        .align(Align::Stretch)
    }
}
