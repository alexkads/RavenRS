//! The engine end to end: data binding, grouping, summaries, master-detail,
//! pagination, formatting rules and error handling.
#![allow(clippy::expect_used, reason = "test helpers fail loudly")]

use proptest::prelude::*;
use ravenrs_report::wizard::{self, FieldKind};
use ravenrs_report::{
    Band, Control, ControlKind, DataSource, DataSourceKind, DateTime, DetailReport, Document, Group, Item,
    PageSettings, ParameterKind, RenderOptions, Report, ReportEngine, Style, Table, TableRow, samples,
};
use serde_json::{Value as Json, json};

fn render(report: &Report, data: &Json) -> Document {
    ReportEngine::new().render(report, data)
}

fn field(expr: &str) -> Control {
    Control::field(expr).at(0.0, 0.0, 300.0, 18.0)
}

fn sales() -> Json {
    json!([
        { "Region": "Sul", "City": "Curitiba", "Amount": 10 },
        { "Region": "Norte", "City": "Manaus", "Amount": 5 },
        { "Region": "Sul", "City": "Porto Alegre", "Amount": 20 },
        { "Region": "Sul", "City": "Curitiba", "Amount": 1 },
    ])
}

#[test]
fn groups_sort_and_scope_their_summaries() {
    let report = Report::new("r")
        .culture("en-US")
        .report_header(Band::new(20.0).control(field("Count()")))
        .group(
            Group::new("[Region]")
                .header(Band::new(20.0).control(field("'H ' + [Region] + ' ' + ToStr(Count())")))
                .footer(Band::new(20.0).control(field("'F ' + ToStr(Sum([Amount]))"))),
        )
        .sort_by("[Amount]", false)
        .detail(Band::new(20.0).control(field("[City] + ' ' + ToStr([Amount])")))
        .report_footer(Band::new(20.0).control(field("'T ' + ToStr(Sum([Amount]))")));
    let doc = render(&report, &sales());
    assert_eq!(
        doc.page_text(0),
        [
            "4",
            "H Norte 1",
            "Manaus 5",
            "F 5",
            "H Sul 3",
            "Curitiba 1",
            "Curitiba 10",
            "Porto Alegre 20",
            "F 31",
            "T 36"
        ]
    );
    assert!(doc.errors.is_empty());
}

#[test]
fn nested_groups_close_inner_levels_first() {
    let report = Report::new("r")
        .group(Group::new("[Region]").footer(Band::new(20.0).control(field("'R ' + ToStr(Sum([Amount]))"))))
        .group(Group::new("[City]").footer(Band::new(20.0).control(field("'C ' + ToStr(Sum([Amount]))"))))
        .detail(Band::new(0.0));
    assert_eq!(render(&report, &sales()).page_text(0), ["C 5", "R 5", "C 11", "C 20", "R 31"]);
}

#[test]
fn descending_groups_and_filters_with_parameters() {
    let report = Report::new("r")
        .parameter("Min", ParameterKind::Number, json!(5))
        .filter("[Amount] >= ?Min")
        .group(Group { descending: true, ..Group::new("[Region]") })
        .sort_by("[Amount]", true)
        .detail(Band::new(20.0).control(field("[Amount]")));
    assert_eq!(render(&report, &sales()).page_text(0), ["20", "10", "5"]);
    let mut options = RenderOptions::default();
    options.parameters.insert("Min".into(), json!(15));
    let doc = ReportEngine::new().render_with(&report, &sales(), &options);
    assert_eq!(doc.page_text(0), ["20"]);
}

#[test]
fn pages_break_and_number_themselves() {
    let rows: Vec<Json> = (0..100).map(|i| json!({ "N": i })).collect();
    let report = Report::new("r")
        .page(PageSettings::a4())
        .page_header(Band::new(30.0).control(field("'Top'")))
        .page_footer(
            Band::new(30.0).control(field("FormatString('Page {0} of {1}', PageNumber(), PageCount())")),
        )
        .detail(Band::new(30.0).control(field("[N]")));
    let doc = render(&report, &Json::Array(rows));
    // (841.89 - 2 * 42.52 - 60) / 30 = 23 rows per page.
    assert_eq!(doc.pages.len(), 5);
    assert_eq!(doc.page_text(0).len(), 23 + 2);
    assert!(doc.page_contains(0, "Page 1 of 5"));
    assert!(doc.page_contains(4, "Page 5 of 5"));
    assert!(doc.page_text(1).contains(&"23".to_owned()), "the next page continues");
    for p in &doc.pages {
        for item in &p.items {
            if let Item::Text(t) = item {
                assert!(t.clip.max_y() <= p.height - 42.0, "inside the bottom margin: {:?}", t.lines);
            }
        }
    }
}

