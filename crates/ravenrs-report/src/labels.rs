//! Labels: the sizes they come in, and reports laid out for them.
//!
//! A label is a report like any other: each row is a label, in the detail band. A roll has one
//! label per page; a sheet flows them in columns. So the designer, the preview and every export
//! work on them, and what is specific to label printers stays out of the engine.

use crate::model::{Band, Columns, Control, PageSettings, Report, Style, TextAlign, mm};

/// A size labels come in.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LabelStock {
    /// What it is called.
    pub name: &'static str,
    /// The paper: one label for a roll, a whole sheet otherwise.
    pub page: PageSettings,
    /// How they sit on the paper. A roll has none.
    pub columns: Option<Columns>,
    /// The width of each label, in points.
    pub width: f32,
    /// The height of each label, in points.
    pub height: f32,
    /// A roll with no gaps, where a page is as long as what it prints.
    pub continuous: bool,
}

impl LabelStock {
    /// A roll of receipt paper, with this much of its width within reach of the printer. Rows take
    /// the height they need, so its detail band starts small.
    pub fn receipt(name: &'static str, printable_width: f32) -> Self {
        Self {
            name,
            page: receipt_page(printable_width, 0.0),
            columns: None,
            width: mm(printable_width),
            height: 14.0,
            continuous: true,
        }
    }

    /// One label after another on a roll, of this size in millimeters.
    pub fn roll(name: &'static str, width: f32, height: f32) -> Self {
        Self {
            name,
            page: PageSettings::label_mm(width, height),
            columns: None,
            width: mm(width),
            height: mm(height),
            continuous: false,
        }
    }

    /// A sheet of A4 with `across` labels on each line, laid out as `sheet` says.
    pub fn a4_sheet(name: &'static str, across: u32, sheet: SheetLayout) -> Self {
        Self {
            name,
            page: PageSettings {
                margin_top: mm(sheet.top),
                margin_bottom: mm(sheet.top),
                margin_left: mm(sheet.side),
                margin_right: mm(sheet.side),
                ..PageSettings::a4()
            },
            columns: Some(Columns::new(across).spaced(mm(sheet.gap_across), mm(sheet.gap_down))),
            width: mm(sheet.width),
            height: mm(sheet.height),
            continuous: false,
        }
    }

    /// A report with no content yet, its paper and detail band set for this label.
    pub fn report(&self, name: impl Into<String>) -> Report {
        let mut report = Report::new(name).page(self.page);
        report.columns = self.columns;
        report.continuous = self.continuous;
        report.detail = Band::new(self.height);
        report
    }
}

/// A roll of receipt paper, in millimeters of what the printer can reach: 72 on the 80 mm roll, 48 on
/// the 58 mm one. A report on it is as long as what it prints, up to about two meters.
pub fn receipt_page(printable_width: f32, margin: f32) -> PageSettings {
    PageSettings {
        width: mm(printable_width),
        height: mm(2000.0),
        margin_top: mm(margin),
        margin_bottom: mm(margin),
        margin_left: mm(margin),
        margin_right: mm(margin),
    }
}

/// How labels sit on a sheet, in millimeters.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SheetLayout {
    /// Each label's width.
    pub width: f32,
    /// Each label's height.
    pub height: f32,
    /// The margin above and below.
    pub top: f32,
    /// The margin on the sides.
    pub side: f32,
    /// Between labels across.
    pub gap_across: f32,
    /// Between labels down.
    pub gap_down: f32,
}

/// The common sizes: rolls of labels, sheets, then rolls of receipt paper.
pub fn common_stocks() -> Vec<LabelStock> {
    vec![
        LabelStock::roll("Rolo 40 x 25 mm", 40.0, 25.0),
        LabelStock::roll("Rolo 60 x 40 mm", 60.0, 40.0),
        LabelStock::roll("Rolo 100 x 50 mm", 100.0, 50.0),
        LabelStock::a4_sheet(
            "Folha A4, 3 x 7, 63,5 x 38,1 mm",
            3,
            SheetLayout { width: 63.5, height: 38.1, top: 15.15, side: 7.2, gap_across: 2.55, gap_down: 0.0 },
        ),
        LabelStock::a4_sheet(
            "Folha A4, 2 x 7, 99,1 x 38,1 mm",
            2,
            SheetLayout { width: 99.1, height: 38.1, top: 15.15, side: 4.65, gap_across: 2.5, gap_down: 0.0 },
        ),
        LabelStock::receipt("Bobina 80 mm", 72.0),
        LabelStock::receipt("Bobina 58 mm", 48.0),
    ]
}

