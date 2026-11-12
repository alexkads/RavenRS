//! The data display and data grid pages.

use ravenrs::prelude::*;

use crate::{Gallery, columns, demo, page_header};

impl Gallery {
    pub(crate) fn data_display(&self, width: f32) -> impl View + use<> {
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

    pub(crate) fn data_grid(&self, width: f32) -> impl View + use<> {
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
            .vertical_lines(false)
            .scenarios(self.grid_scenarios.binding())
            .height(620.0)
            .on_export(move |csv| {
                let lines = csv.lines().count().saturating_sub(1);
                info.set(format!("Exported {lines} rows as CSV ({} KB).", csv.len() / 1024));
            }),
            Text::bound(&self.export_info, Clone::clone).style(TextStyle::Callout).foreground(Semantic::InkMuted),
            demo(
                "Row details and files",
                "An arrow, or the Right key, opens what is below a row. The Export menu makes a real workbook or PDF, here only measured.",
                DataGrid::new(
                    GridData::new(
                        (1..=200)
                            .map(|i| vec![format!("Account {i}").into(), (f64::from(i) * 37.5).into()])
                            .collect(),
                    ),
                    [
                        GridColumn::text("Account").fill(),
                        GridColumn::currency("Balance", "R$ ", 2).width(180.0).summary(Summary::Sum),
                    ],
                )
                .row_detail(|row, _| -> AnyView {
                    Box::new(
                        DataGrid::new(
                            GridData::new(
                                (1..=20)
                                    .map(|i| vec![format!("Entry {i}").into(), (f64::from(i) * 2.5).into()])
                                    .collect(),
                            ),
                            [
                                GridColumn::text(row[0].key_text()).fill(),
                                GridColumn::currency("Amount", "R$ ", 2).width(160.0),
                            ],
                        )
                        .toolbar(false)
                        .filter_row(false)
                        .footer(false)
                        .height(180.0),
                    )
                })
                .detail_height(204.0)
                .export_title("Accounts")
                .on_export_file({
                    let info = self.export_info.clone();
                    move |format, table| {
                        info.set(match ravenrs_grid_export::export(&table, format, None) {
                            Ok(file) => format!("{}: {} KB.", file.name, file.bytes.len() / 1024),
                            Err(e) => e,
                        });
                    }
                })
                .height(360.0),
            ),
            demo(
                "Rows on a server: infinite scroll",
                "Five million rows that stay on the server. The grid asks for what is on the screen, once scrolling rests, at most two chunks at a time, and holds eight.",
                DataGrid::remote(
                    self.grid_remote.clone(),
                    [GridColumn::number("Id").width(120.0), GridColumn::text("Customer").fill()],
                )
                .height(320.0),
            ),
            demo(
                "Rows on a server: pages",
                "The same rows, a page at a time. The first answer says how many there are; no other asks.",
                DataGrid::remote(
                    self.grid_remote_pages.clone(),
                    [GridColumn::number("Id").width(120.0), GridColumn::text("Customer").fill()],
                )
                .page_size(25)
                .height(360.0),
            ),
        ))
        .spacing(20.0)
        .align(Align::Stretch)
    }
}