#[test]
fn page_footers_sum_the_rows_on_their_page() {
    let rows: Vec<Json> = (0..50).map(|_| json!({ "V": 2 })).collect();
    let report = Report::new("r")
        .page_footer(Band::new(30.0).control(field("Sum([V])")))
        .detail(Band::new(100.0).control(field("[V]")));
    let doc = render(&report, &Json::Array(rows));
    // 7 rows per page: (841.89 - 85.04 - 30) / 100.
    assert!(doc.page_text(0).contains(&"14".to_owned()));
    let last = doc.pages.len() - 1;
    assert!(doc.page_text(last).contains(&"2".to_owned()));
}

#[test]
fn report_header_prints_above_the_page_header_on_page_one_only() {
    let rows: Vec<Json> = (0..60).map(|i| json!({ "N": i })).collect();
    let report = Report::new("r")
        .report_header(Band::new(50.0).control(field("'Title'")))
        .page_header(Band::new(20.0).control(field("'Columns'")))
        .detail(Band::new(20.0).control(field("[N]")));
    let doc = render(&report, &Json::Array(rows));
    assert_eq!(doc.page_text(0)[..3], ["Title", "Columns", "0"]);
    assert_eq!(doc.page_text(1)[0], "Columns");
    assert!(!doc.page_contains(1, "Title"));
}

#[test]
fn repeated_headers_and_keep_with_next() {
    let rows: Vec<Json> = (0..80).map(|i| json!({ "G": "A", "N": i })).collect();
    let report = Report::new("r")
        .group(Group::new("[G]").header(Band::new(20.0).control(field("'Group ' + [G]"))).repeat_header())
        .detail(Band::new(20.0).control(field("[N]")));
    let doc = render(&report, &Json::Array(rows));
    assert!(doc.pages.len() >= 2);
    for page in 0..doc.pages.len() {
        assert_eq!(doc.page_text(page)[0], "Group A", "page {page}");
    }

    // A header that would be alone at the bottom moves to the next page with its first row.
    let height = PageSettings::a4().height - 2.0 * PageSettings::a4().margin_top;
    let report = Report::new("r")
        .report_header(Band::new(height - 25.0))
        .group(Group::new("[G]").header(Band::new(20.0).control(field("'Group'"))))
        .detail(Band::new(20.0).control(field("[N]")));
    let doc = render(&report, &json!([{ "G": "A", "N": 1 }]));
    assert_eq!(doc.pages.len(), 2);
    assert_eq!(doc.page_text(1), ["Group", "1"]);
}

#[test]
fn page_break_before_starts_each_group_on_a_new_page() {
    let report = Report::new("r")
        .group(Group::new("[Region]").header(Band::new(20.0).control(field("[Region]"))).page_break_before())
        .detail(Band::new(0.0));
    let doc = render(&report, &sales());
    assert_eq!(doc.pages.len(), 2, "Norte, then Sul");
    assert_eq!(doc.page_text(1), ["Sul"]);
}

#[test]
fn master_detail_binds_nested_rows_and_reaches_the_master() {
    let report = Report::new("r").detail(Band::new(20.0).control(field("[Number]"))).detail_report(
        DetailReport::new("Items")
            .header(Band::new(20.0).control(field("'Items of ' + [Number]")))
            .detail(
                Band::new(20.0).control(field("[^.Number] + '/' + [Sku] + ' ' + ToStr(RunningSum([Qty]))")),
            )
            .footer(Band::new(20.0).control(field("'Total ' + ToStr(Sum([Qty]))"))),
    );
    let data = json!([
        { "Number": "A", "Items": [{ "Sku": "x", "Qty": 2 }, { "Sku": "y", "Qty": 3 }] },
        { "Number": "B", "Items": [] },
    ]);
    let doc = render(&report, &data);
    assert_eq!(doc.page_text(0), ["A", "Items of A", "A/x 2", "A/y 5", "Total 5", "B"]);
}