/// The expressions a shelf label shows, field by field.
#[derive(Clone, Debug, PartialEq)]
pub struct ShelfLabelFields {
    /// What the product is.
    pub description: String,
    /// Its price.
    pub price: String,
    /// The format of the price.
    pub price_format: String,
    /// Its GTIN or other code.
    pub barcode: String,
    /// The format of the code.
    pub symbology: crate::barcode::Symbology,
}

/// A shelf label for this stock: description on top, the price large, the code below. It is a
/// starting point, to change in the designer.
pub fn shelf_label(stock: &LabelStock, name: impl Into<String>, fields: &ShelfLabelFields) -> Report {
    let (width, height) = (stock.width, stock.height);
    let inset = (width * 0.04).min(mm(2.0));
    let inner = width - 2.0 * inset;
    let description = height * 0.26;
    let price = height * 0.30;
    let code = height - description - price - 2.0 * inset;
    let mut report = stock.report(name);
    report.detail = Band::new(height)
        .control(
            Control::field(fields.description.clone())
                .at(inset, inset, inner, description)
                .fixed_height()
                .style(Style::new().font_size((description * 0.42).clamp(6.0, 14.0)).bold()),
        )
        .control(
            Control::field(fields.price.clone())
                .at(inset, inset + description, inner, price)
                .fixed_height()
                .format(&fields.price_format)
                .style(
                    Style::new().font_size((price * 0.72).clamp(9.0, 40.0)).bold().align(TextAlign::Right),
                ),
        )
        .control(
            Control::barcode(fields.symbology, fields.barcode.clone())
                .at(inset, inset + description + price, inner, code)
                .style(Style::new().font_size((code * 0.2).clamp(5.0, 9.0))),
        );
    report
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_stock_fits_its_paper() {
        for stock in common_stocks() {
            let across = stock.columns.map_or(1, |columns| columns.count) as f32;
            let spacing = stock.columns.map_or(0.0, |columns| columns.spacing);
            let used = across * stock.width + (across - 1.0) * spacing;
            assert!(used <= stock.page.content_width() + 0.5, "{}: {used}", stock.name);
            assert!(stock.height <= stock.page.height, "{}", stock.name);
        }
    }

    #[test]
    fn a_label_report_keeps_its_stock_through_json() {
        let stock = common_stocks()[3];
        let report = stock.report("Etiqueta");
        let json = report.to_json();
        let back = Report::from_json(&json).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(back.columns, stock.columns);
        assert_eq!(back.page, stock.page);
    }

    #[test]
    fn a_long_description_does_not_push_the_code_off_the_label() {
        use crate::{Item, ReportEngine};
        let stock = common_stocks()[1];
        let report = shelf_label(
            &stock,
            "Etiqueta",
            &ShelfLabelFields {
                description: "[Name]".into(),
                price: "[Price]".into(),
                price_format: "C2".into(),
                barcode: "[Code]".into(),
                symbology: crate::barcode::Symbology::Ean13,
            },
        );
        let long = "Sabão em pó concentrado para roupas brancas e coloridas, pacote econômico de 5 kg";
        let data = serde_json::json!([{ "Name": long, "Price": 5.49, "Code": "7891000100103" }]);
        let document = ReportEngine::new().render(&report, &data);
        assert_eq!(document.pages.len(), 1);
        let bars = document.pages[0]
            .items
            .iter()
            .find_map(|item| match item {
                Item::BarcodeMark { rect, .. } => Some(*rect),
                _ => None,
            })
            .unwrap_or_else(|| panic!("no code: {:?}", document.errors));
        assert!(bars.max_y() <= stock.height + 0.5, "{bars:?} on a label of {}", stock.height);
    }
}
