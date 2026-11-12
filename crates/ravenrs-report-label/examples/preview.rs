//! Saves a shelf label as it reaches the printer, one pixel per dot.
//! `cargo run -p ravenrs-report-label --example preview -- etiqueta.png`

#![allow(clippy::unwrap_used, clippy::expect_used)]

use ravenrs_report::ReportEngine;
use ravenrs_report::barcode::Symbology;
use ravenrs_report::labels::{LabelStock, ShelfLabelFields, shelf_label};
use ravenrs_report_label::{bitmap_to_png, rasterize_page};
use serde_json::json;

fn main() {
    let path = std::env::args().nth(1).unwrap_or_else(|| "etiqueta.png".to_owned());
    let stock = LabelStock::roll("Rolo 60 x 40 mm", 60.0, 40.0);
    let fields = ShelfLabelFields {
        description: "[Nome]".to_owned(),
        price: "[Preco]".to_owned(),
        price_format: "C2".to_owned(),
        barcode: "[Codigo]".to_owned(),
        symbology: Symbology::Ean13,
    };
    let report = shelf_label(&stock, "Gôndola", &fields).culture("pt-BR");
    let mut engine = ReportEngine::new();
    let data = json!([{ "Nome": "Açúcar União Orgânico", "Preco": 12.9, "Codigo": "7891234567895" }]);
    let document = engine.render(&report, &data);
    let bitmap = rasterize_page(&document.pages[0], 203, engine.text_system()).unwrap();
    std::fs::write(&path, bitmap_to_png(&bitmap)).unwrap();
    println!("{path}: {} x {} pontos", bitmap.width(), bitmap.height());
}