#[test]
fn text_grows_and_pushes_what_is_below() {
    let long = "A long description that certainly does not fit in one line of this narrow label at all";
    let report = Report::new("r").detail(
        Band::new(40.0)
            .control(Control::label(long).at(0.0, 0.0, 120.0, 18.0))
            .control(Control::label("beside").at(130.0, 0.0, 100.0, 18.0))
            .control(Control::label("below").at(0.0, 22.0, 100.0, 18.0)),
    );
    let doc = render(&report, &json!([{}]));
    let lines: Vec<_> = doc.pages[0]
        .items
        .iter()
        .filter_map(|i| if let Item::Text(t) = i { Some(t) } else { None })
        .collect();
    let of = |s: &str| lines.iter().find(|t| t.lines.iter().any(|l| l.text.trim() == s)).expect(s);
    let grown = lines.iter().find(|t| t.lines.len() > 1).expect("wrapped text");
    assert!(of("below").clip.y() >= grown.clip.max_y() - 0.01, "pushed under the grown label");
    assert!((of("beside").clip.y() - grown.clip.y()).abs() < 0.01, "a neighbor stays on its row");
    assert_eq!(
        grown.lines.iter().map(|l| l.text.clone()).collect::<Vec<_>>().join(""),
        long,
        "wrapped lines keep all the text"
    );
}

#[test]
fn table_rows_grow_to_their_tallest_cell() {
    let long = "This cell has enough text to need several lines in a narrow column";
    let report =
        Report::new("r").detail(Band::new(20.0).control(Control::table(
            Table::new().row(TableRow::new(20.0).text(80.0, long).text(80.0, "short")),
        )));
    let doc = render(&report, &json!([{}]));
    let clips: Vec<_> = doc.pages[0]
        .items
        .iter()
        .filter_map(|i| if let Item::Text(t) = i { Some(t.clip) } else { None })
        .collect();
    assert_eq!(clips.len(), 2);
    assert!(clips[0].height() > 20.0);
    assert_eq!(clips[0].height(), clips[1].height(), "cells of a row share its height");
}

#[test]
fn conditional_rules_visibility_and_zebra() {
    let red = Style::new().color("#ff0000");
    let report = Report::new("r")
        .style("even", Style::new().background("#eeeeee"))
        .style("odd", Style::new().background("#ffffff"))
        .detail(
            Band::new(20.0)
                .control(field("[N]").rule("[N] > 1", red))
                .control(Control::label("hidden").at(0.0, 0.0, 10.0, 10.0).visible("[N] = 99"))
                .zebra("odd", "even"),
        );
    let doc = render(&report, &json!([{ "N": 1 }, { "N": 2 }]));
    assert!(!doc.page_contains(0, "hidden"));
    let texts: Vec<_> = doc.pages[0]
        .items
        .iter()
        .filter_map(|i| if let Item::Text(t) = i { Some(t) } else { None })
        .collect();
    assert!(texts[0].color.r < 0.5 && texts[1].color.r > 0.9, "the rule colors only the second row");
    let fills = doc.pages[0].items.iter().filter(|i| matches!(i, Item::Rect { fill: Some(_), .. })).count();
    assert_eq!(fills, 2, "one stripe per row");
}

#[test]
fn bad_expressions_are_reported_and_shown_without_failing() {
    let report = Report::new("r").detail(Band::new(20.0).control(field("[A] +")).control(field("Sum(")));
    let doc = render(&report, &json!([{ "A": 1 }, { "A": 2 }]));
    assert_eq!(doc.pages.len(), 1);
    assert!(doc.page_contains(0, "#ERRO"));
    assert_eq!(doc.errors.len(), 2, "each problem once, not per row: {:?}", doc.errors);
    assert!(doc.errors[0].contains("[A] +"));

    let missing = render(&Report::new("r").data_member("nope"), &json!({}));
    assert!(missing.errors[0].contains("nope"));
}

#[test]
fn empty_data_still_prints_headers_and_footers() {
    let report = Report::new("r")
        .report_header(Band::new(20.0).control(field("'Header ' + ToStr(Count())")))
        .detail(Band::new(20.0).control(field("[N]")))
        .report_footer(Band::new(20.0).control(field("'Footer'")));
    assert_eq!(render(&report, &json!([])).page_text(0), ["Header 0", "Footer"]);
    assert_eq!(render(&report, &Json::Null).page_text(0), ["Header 0", "Footer"]);
}

