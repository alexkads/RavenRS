//! A household budget in the RavenRS spreadsheet, shared by the native and the web build.

use ravenrs::prelude::*;
use ravenrs_spreadsheet::{Spreadsheet, SpreadsheetState};
use serde_json::json;

/// Category, budget and spent, for each line of the sample.
const LINES: [(&str, f64, f64); 8] = [
    ("Moradia", 2400.0, 2400.0),
    ("Mercado", 1300.0, 1452.37),
    ("Transporte", 600.0, 488.9),
    ("Saúde", 450.0, 512.0),
    ("Educação", 900.0, 900.0),
    ("Lazer", 400.0, 615.5),
    ("Contas da casa", 520.0, 498.13),
    ("Reserva", 800.0, 800.0),
];

/// A budget with formulas, formats and a second sheet that reads the first.
pub fn sample() -> SpreadsheetState {
    let sheet = SpreadsheetState::new();
    sheet.set_file_name("Orçamento");
    let set = |cell: &str, input: &str| {
        let _ = sheet.set_cell(cell, input);
    };
    let run = |command: &str, params: serde_json::Value| {
        let _ = sheet.execute(command, params);
    };
    run("sheet.rename", json!({ "name": "Orçamento" }));
    set("A1", "Orçamento de novembro");
    run("home.bold", json!({ "range": "A1", "on": true }));
    run("home.fontSize", json!({ "range": "A1", "size": 16 }));
    for (cell, title) in
        [("A3", "Categoria"), ("B3", "Previsto"), ("C3", "Gasto"), ("D3", "Diferença"), ("E3", "% usado")]
    {
        set(cell, title);
    }
    run("home.bold", json!({ "range": "A3:E3", "on": true }));
    run("home.fillColor", json!({ "range": "A3:E3", "color": "#DCE6F1" }));
    run("home.borders", json!({ "range": "A3:E3", "border": "bottom" }));
    for (i, (name, budget, spent)) in LINES.iter().enumerate() {
        let row = i + 4;
        set(&format!("A{row}"), name);
        set(&format!("B{row}"), &budget.to_string());
        set(&format!("C{row}"), &spent.to_string());
        set(&format!("D{row}"), &format!("=B{row}-C{row}"));
        set(&format!("E{row}"), &format!("=C{row}/B{row}"));
    }
    let last = LINES.len() + 3;
    let total = last + 1;
    set(&format!("A{total}"), "Total");
    for col in ["B", "C", "D"] {
        set(&format!("{col}{total}"), &format!("=SUM({col}4:{col}{last})"));
    }
    set(&format!("E{total}"), &format!("=C{total}/B{total}"));
    run("home.bold", json!({ "range": format!("A{total}:E{total}"), "on": true }));
    run("home.borders", json!({ "range": format!("A{total}:E{total}"), "border": "top" }));
    run(
        "home.numberFormat",
        json!({ "range": format!("B4:D{total}"), "code": "\"R$\" #,##0.00;[Red]-\"R$\" #,##0.00" }),
    );
    run("home.numberFormat", json!({ "range": format!("E4:E{total}"), "code": "0%" }));
    run("home.columnWidth", json!({ "cols": "A:A", "width": 150 }));
    run("home.columnWidth", json!({ "cols": "B:D", "width": 110 }));

    run("home.insertSheet", json!({ "name": "Resumo", "before": 9 }));
    run("sheet.activate", json!({ "sheet": 1 }));
    set("A1", "Maior gasto");
    set(
        "B1",
        &format!("=INDEX(Orçamento!A4:A{last},MATCH(MAX(Orçamento!C4:C{last}),Orçamento!C4:C{last},0))"),
    );
    set("A2", "Acima do previsto");
    set("B2", &format!("=COUNTIF(Orçamento!D4:D{last},\"<0\")"));
    set("A3", "Sobra do mês");
    set("B3", &format!("=Orçamento!D{total}"));
    run("home.numberFormat", json!({ "range": "B3", "code": "\"R$\" #,##0.00" }));
    run("home.columnWidth", json!({ "cols": "A:B", "width": 140 }));
    run("sheet.activate", json!({ "sheet": 0 }));
    let _ = sheet.select("B4");
    sheet.with_session(|s| {
        if let Some(doc) = s.active_mut() {
            doc.undo.clear();
        }
    });
    sheet
}

/// The app: the sample budget, in a spreadsheet filling the window.
pub fn app() -> impl View {
    Spreadsheet::new(&sample())
}

#[cfg(target_arch = "wasm32")]
mod web_entry {
    use std::cell::RefCell;

    use wasm_bindgen::prelude::*;

    thread_local! {
        static APP: RefCell<Option<ravenrs::web::WebApp>> = const { RefCell::new(None) };
    }

    /// Runs automatically when the module is instantiated.
    #[wasm_bindgen(start)]
    pub fn start() {
        console_error_panic_hook::set_once();
        wasm_bindgen_futures::spawn_local(async {
            match ravenrs::web::start(super::app(), ravenrs::web::WebOptions::new("raven-canvas")).await {
                Ok(app) => APP.with(|a| *a.borrow_mut() = Some(app)),
                Err(e) => web_sys::console::error_1(&e.to_string().into()),
            }
        });
    }

    /// Unmounts the app and releases its listeners and GPU resources.
    #[wasm_bindgen]
    pub fn stop() {
        APP.with(|a| a.borrow_mut().take());
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn the_sample_adds_up() {
        let sheet = sample();
        assert_eq!(sheet.sheet_names(), ["Orçamento", "Resumo"]);
        assert_eq!(sheet.cell_text("B12"), "R$ 7,370.00");
        assert_eq!(sheet.cell_text("E5"), "112%");
        assert!(
            sheet.cell_text("D5").starts_with('-'),
            "overspent shows negative: {}",
            sheet.cell_text("D5")
        );
        sheet.execute("sheet.activate", json!({ "sheet": 1 })).unwrap();
        assert_eq!(sheet.cell_text("B1"), "Moradia");
        assert_eq!(sheet.cell_text("B2"), "3");
        assert!(!sheet.can_undo());
    }
}
