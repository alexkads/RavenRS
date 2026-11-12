//! Reads the codes of real shelf labels with `rxing`, a decoder that shares nothing with our encoder,
//! from the picture that goes to the printer. A code that does not fit must be an explicit error, and
//! the table below is what really happens at 203 dpi.

#![allow(clippy::unwrap_used, clippy::expect_used, missing_docs)]

use ravenrs_report::ReportEngine;
use ravenrs_report::barcode::{Pattern, Symbology, encode};
use ravenrs_report::labels::{LabelStock, ShelfLabelFields, shelf_label};
use ravenrs_report_label::{Bitmap, LabelError, LabelJob, Language, compile, rasterize_page};
use rxing::BarcodeFormat;
use rxing::helpers::detect_in_luma;
use serde_json::json;

const DPI: u32 = 203;

struct Case {
    symbology: Symbology,
    format: BarcodeFormat,
    content: &'static str,
    fits_40_x_25: bool,
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            symbology: Symbology::Ean13,
            format: BarcodeFormat::EAN_13,
            content: "789123456789",
            fits_40_x_25: false,
        },
        Case {
            symbology: Symbology::Ean8,
            format: BarcodeFormat::EAN_8,
            content: "7891234",
            fits_40_x_25: true,
        },
        Case {
            symbology: Symbology::UpcA,
            format: BarcodeFormat::UPC_A,
            content: "03600029145",
            fits_40_x_25: false,
        },
        Case {
            symbology: Symbology::Code128,
            format: BarcodeFormat::CODE_128,
            content: "ABC-1234",
            fits_40_x_25: true,
        },
        Case {
            symbology: Symbology::Itf14,
            format: BarcodeFormat::ITF,
            content: "1789123456789",
            fits_40_x_25: false,
        },
        Case {
            symbology: Symbology::Qr,
            format: BarcodeFormat::QR_CODE,
            content: "RAVENRS",
            fits_40_x_25: true,
        },
    ]
}

fn stocks() -> [LabelStock; 2] {
    [LabelStock::roll("Rolo 40 x 25 mm", 40.0, 25.0), LabelStock::roll("Rolo 60 x 40 mm", 60.0, 40.0)]
}

fn label_document(stock: &LabelStock, case: &Case) -> (ravenrs_report::Document, ReportEngine) {
    let fields = ShelfLabelFields {
        description: "[Nome]".to_owned(),
        price: "[Preco]".to_owned(),
        price_format: "C2".to_owned(),
        barcode: "[Codigo]".to_owned(),
        symbology: case.symbology,
    };
    let report = shelf_label(stock, "Gôndola", &fields);
    let mut engine = ReportEngine::new();
    let data = json!([{ "Nome": "Açúcar União", "Preco": 12.9, "Codigo": case.content }]);
    let document = engine.render(&report, &data);
    assert!(document.errors.is_empty(), "{:?}", document.errors);
    assert_eq!(document.pages.len(), 1);
    (document, engine)
}

fn luma(bitmap: &Bitmap) -> Vec<u8> {
    let mut pixels = Vec::with_capacity((bitmap.width() * bitmap.height()) as usize);
    for y in 0..bitmap.height() {
        for x in 0..bitmap.width() {
            pixels.push(if bitmap.get(x, y) { 0 } else { 255 });
        }
    }
    pixels
}

/// What a scanner should give back: linear codes with their check digit, as encoded.
fn expected_text(case: &Case) -> String {
    match encode(case.symbology, case.content).unwrap() {
        Pattern::Linear { text, .. } => text,
        _ => case.content.to_owned(),
    }
}

#[test]
fn every_code_reads_back_or_says_it_does_not_fit() {
    for stock in stocks() {
        let small = stock.width < 150.0;
        for case in cases() {
            let (document, mut engine) = label_document(&stock, &case);
            let result = rasterize_page(&document.pages[0], DPI, engine.text_system());
            let should_fit = !small || case.fits_40_x_25;
            match result {
                Ok(bitmap) => {
                    assert!(should_fit, "{} {:?} was expected not to fit", stock.name, case.symbology);
                    let read =
                        detect_in_luma(luma(&bitmap), bitmap.width(), bitmap.height(), Some(case.format))
                            .unwrap_or_else(|e| panic!("{} {:?}: {e}", stock.name, case.symbology));
                    assert_eq!(read.getText(), expected_text(&case), "{} {:?}", stock.name, case.symbology);
                    println!("{}: {:?} lido: {}", stock.name, case.symbology, read.getText());
                }
                Err(error) => {
                    assert!(!should_fit, "{} {:?}: {error}", stock.name, case.symbology);
                    assert!(matches!(error, LabelError::BarcodeDoesNotFit { .. }), "{error:?}");
                    println!("{}: {:?} erro: {error}", stock.name, case.symbology);
                }
            }
        }
    }
}

#[test]
fn compile_passes_the_error_on_and_names_the_label() {
    let stock = LabelStock::roll("Rolo 40 x 25 mm", 40.0, 25.0);
    let case = Case {
        symbology: Symbology::Ean13,
        format: BarcodeFormat::EAN_13,
        content: "789123456789",
        fits_40_x_25: false,
    };
    let (document, _) = label_document(&stock, &case);
    let error = compile(&document, &LabelJob::new(Language::Zpl, DPI)).unwrap_err();
    let text = error.to_string();
    assert!(text.starts_with("etiqueta 1: o código EAN-13"), "{text}");
    assert!(matches!(error, LabelError::OnLabel { label: 1, .. }));
}

#[test]
fn the_bars_on_the_label_are_whole_multiples_of_one_module() {
    let stock = LabelStock::roll("Rolo 60 x 40 mm", 60.0, 40.0);
    let case = Case {
        symbology: Symbology::Ean13,
        format: BarcodeFormat::EAN_13,
        content: "789123456789",
        fits_40_x_25: false,
    };
    let (document, mut engine) = label_document(&stock, &case);
    let bitmap = rasterize_page(&document.pages[0], DPI, engine.text_system()).unwrap();
    // A row through the bars: the one with the most changes between black and white.
    let changes = |y: u32| (1..bitmap.width()).filter(|&x| bitmap.get(x, y) != bitmap.get(x - 1, y)).count();
    let row = (0..bitmap.height()).max_by_key(|&y| changes(y)).unwrap();
    let mut widths = Vec::new();
    let mut run = 1;
    for x in 1..bitmap.width() {
        if bitmap.get(x, row) == bitmap.get(x - 1, row) {
            run += 1;
        } else {
            widths.push(run);
            run = 1;
        }
    }
    widths.push(run);
    // The margins on each side are not bars.
    let bars = &widths[1..widths.len() - 1];
    assert_eq!(bars.len(), 59, "EAN-13 has 30 bars and 29 spaces");
    assert!(bars.iter().all(|width| width % 3 == 0), "{bars:?}");
}