#[test]
fn a_band_taller_than_the_page_does_not_loop_forever() {
    let report = Report::new("r").detail(Band::new(5000.0).control(field("[N]")));
    let doc = render(&report, &json!([{ "N": 1 }, { "N": 2 }, { "N": 3 }]));
    assert_eq!(doc.pages.len(), 3);
}

#[test]
fn dates_and_clock_use_the_culture_and_options() {
    let report = Report::new("r").culture("pt-BR").detail(
        Band::new(20.0)
            .control(field("[D]").format("dd/MM/yyyy"))
            .control(field("Today()").format("D").at(0.0, 20.0, 300.0, 18.0))
            .control(field("[V]").format("C2").at(0.0, 40.0, 300.0, 18.0)),
    );
    let options = RenderOptions { now: DateTime::parse("2026-11-01"), ..RenderOptions::default() };
    let doc =
        ReportEngine::new().render_with(&report, &json!([{ "D": "2026-03-15", "V": 1234.5 }]), &options);
    assert_eq!(doc.page_text(0), ["15/03/2026", "domingo, 1 de novembro de 2026", "R$\u{a0}1.234,50"]);
}

#[test]
fn layouts_round_trip_through_json_with_their_data_sources() {
    for report in [samples::invoice(), samples::sales_by_region(5), samples::customer_list()] {
        let json = report.to_json();
        let back = Report::from_json(&json).expect("valid JSON");
        assert_eq!(back, report);
        assert!(back.embedded_data().is_some(), "the data travels with the layout");
    }
    let by_hand = r##"{
        "name": "Mínimo",
        "dataSources": [{ "name": "D", "type": "json", "data": [{ "A": 1 }] }],
        "detail": { "height": 20, "controls": [
            { "type": "label", "expression": "[A]", "x": 0, "y": 0, "width": 100, "height": 18, "style": { "bold": true, "color": "#112233" } }
        ] }
    }"##;
    let report = Report::from_json(by_hand).expect("a hand-written layout");
    assert!(matches!(report.detail.controls[0].kind, ControlKind::Label(_)));
    let doc = render(&report, report.embedded_data().expect("embedded"));
    assert_eq!(doc.page_text(0), ["1"]);
    let url = r#"{ "dataSources": [{ "name": "api", "type": "url", "url": "https://example.com/d.json" }] }"#;
    let report = Report::from_json(url).expect("url source");
    assert!(matches!(report.data_sources[0].kind, DataSourceKind::Url { .. }));
    assert!(report.embedded_data().is_none());
}

#[test]
fn samples_render_without_errors() {
    let mut engine = ReportEngine::new();
    for report in [samples::invoice(), samples::sales_by_region(500), samples::customer_list()] {
        let data = report.embedded_data().cloned().expect("sample data");
        let doc = engine.render(&report, &data);
        assert!(doc.errors.is_empty(), "{}: {:?}", report.name, doc.errors);
        assert!(!doc.pages.is_empty());
        assert!(doc.page_svg(0).starts_with("<svg"));
    }
    let doc = engine.render(&samples::invoice(), &samples::invoice_data());
    assert!(doc.page_contains(0, "R$\u{a0}45.597,60"), "{:?}", doc.page_text(0));
}

#[test]
fn wizard_output_is_a_working_report() {
    let data = samples::sales_data(50);
    let fields = wizard::fields(&data, None);
    assert!(fields.iter().any(|f| f.path == "Quantity" && f.kind == FieldKind::Number));
    let columns = fields
        .iter()
        .filter(|f| ["Customer", "Quantity", "UnitPrice"].contains(&f.path.as_str()))
        .cloned()
        .collect();
    let options = wizard::WizardOptions {
        title: "Sales".into(),
        culture: "en-US".into(),
        columns,
        group_by: Some("Region".into()),
        totals: vec!["Quantity".into()],
        ..wizard::WizardOptions::default()
    };
    let report = wizard::table_report(
        DataSource { name: "S".into(), kind: DataSourceKind::Json { data: data.clone() } },
        &options,
    );
    let doc = render(&report, &data);
    assert!(doc.errors.is_empty(), "{:?}", doc.errors);
    assert_eq!(doc.page_text(0)[0], "Sales", "the title comes first");
    assert!(doc.page_text(0).iter().any(|l| l == "QUANTITY"));
}

