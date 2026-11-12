//! Records, lists and charts.

use ravenrs::prelude::*;

samples! {
    "TreeView" => tree_view, "TileLayout" => tile_layout, "CardView" => card_view, "DataGrid" => data_grid,
    "Table" => table, "List" => list, "Stat" => stat, "Chart" => chart, "ChartReference" => chart_reference,
    "SankeyChart" => sankey_chart, "FlowCanvas" => flow_canvas, "ChartCard" => chart_card, "BarChart" => bar_chart,
    "AreaChart" => area_chart, "Barcode" => barcode, "Timeline" => timeline, "PropertyRow" => property_row,
    "VirtualList" => virtual_list, "PagedDocumentView" => paged_document_view,
    "FileBrowser" => file_browser, "SortableList" => sortable_list,
}

// sample: TreeView, 260
fn tree_view() -> impl View {
    let selected = State::new(Some(String::from("invoices")));
    let opened = State::new(std::collections::BTreeSet::from([String::from("sales")]));
    TreeView::new(vec![
        TreeNode::new("sales", "Sales")
            .icon(IconName::Folder)
            .child(TreeNode::new("invoices", "Invoices").icon(IconName::FileText))
            .child(TreeNode::new("quotes", "Quotes").icon(IconName::FileText)),
        TreeNode::new("stock", "Stock")
            .icon(IconName::Folder)
            .child(TreeNode::new("items", "Items").icon(IconName::FileText)),
        TreeNode::new("reports", "Reports").icon(IconName::FileText),
    ])
    .selected(selected.binding())
    .expanded(opened.binding())
}
// end

// sample: TileLayout, 400
fn tile_layout() -> impl View {
    let spots = State::new(vec![TileSpot::new("sales", 2, 1)]);
    TileLayout::new(spots.binding())
        .columns(3)
        .row_height(110.0)
        .tile(Tile::new("sales", "Sales", Stat::new("This month", "R$ 48,200")))
        .tile(
            Tile::new("bills", "Bills", Stat::new("Open", "12"))
                .accessory(Badge::new("3 late").tone(Tone::Danger)),
        )
        .tile(Tile::new("stock", "Stock", Stat::new("Items", "1,204")))
        .tile(Tile::new("people", "People", Stat::new("Active", "37")))
}
// end

