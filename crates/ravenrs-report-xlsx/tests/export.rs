//! The workbook holds real values, on a grid that follows the page.
#![allow(clippy::unwrap_used, clippy::expect_used, missing_docs)]

use std::io::{Cursor, Read};

use ravenrs_report::{Band, Control, Document, Report, ReportEngine, samples};
use ravenrs_report_xlsx::{to_csv, to_xlsx};
use serde_json::json;

fn part(xlsx: &[u8], name: &str) -> String {
    let mut archive = zip::ZipArchive::new(Cursor::new(xlsx)).unwrap();
    let mut out = String::new();
    archive.by_name(name).unwrap().read_to_string(&mut out).unwrap();
    out
}

fn ledger(rows: usize) -> Document {
    let report = Report::new("Razão: 2026")
        .culture("pt-BR")
        .page_header(Band::new(20.0).control(Control::label("CABEÇALHO").at(0.0, 0.0, 200.0, 20.0)))
        .page_footer(Band::new(20.0).control(Control::label("RODAPÉ").at(0.0, 0.0, 200.0, 20.0)))
        .detail(
            Band::new(100.0)
                .control(Control::field("[Name]").at(0.0, 0.0, 200.0, 100.0))
                .control(Control::field("[Amount]").format("C2").at(200.0, 0.0, 80.0, 100.0))
                .control(Control::field("[When]").format("dd/MM/yyyy").at(280.0, 0.0, 80.0, 100.0)),
        );
    let data: Vec<_> = (0..rows)
        .map(|i| json!({ "Name": format!("Linha {i}"), "Amount": 1000.5 + i as f64, "When": "2026-11-01" }))
        .collect();
    ReportEngine::new().render(&report, &json!(data))
}

#[test]
fn numbers_and_dates_are_values_not_text() {
    let sheet = part(&to_xlsx(&ledger(1)).unwrap(), "xl/worksheets/sheet1.xml");
    assert!(sheet.contains("<v>1000.5</v>"), "{sheet}");
    assert!(sheet.contains("<v>46327</v>"), "{sheet}");
}

#[test]
fn excel_formats_follow_the_report() {
    let styles = part(&to_xlsx(&ledger(1)).unwrap(), "xl/styles.xml");
    assert!(styles.contains("#,##0.00"), "{styles}");
    assert!(styles.contains("[$-416]dd/mm/yyyy"), "{styles}");
}

#[test]
fn pages_flow_into_one_sheet_with_the_page_header_once_and_no_footer() {
    let doc = ledger(40);
    assert!(doc.pages.len() > 1);
    let xlsx = to_xlsx(&doc).unwrap();
    let sheet = part(&xlsx, "xl/worksheets/sheet1.xml");
    let strings = part(&xlsx, "xl/sharedStrings.xml");
    assert!(strings.contains("CABEÇALHO") && !strings.contains("RODAPÉ"), "{strings}");
    assert_eq!(strings.matches("CABEÇALHO").count(), 1);
    assert_eq!(sheet.matches("<v>46327</v>").count(), 40);
    assert!(strings.contains("Linha 39"));
    // One row for the header and one per record, without gaps between pages.
    assert_eq!(sheet.matches("<row ").count(), 41);
}

#[test]
fn sheet_name_is_made_valid() {
    let book = part(&to_xlsx(&ledger(1)).unwrap(), "xl/workbook.xml");
    assert!(book.contains("name=\"Razão  2026\""), "{book}");
}

#[test]
fn csv_has_one_record_per_line_with_the_culture_separator() {
    let csv = to_csv(&ledger(2));
    let lines: Vec<&str> = csv.split("\r\n").collect();
    assert_eq!(lines[0], "CABEÇALHO;;");
    assert_eq!(lines[1], "Linha 0;R$\u{a0}1.000,50;01/11/2026");
    assert_eq!(lines[2], "Linha 1;R$\u{a0}1.001,50;01/11/2026");
    assert_eq!(lines[3], "");
}

#[test]
fn csv_quotes_fields_that_need_it() {
    let report = Report::new("T").detail(
        Band::new(20.0)
            .control(Control::field("[A]").at(0.0, 0.0, 100.0, 20.0))
            .control(Control::field("[B]").at(100.0, 0.0, 100.0, 20.0)),
    );
    let doc = ReportEngine::new().render(&report, &json!([{ "A": "um, dois", "B": "diz \"oi\"" }]));
    assert_eq!(to_csv(&doc), "\"um, dois\",\"diz \"\"oi\"\"\"\r\n");
}

#[test]
fn every_sample_exports_without_overlapping_merges() {
    for report in [samples::invoice(), samples::sales_by_region(500), samples::customer_list()] {
        let data = report.embedded_data().cloned().unwrap_or_default();
        let doc = ReportEngine::new().render(&report, &data);
        assert!(to_xlsx(&doc).is_ok(), "{}", report.name);
        assert!(!to_csv(&doc).is_empty());
    }
}

#[test]
fn an_empty_document_still_gives_a_valid_workbook() {
    assert!(to_xlsx(&Document::default()).is_ok());
    assert_eq!(to_csv(&Document::default()), "");
}