#[test]
fn engines_accept_custom_functions_and_unknown_ones_are_reported() {
    let report = Report::new("r").detail(
        Band::new(20.0)
            .control(field("Discount([V])"))
            .control(field("Nope([V])").at(0.0, 20.0, 100.0, 18.0)),
    );
    let mut engine = ReportEngine::new();
    engine.register_function("Discount", |args| {
        Ok(ravenrs_report::Value::Number(args[0].as_number().unwrap_or(0.0) * 0.9))
    });
    let doc = engine.render(&report, &json!([{ "V": 100 }]));
    assert_eq!(doc.page_text(0), ["90", "#ERRO"]);
    assert!(doc.errors[0].contains("unknown function 'Nope'"), "{:?}", doc.errors);
}

#[test]
fn print_html_has_one_sheet_per_page_at_the_paper_size_and_escapes_text() {
    let rows: Vec<Json> = (0..100).map(|i| json!({ "N": format!("<b>{i}</b> & co") })).collect();
    let report = Report::new("A & B").detail(Band::new(30.0).control(field("[N]")));
    let doc = render(&report, &Json::Array(rows));
    assert!(doc.pages.len() > 1);
    let html = doc.print_html();
    assert_eq!(html.matches("class=\"sheet\"").count(), doc.pages.len());
    assert!(html.contains("@page{size:595.28pt 841.89pt;margin:0}"));
    assert!(html.contains("<title>A &amp; B</title>"));
    assert!(html.contains("&lt;b&gt;0&lt;/b&gt; &amp; co"));
    assert!(!html.contains("<b>0</b>"), "data never becomes markup");
    assert!(html.contains("font-family:Inter;font-weight:600"), "the font is embedded");
}

proptest! {
    /// Every row prints exactly once and group subtotals add up to the grand total.
    #[test]
    fn rows_print_once_and_totals_add_up(values in proptest::collection::vec((0u8..5, 0i32..1000), 0..300)) {
        let rows: Vec<Json> = values.iter().enumerate().map(|(i, (g, v))| json!({ "Id": format!("row{i}"), "G": g, "V": v })).collect();
        let report = Report::new("r")
            .group(Group::new("[G]").footer(Band::new(18.0).control(field("'sub=' + ToStr(Sum([V]))"))))
            .detail(Band::new(18.0).control(field("[Id]")))
            .report_footer(Band::new(18.0).control(field("'all=' + ToStr(Sum([V]))")));
        let doc = render(&report, &Json::Array(rows));
        let text: Vec<String> = (0..doc.pages.len()).flat_map(|p| doc.page_text(p)).collect();
        let printed = text.iter().filter(|l| l.starts_with("row")).count();
        prop_assert_eq!(printed, values.len());
        let num = |l: &str, prefix: &str| l.strip_prefix(prefix).and_then(|n| n.parse::<f64>().ok());
        let subtotal: f64 = text.iter().filter_map(|l| num(l, "sub=")).sum();
        let total: f64 = text.iter().find_map(|l| num(l, "all=")).unwrap_or(0.0);
        prop_assert_eq!(subtotal, total);
        prop_assert_eq!(total, values.iter().map(|(_, v)| f64::from(*v)).sum::<f64>());
    }

    /// Expressions never panic on arbitrary input.
    #[test]
    fn parser_never_panics(src in "\\PC{0,60}") {
        let _ = ravenrs_report::Criteria::parse(&src);
    }
}

fn png_base64(width: u32, height: u32, alpha: u8) -> String {
    let image = image::RgbaImage::from_pixel(width, height, image::Rgba([200, 30, 30, alpha]));
    let mut bytes = std::io::Cursor::new(Vec::new());
    image.write_to(&mut bytes, image::ImageFormat::Png).expect("encodes");
    encode_base64(&bytes.into_inner())
}

