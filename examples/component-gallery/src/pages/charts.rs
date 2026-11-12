//! The charts page.

use ravenrs::prelude::*;

use crate::{Gallery, charts, columns, demo, page_header, sample_demos};

impl Gallery {
    pub(crate) fn charts(&self, width: f32) -> impl View + use<> {
        VStack::new((
            page_header("Charts", "Every pattern of the shadcn/ui catalog. Shapes in a picture at the screen's density, labels as real text."),
            charts::ChartCatalog { columns: columns(width, 2) },
            sample_demos(&[
                ("ChartReference", "A dashed line at a value that matters, over an axis of fixed ends."),
                ("SankeyChart", "How much flows from what to what. Walk it with the arrow keys; flows in a circle say so."),
                ("FlowCanvas", "Nodes joined by connections. Drag a node or the surface, drag from a port to connect, Shift-drag to select; an input that takes something else says so."),
            ]),
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
                        "Pie with guides",
                        "Labels in a column on each side, none over another, and a real gap between slices.",
                        Chart::new(ChartKind::Pie)
                            .categories(["Rent", "Food", "Fun", "Car", "Gym", "Pets", "Taxes"])
                            .series("Spent", [42.0, 20.0, 1.5, 2.0, 1.0, 12.0, 18.0])
                            .category_labels(ChartLabelAt::Outside)
                            .label_guides()
                            .hide_legend()
                            .height(260.0),
                    ),
                    demo(
                        "Bars from the top",
                        "Bars can grow from any side, fill from tip to base and keep a least length.",
                        Chart::new(ChartKind::Bar)
                            .categories(["Jan", "Feb", "Mar", "Apr", "May", "Jun"])
                            .series("Depth", [12.0, 0.2, 15.0, 9.0, 22.0, 17.0])
                            .bars_from(ChartBarFrom::Top)
                            .gradient()
                            .min_bar_length(4.0)
                            .show_value_axis()
                            .height(260.0),
                    ),
                    demo(
                        "Candlestick",
                        "What a price did each day: rising candles are outlines, falling ones are solid.",
                        Chart::new(ChartKind::Candlestick)
                            .categories(["Mon", "Tue", "Wed", "Thu", "Fri", "Mon", "Tue", "Wed"])
                            .candles([
                                [100.0, 112.0, 98.0, 110.0],
                                [110.0, 111.0, 101.0, 102.0],
                                [102.0, 108.0, 99.0, 106.0],
                                [106.0, 120.0, 105.0, 118.0],
                                [118.0, 119.0, 109.0, 111.0],
                                [111.0, 113.0, 100.0, 103.0],
                                [103.0, 104.0, 92.0, 95.0],
                                [95.0, 105.0, 94.0, 104.0],
                            ])
                            .height(260.0),
                    ),
                    demo(
                        "Scatter chart",
                        "Dots at numeric positions.",
                        Chart::new(ChartKind::Scatter)
                            .categories(["1", "2", "3", "4", "5", "6", "7", "8"])
                            .positions([1.0, 2.0, 3.5, 4.0, 5.5, 6.0, 7.5, 9.0])
                            .series("Latency", [12.0, 19.0, 17.0, 26.0, 24.0, 33.0, 30.0, 41.0])
                            .tooltip(ChartTooltip::new())
                            .height(260.0),
                    ),
                    demo(
                        "Sparkline",
                        "A small line to sit next to a number.",
                        HStack::new((
                            Stat::new("Revenue", "R$ 128k").delta("+12.4%", true),
                            Chart::new(ChartKind::Sparkline)
                                .series("Revenue", [4.0, 6.0, 5.0, 8.0, 7.0, 10.0, 9.0, 13.0])
                                .tooltip(ChartTooltip::new().hide_label())
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
}