// sample: CardView, 340
fn card_view() -> impl View {
    let selected = State::new(std::collections::BTreeSet::from([1]));
    CardView::new(10_000, |i| -> AnyView {
        Box::new(
            VStack::new((
                Text::new(format!("Customer {i}")).font_weight(Weight::SEMIBOLD),
                Text::new(format!("R$ {},00", 100 + i * 7)).foreground(Semantic::InkMuted),
            ))
            .spacing(4.0),
        )
    })
    .card_name(|i| format!("Customer {i}"))
    .card_width(180.0)
    .card_height(84.0)
    .selection(selected.binding())
    .height(300.0)
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

// sample: ChartReference, 280
fn chart_reference() -> impl View {
    Chart::new(ChartKind::Bar)
        .categories(["Jan", "Feb", "Mar", "Apr", "May", "Jun"])
        .series("Sales", [12.0, 18.0, 15.0, 21.0, 19.0, 24.0])
        .show_value_axis()
        .grid_dashed()
        .value_domain(0.0, 25.0)
        .tooltip(ChartTooltip::new().value_format(|value| format!("{value:.0} units")))
        .reference(ChartReference::new(20.0).label("Target").color(Semantic::Warning))
        .height(260.0)
}
// end

// sample: SankeyChart, 340
fn sankey_chart() -> impl View {
    SankeyChart::new()
        .title("Where the money goes")
        .flow("Salary", "Taxes", 1200.0)
        .flow("Salary", "Home", 1500.0)
        .flow("Salary", "Savings", 800.0)
        .flow("Freelance", "Savings", 600.0)
        .flow("Home", "Rent", 1100.0)
        .flow("Home", "Power", 400.0)
        .flow("Savings", "Funds", 900.0)
        .flow("Savings", "Stocks", 500.0)
        .format(|value| format!("$ {value:.0}"))
        .height(300.0)
}
// end

// sample: FlowCanvas, 460
fn flow_canvas() -> impl View {
    let graph = State::new(
        FlowGraph::new()
            .node(
                FlowNode::new("sale", "trigger", "New sale", 0.0, 96.0)
                    .subtitle("Registers 1 and 2")
                    .port(FlowPort::output("out").kind("sale")),
            )
            .node(
                FlowNode::new("stock", "action", "Check stock", 264.0, 96.0)
                    .subtitle("For each item")
                    .port(FlowPort::input("in").kind("sale"))
                    .port(FlowPort::output("out").kind("sale")),
            )
            .node(
                FlowNode::new("enough", "condition", "Enough in stock?", 528.0, 96.0)
                    .port(FlowPort::input("in").kind("sale"))
                    .port(FlowPort::output("yes").side(PortSide::Bottom).kind("sale"))
                    .port(FlowPort::output("no").side(PortSide::Bottom).kind("shortage")),
            )
            .node(
                FlowNode::new("invoice", "action", "Issue receipt", 360.0, 312.0)
                    .port(FlowPort::input("in").side(PortSide::Top).kind("sale")),
            )
            .node(
                FlowNode::new("warn", "action", "Tell the manager", 648.0, 312.0)
                    .status(FlowNodeStatus::Warning("No chat chosen".into()))
                    .port(FlowPort::input("in").side(PortSide::Top).kind("shortage")),
            )
            .edge(
                FlowEdge::new("e1", FlowEndpoint::new("sale", "out"), FlowEndpoint::new("stock", "in"))
                    .animated(true),
            )
            .edge(FlowEdge::new("e2", FlowEndpoint::new("stock", "out"), FlowEndpoint::new("enough", "in")))
            .edge(
                FlowEdge::new("e3", FlowEndpoint::new("enough", "yes"), FlowEndpoint::new("invoice", "in"))
                    .label("Yes"),
            )
            .edge(
                FlowEdge::new("e4", FlowEndpoint::new("enough", "no"), FlowEndpoint::new("warn", "in"))
                    .shape(EdgeShape::Step)
                    .label("No"),
            ),
    );
    FlowCanvas::new(graph.binding())
        .kind("trigger", FlowKind::new(IconName::Zap, "Trigger").tone(Semantic::Warning))
        .kind("action", FlowKind::new(IconName::Package, "Action"))
        .kind("condition", FlowKind::new(IconName::GitBranch, "Condition").tone(Semantic::Success))
        .validate(|graph, from, to| {
            let carries = |end: &FlowEndpoint| graph.port(end).and_then(|port| port.kind.clone());
            if carries(from) == carries(to) {
                Ok(())
            } else {
                Err(String::from("This input takes something else"))
            }
        })
        .size(880.0, 440.0)
}
// end

// sample: ChartCard, 420
fn chart_card() -> impl View {
    ChartCard::new(
        Chart::new(ChartKind::Area)
            .categories(["Jan", "Feb", "Mar", "Apr", "May", "Jun"])
            .series("Desktop", [186.0, 305.0, 237.0, 73.0, 209.0, 214.0])
            .series("Mobile", [80.0, 200.0, 120.0, 190.0, 130.0, 140.0])
            .stack(ChartStack::Normal)
            .gradient()
            .height(220.0),
    )
    .title("Visitors")
    .description("January to June")
    .trend("Up by 5.2% this month", IconName::TrendingUp)
    .note("All channels")
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

// sample: Timeline, 190
fn timeline() -> impl View {
    Timeline::new(vec![
        TimelineTrack::new(
            "Notes",
            34.0,
            vec![TimelineItem::new(1, 2_000, 3_000, "Callout").selected(true).movable()],
        ),
        TimelineTrack::new(
            "Screen",
            34.0,
            vec![TimelineItem::new(2, 6_000, 4_000, "Zoom").tone(Tone::Success)],
        ),
        TimelineTrack::new(
            "Film",
            44.0,
            vec![TimelineItem::new(3, 0, 14_000, "Take 1").tone(Tone::Accent).trimmable()],
        ),
    ])
    .markers(vec![TimelineMarker { at_ms: 6_000, title: "Chapter two".to_owned() }])
    .position_ms(4_000)
}
// end

// sample: VirtualList, 280
fn virtual_list() -> impl View {
    // Ten thousand rows of different heights: only what shows exists.
    let selected = State::new(None::<usize>);
    VirtualList::new(10_000, |index| {
        let tall = 1 + index % 3;
        let words = vec!["A row that says how tall it is"; tall].join("\n");
        VStack::new((Text::new(format!("Row {index}")), Text::new(words))).padding(8.0)
    })
    .key(|index| index as u64)
    .selection(selected.binding())
    .height(260.0)
}
// end

// sample: PagedDocumentView, 420
fn paged_document_view() -> impl View {
    let sheet = ImageData::from_fn(120, 170, |x, y| {
        // Lines of a page, where a real app would hand over a rendered PDF.
        let line = y % 12 < 3 && (10..110).contains(&x) && (14..156).contains(&y);
        if line { Color::rgb(0.55, 0.55, 0.6) } else { Color::rgb(0.98, 0.98, 0.97) }
    });
    PagedDocumentView::new(
        48,
        |_| Size::new(420.0, 595.0),
        // Page 3 has not been drawn yet: it shows a placeholder of the right size.
        move |page| sheet.clone().filter(|_| page != 2),
    )
    .height(400.0)
}
// end

// sample: FileBrowser, 320
fn file_browser() -> impl View {
    let entries = vec![
        FileEntry::folder("Weddings").modified("Oct 12, 2026", 12),
        FileEntry::folder("Portraits").modified("Sep 3, 2026", 3),
        FileEntry::file("catalog.db", 48_302_112).modified("Nov 2, 2026", 32),
        FileEntry::file("notes.txt", 2_204).modified("Oct 30, 2026", 30).icon(IconName::FileText),
        FileEntry::file("cover.jpg", 3_480_000).modified("Aug 21, 2026", 1).icon(IconName::Image),
    ];
    FileBrowser::new(["Backups", "2026", "Studio"], entries).height(300.0)
}
// end

// sample: SortableList, 210
fn sortable_list() -> impl View {
    let layers = State::new(vec!["Exposure", "Curves", "Grain", "Vignette"]);
    view_fn(move || {
        let shown = layers.get();
        let (names, moved) = (shown.clone(), layers.clone());
        SortableList::new(shown.len(), move |index| Text::new(shown[index]))
            .row_name(move |index| names[index].to_owned())
            .on_reorder(move |from, to| {
                moved.update(|list| {
                    let layer = list.remove(from);
                    list.insert(to, layer);
                });
            })
    })
}
// end

// sample: PropertyRow, 130
fn property_row() -> impl View {
    VStack::new((
        PropertyRow::new("Camera", "NIKON D3100"),
        PropertyRow::new("Lens", ""),
        PropertyRow::with("Status", Badge::new("Edited").tone(Tone::Success)),
    ))
    .spacing(8.0)
    .align(Align::Stretch)
}
// end