fn encode_base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let n = u32::from(chunk[0]) << 16
            | u32::from(chunk.get(1).copied().unwrap_or(0)) << 8
            | u32::from(chunk.get(2).copied().unwrap_or(0));
        out.push(TABLE[(n >> 18) as usize & 63] as char);
        out.push(TABLE[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 { TABLE[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if chunk.len() > 2 { TABLE[n as usize & 63] as char } else { '=' });
    }
    out
}

fn one_row(control: ravenrs_report::Control) -> ravenrs_report::Document {
    let report = ravenrs_report::Report::new("T").detail(ravenrs_report::Band::new(100.0).control(control));
    ravenrs_report::ReportEngine::new().render(&report, &serde_json::json!([{ "A": "x" }]))
}

fn image_of(doc: &ravenrs_report::Document) -> (ravenrs_core::Rect, ravenrs_core::Rect) {
    doc.pages[0]
        .items
        .iter()
        .find_map(|i| match i {
            ravenrs_report::Item::Image { rect, clip, .. } => Some((*rect, *clip)),
            _ => None,
        })
        .expect("an image")
}

#[test]
fn pictures_fit_their_box_by_mode() {
    use ravenrs_report::{Control, ControlKind, Picture, PictureMode};
    let doc_with = |mode, source: String| {
        one_row(
            Control {
                kind: ControlKind::Picture(Picture { source, mode, ..Picture::default() }),
                ..Control::default()
            }
            .at(0.0, 0.0, 100.0, 100.0),
        )
    };
    let wide = png_base64(40, 20, 255);
    let (zoom, clip) = image_of(&doc_with(PictureMode::Zoom, wide.clone()));
    assert_eq!((zoom.width(), zoom.height()), (100.0, 50.0), "as large as fits");
    assert!((zoom.y() - clip.y() - 25.0).abs() < 0.01, "centered");
    let (squeeze, _) = image_of(&doc_with(PictureMode::Squeeze, wide.clone()));
    assert_eq!((squeeze.width(), squeeze.height()), (40.0, 20.0), "not enlarged");
    let (stretch, clip) = image_of(&doc_with(PictureMode::Stretch, wide.clone()));
    assert_eq!(stretch, clip);
    let (normal, clip) = image_of(&doc_with(PictureMode::Normal, png_base64(400, 300, 255)));
    assert_eq!((normal.width(), normal.height()), (400.0, 300.0), "cut at the box");
    assert_eq!((normal.x(), normal.y()), (clip.x(), clip.y()));
}

#[test]
fn pictures_come_from_data_addresses_base64_and_fields() {
    use ravenrs_report::{Control, ControlKind, Picture};
    let base64 = png_base64(4, 4, 255);
    image_of(&one_row(Control::picture(format!("data:image/png;base64,{base64}"))));
    image_of(&one_row(Control::picture(base64.clone())));
    let from_field = Control {
        kind: ControlKind::Picture(Picture { expression: Some("[Photo]".into()), ..Picture::default() }),
        ..Control::default()
    };
    let report = ravenrs_report::Report::new("T").detail(ravenrs_report::Band::new(50.0).control(from_field));
    let doc = ravenrs_report::ReportEngine::new().render(&report, &serde_json::json!([{ "Photo": base64 }]));
    image_of(&doc);
}

#[test]
fn a_bad_picture_is_reported_and_the_report_goes_on() {
    for source in ["not base64 !!", "aGVsbG8gd29ybGQ=", "data:image/png;base64,aGVsbG8="] {
        let doc = one_row(ravenrs_report::Control::picture(source));
        assert!(doc.errors.iter().any(|e| e.contains("image")), "{source}: {:?}", doc.errors);
        assert!(!doc.pages[0].items.iter().any(|i| matches!(i, ravenrs_report::Item::Image { .. })));
    }
}

#[test]
fn an_image_used_on_every_row_is_decoded_once() {
    let source = png_base64(8, 8, 255);
    let report = ravenrs_report::Report::new("T").detail(
        ravenrs_report::Band::new(20.0)
            .control(ravenrs_report::Control::picture(source).at(0.0, 0.0, 20.0, 20.0)),
    );
    let doc = ravenrs_report::ReportEngine::new().render(&report, &serde_json::json!([{}, {}, {}]));
    let ids: std::collections::HashSet<u64> = doc.pages[0]
        .items
        .iter()
        .filter_map(|i| match i {
            ravenrs_report::Item::Image { image, .. } => Some(image.pixels().id()),
            _ => None,
        })
        .collect();
    assert_eq!(ids.len(), 1);
}

#[test]
fn links_only_open_safe_addresses() {
    let links = |url: &str| {
        let doc = one_row(ravenrs_report::Control::link("abrir", url).at(10.0, 5.0, 100.0, 20.0));
        let found: Vec<String> = doc.pages[0]
            .items
            .iter()
            .filter_map(|i| match i {
                ravenrs_report::Item::Link { url, .. } => Some(url.clone()),
                _ => None,
            })
            .collect();
        (found, doc.errors)
    };
    for good in ["https://example.com/a?b=1", "http://example.com", "mailto:ana@example.com"] {
        assert_eq!(links(good).0, [good]);
    }
    for bad in [
        "javascript:alert(1)",
        "file:///etc/passwd",
        "JAVASCRIPT:alert(1)",
        "data:text/html,x",
        "https://a\nb",
    ] {
        let (found, errors) = links(bad);
        assert!(found.is_empty(), "{bad}");
        assert!(!errors.is_empty(), "{bad}");
    }
    assert_eq!(links("").1, Vec::<String>::new(), "no address is not a mistake");
}

#[test]
fn links_can_come_from_data_and_svg_escapes_them() {
    use ravenrs_report::{Control, ControlKind, Label, LinkLabel};
    let control = Control {
        kind: ControlKind::Link(LinkLabel {
            label: Label { text: "site".into(), ..Label::default() },
            url_expression: Some("'https://example.com/?a=' + [A] + '&b=\"'".into()),
            ..LinkLabel::default()
        }),
        ..Control::default()
    };
    let doc = one_row(control);
    assert_eq!(doc.page_text(0), ["site"], "the link prints its text");
    let svg = doc.page_svg(0);
    assert!(svg.contains(r#"href="https://example.com/?a=x&amp;b=&quot;""#), "{svg}");
}

#[test]
fn bookmarks_print_nothing_and_take_no_room() {
    let report = ravenrs_report::Report::new("T").detail(
        ravenrs_report::Band::new(20.0)
            .control(ravenrs_report::Control::bookmark("[A]", 1))
            .control(ravenrs_report::Control::field("[A]").at(0.0, 0.0, 100.0, 20.0)),
    );
    let doc = ravenrs_report::ReportEngine::new()
        .render(&report, &serde_json::json!([{ "A": "um" }, { "A": "dois" }]));
    assert_eq!(doc.page_text(0), ["um", "dois"]);
    let marks: Vec<(String, u32)> = doc.pages[0]
        .items
        .iter()
        .filter_map(|i| match i {
            ravenrs_report::Item::Bookmark { title, level, .. } => Some((title.clone(), *level)),
            _ => None,
        })
        .collect();
    assert_eq!(marks, [("um".to_owned(), 1), ("dois".to_owned(), 1)]);
}

#[test]
fn the_new_controls_survive_a_trip_through_json() {
    use ravenrs_report::{Control, Report};
    let report = Report::new("T").detail(
        ravenrs_report::Band::new(20.0)
            .control(Control::picture("abc"))
            .control(Control::link("a", "https://b"))
            .control(Control::bookmark("[C]", 2)),
    );
    let again = Report::from_json(&report.to_json()).expect("reads back");
    assert_eq!(again, report);
}

#[test]
fn barcodes_draw_marks_with_their_text_and_report_bad_content() {
    use ravenrs_report::barcode::Symbology;
    use ravenrs_report::{Band, Control, Report, ReportEngine};
    let report = Report::new("T").detail(
        Band::new(60.0).control(Control::barcode(Symbology::Ean13, "[Sku]").at(0.0, 0.0, 200.0, 60.0)),
    );
    let mut engine = ReportEngine::new();
    let doc = engine.render(&report, &serde_json::json!([{ "Sku": "400638133393" }, { "Sku": "abc" }, {}]));
    let marks =
        doc.pages[0].items.iter().filter(|i| matches!(i, ravenrs_report::Item::Vector { .. })).count();
    assert_eq!(marks, 1, "only the valid row draws");
    assert_eq!(doc.page_text(0), ["4006381333931"], "the check digit is printed");
    assert_eq!(doc.errors.len(), 1, "a bad code is reported, an empty one is not: {:?}", doc.errors);
    assert!(doc.errors[0].contains("EAN-13"), "{:?}", doc.errors);
    let again = Report::from_json(&report.to_json()).expect("reads back");
    assert_eq!(again, report);
}

#[test]
fn a_qr_code_prints_no_text_and_stays_square() {
    use ravenrs_report::barcode::Symbology;
    use ravenrs_report::{Band, Control, Report, ReportEngine};
    let report =
        Report::new("T")
            .detail(Band::new(100.0).control(
                Control::barcode(Symbology::Qr, "'https://example.com'").at(0.0, 0.0, 200.0, 100.0),
            ));
    let doc = ReportEngine::new().render(&report, &serde_json::json!([{}]));
    assert!(doc.page_text(0).is_empty());
    assert!(doc.page_svg(0).contains("<path"));
}

fn dashboard_items() -> (ravenrs_report::Document, usize) {
    let report = ravenrs_report::samples::sales_dashboard();
    let data = report.embedded_data().cloned().expect("embedded data");
    let doc = ravenrs_report::ReportEngine::new().render(&report, &data);
    let charts =
        doc.pages[0].items.iter().filter(|i| matches!(i, ravenrs_report::Item::Vector { .. })).count();
    (doc, charts)
}

#[test]
fn the_dashboard_sample_draws_its_three_charts_without_errors() {
    let (doc, charts) = dashboard_items();
    assert_eq!(charts, 3);
    assert!(doc.errors.is_empty(), "{:?}", doc.errors);
    let text = doc.page_text(0).join(" | ");
    assert!(
        text.contains("Vendas por mês") && text.contains("Por região") && text.contains("Por produto"),
        "{text}"
    );
    assert!(doc.page_svg(0).contains("<path"));
}

fn chart_report(chart: ravenrs_report::Chart) -> ravenrs_report::Document {
    let control = ravenrs_report::Control {
        kind: ravenrs_report::ControlKind::Chart(chart),
        ..ravenrs_report::Control::default()
    }
    .at(0.0, 0.0, 400.0, 200.0);
    let report =
        ravenrs_report::Report::new("T").report_header(ravenrs_report::Band::new(200.0).control(control));
    ravenrs_report::ReportEngine::new().render(
        &report,
        &serde_json::json!([
            { "R": "Sul", "V": 10, "Items": [{ "Q": 1 }, { "Q": 5 }] },
            { "R": "Norte", "V": 20 },
            { "R": "Sul", "V": 5 }
        ]),
    )
}

#[test]
fn rows_with_the_same_argument_add_up_into_one_point() {
    use ravenrs_report::chart::ChartKind;
    use ravenrs_report::{Chart, ChartSeries};
    let doc = chart_report(Chart {
        kind: ChartKind::Pie,
        argument: Some("[R]".into()),
        series: vec![ChartSeries::new("", "[V]")],
        ..Chart::default()
    });
    let text = doc.page_text(0);
    // 15 of 35 and 20 of 35.
    assert!(text.contains(&"43%".to_owned()) && text.contains(&"57%".to_owned()), "{text:?}");
    assert!(text.contains(&"Sul".to_owned()) && text.contains(&"Norte".to_owned()), "{text:?}");
}

#[test]
fn sorted_points_follow_their_argument() {
    use ravenrs_report::chart::ChartKind;
    use ravenrs_report::{Chart, ChartSeries};
    let positions = |sorted| {
        let doc = chart_report(Chart {
            kind: ChartKind::Bar,
            argument: Some("[R]".into()),
            series: vec![ChartSeries::new("", "[V]")],
            sorted,
            ..Chart::default()
        });
        let at = |name: &str| {
            doc.pages[0].items.iter().find_map(|item| match item {
                ravenrs_report::Item::Text(t) if t.lines.iter().any(|l| l.text == name) => Some(t.clip.x()),
                _ => None,
            })
        };
        (at("Sul").expect("Sul"), at("Norte").expect("Norte"))
    };
    let (sul, norte) = positions(false);
    assert!(sul < norte, "first seen first");
    let (sul, norte) = positions(true);
    assert!(norte < sul, "alphabetical");
}

#[test]
fn a_bad_series_expression_is_reported_and_a_chart_without_data_is_empty() {
    use ravenrs_report::{Chart, ChartSeries};
    let doc = chart_report(Chart {
        argument: Some("[R]".into()),
        series: vec![ChartSeries::new("", "Nope(")],
        ..Chart::default()
    });
    assert!(!doc.errors.is_empty());
    let doc = chart_report(Chart {
        data_member: Some("Missing".into()),
        series: vec![ChartSeries::new("", "[Q]")],
        ..Chart::default()
    });
    assert!(doc.errors.is_empty(), "{:?}", doc.errors);
}
