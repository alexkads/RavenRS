//! The receipt: an ordinary stored report, on one row for each item with the rest repeated on each
//! row, so anyone can change it in the standard designer. All values are text, ready to print.

use ravenrs_app::{EntityInfo, FieldInfo, FieldKind, Object, Value};
use ravenrs_app_report::{ReportWords, render_stored_report_with};
use ravenrs_app_ui::Session;
use ravenrs_report::barcode::Symbology;
use ravenrs_report::labels::{LabelStock, common_stocks};
use ravenrs_report::{Band, Control, Document, PrinterAction, Report, Style, TextAlign};

/// The entity of the lines of a receipt, which storage has to know.
pub const CUPOM_LINHA: &str = "NfeCupomLinha";

/// The fields a layout cannot do without and still be the fiscal receipt.
pub const CAMPOS_OBRIGATORIOS: [&str; 4] = ["chave", "qr_code", "numero", "protocolo"];

/// The fields of a line, with what people call them in the designer.
const CAMPOS: [(&str, &str); 26] = [
    ("documento", "Documento"),
    ("ordem", "Item"),
    ("emitente", "Emitente"),
    ("cnpj", "CNPJ"),
    ("ie", "Inscrição estadual"),
    ("endereco", "Endereço"),
    ("titulo", "Título"),
    ("aviso", "Aviso do ambiente"),
    ("codigo", "Código"),
    ("descricao", "Descrição"),
    ("quantidade", "Quantidade"),
    ("unidade", "Unidade"),
    ("unitario", "Valor unitário"),
    ("valor", "Valor do item"),
    ("itens", "Quantidade de itens"),
    ("total", "Total"),
    ("pagamento", "Forma de pagamento"),
    ("recebido", "Valor recebido"),
    ("troco", "Troco"),
    ("chave", "Chave de acesso"),
    ("numero", "Número e série"),
    ("emitido", "Emissão"),
    ("protocolo", "Protocolo"),
    ("consumidor", "Consumidor"),
    ("qr_code", "QR Code"),
    ("observacao", "Observação"),
];

/// The entity of the lines of a receipt.
pub fn cupom_entity() -> EntityInfo {
    CAMPOS.iter().fold(
        EntityInfo::new(CUPOM_LINHA)
            .caption("Linhas do cupom")
            .field(FieldInfo::new("id", FieldKind::Guid).key()),
        |entity, (name, caption)| entity.field(FieldInfo::new(*name, FieldKind::Text).caption(*caption)),
    )
}

fn text(expression: &str, x: f32, y: f32, width: f32, height: f32, style: Style) -> Control {
    Control::field(expression).at(x, y, width, height).style(style)
}

/// The receipt as it comes, for this roll. Everything in it can change in the designer.
pub fn cupom_report(stock: &LabelStock, name: impl Into<String>) -> Report {
    let width = stock.width;
    let base = if stock.width < 180.0 { 7.0 } else { 8.0 };
    let plain = || Style::new().font_size(base);
    let center = || Style::new().font_size(base).align(TextAlign::Center);
    let strong = || Style::new().font_size(base + 1.0).bold().align(TextAlign::Center);
    let right = || Style::new().font_size(base).align(TextAlign::Right);
    let line = base * 1.5;
    let code = width.min(120.0);
    let mut report = stock.report(name);
    report.report_header = Some(
        Band::new(line * 6.0)
            .control(text("[emitente]", 0.0, 0.0, width, line, strong()))
            .control(text("'CNPJ ' + [cnpj] + '  IE ' + [ie]", 0.0, line, width, line, center()))
            .control(text("[endereco]", 0.0, line * 2.0, width, line, center()))
            .control(text("[titulo]", 0.0, line * 3.0, width, line * 2.0, strong()))
            .control(text("[aviso]", 0.0, line * 5.0, width, line, strong())),
    );
    report.detail = Band::new(line * 2.0)
        .control(text("[ordem] + ' ' + [codigo] + ' ' + [descricao]", 0.0, 0.0, width, line, plain()))
        .control(text(
            "[quantidade] + ' ' + [unidade] + ' x ' + [unitario]",
            0.0,
            line,
            width * 0.6,
            line,
            plain(),
        ))
        .control(text("[valor]", width * 0.6, line, width * 0.4, line, right()));
    let mut y = 0.0;
    let mut footer = Band::new(0.0);
    let rows = [
        ("'Qtde. total de itens: ' + [itens]", line, right()),
        ("'TOTAL R$ ' + [total]", line * 1.3, strong()),
        ("[pagamento] + ' R$ ' + [recebido]", line, right()),
        ("'Troco R$ ' + [troco]", line, right()),
        ("'Consulte pela chave de acesso em sua SEFAZ'", line, center()),
        ("[chave]", line * 2.0, center()),
        ("[consumidor]", line, center()),
        ("[numero] + '  ' + [emitido]", line, center()),
        ("'Protocolo: ' + [protocolo]", line, center()),
    ];
    for (expression, height, style) in rows {
        footer = footer.control(text(expression, 0.0, y, width, height, style));
        y += height;
    }
    footer = footer.control(Control::barcode(Symbology::Qr, "[qr_code]").at((width - code) / 2.0, y, code, code));
    y += code + 2.0;
    footer = footer.control(text("[observacao]", 0.0, y, width, line * 2.0, center()));
    y += line * 2.0;
    footer = footer.control(Control::printer_command(PrinterAction::Cut).at(0.0, y, width, 12.0));
    footer.height = y;
    report.report_footer = Some(footer);
    report
}

/// The receipts that come with the module, with what they are called.
pub fn modelos_de_cupom() -> Vec<(String, Report)> {
    common_stocks()
        .into_iter()
        .filter(|stock| stock.continuous)
        .map(|stock| {
            let name = format!("Cupom {}", stock.name.to_lowercase());
            let report = cupom_report(&stock, name.clone());
            (name, report)
        })
        .collect()
}

/// What makes a receipt fiscal and this layout left out.
pub fn campos_faltando(report: &Report) -> Vec<&'static str> {
    let written = report.to_json();
    CAMPOS_OBRIGATORIOS.into_iter().filter(|field| !written.contains(&format!("[{field}]"))).collect()
}

/// Puts the notice of the environment back, for a layout that took it out.
fn com_aviso(report: &mut Report) {
    if report.to_json().contains("[aviso]") {
        return;
    }
    let width = report.page.content_width();
    let notice = Control::field("[aviso]")
        .at(0.0, 0.0, width, 12.0)
        .style(Style::new().font_size(8.0).bold().align(TextAlign::Center));
    let header = report.report_header.get_or_insert_with(|| Band::new(0.0));
    for control in &mut header.controls {
        control.y += 12.0;
    }
    header.height += 12.0;
    header.controls.insert(0, notice);
}

/// A stored receipt over the lines of a document, or what keeps it from being printed.
pub fn renderizar_cupom(
    session: &Session,
    modelo: &Object,
    linhas: &[Object],
    words: &ReportWords,
) -> Result<Document, String> {
    let rows: Vec<(Object, u32)> = linhas.iter().map(|line| (line.clone(), 1)).collect();
    render_stored_report_with(session, modelo, &rows, words, |report| {
        let missing = campos_faltando(report);
        if !missing.is_empty() {
            return Err(format!(
                "Este layout não traz o que o cupom fiscal precisa: {}. Abra-o no editor e inclua.",
                missing.join(", ")
            ));
        }
        com_aviso(report);
        Ok(())
    })
}

/// The lines of the receipt of a document, in order.
pub fn linhas_do_documento(session: &Session, documento: &str) -> Vec<Object> {
    let space = session.frame().space();
    let found = space.borrow().query(CUPOM_LINHA, &ravenrs_app::Query::all()).unwrap_or_default();
    let mut lines: Vec<Object> =
        found.into_iter().filter(|line| line.get("documento") == Value::Text(documento.to_owned())).collect();
    lines.sort_by_key(|line| match line.get("ordem") {
        Value::Text(order) => order.parse::<u32>().unwrap_or(0),
        _ => 0,
    });
    lines
}

#[cfg(test)]
mod tests {
    use ravenrs_report::ReportEngine;
    use ravenrs_report::labels::common_stocks;

    use super::*;

    fn linhas(aviso: &str) -> serde_json::Value {
        let mut rows = Vec::new();
        for n in 1..=3 {
            let mut row = serde_json::Map::new();
            for (name, _) in CAMPOS {
                row.insert(name.to_owned(), serde_json::json!(format!("{name}{n}")));
            }
            row.insert("qr_code".to_owned(), serde_json::json!("https://exemplo/?p=1|3|2"));
            row.insert("aviso".to_owned(), serde_json::json!(aviso));
            rows.push(serde_json::Value::Object(row));
        }
        serde_json::Value::Array(rows)
    }

    #[test]
    fn the_models_have_what_a_receipt_needs_and_fit_their_roll() {
        let models = modelos_de_cupom();
        assert_eq!(models.len(), 2);
        for (name, report) in models {
            assert!(campos_faltando(&report).is_empty(), "{name}");
            let document = ReportEngine::new().render(&report, &linhas("SEM VALOR FISCAL"));
            assert!(document.errors.is_empty(), "{name}: {:?}", document.errors);
            assert_eq!(document.pages.len(), 1, "{name}");
            let page = &document.pages[0];
            assert!(page.height < 2000.0, "{name}: it ends where it prints");
            assert!(
                page.items.iter().any(|item| matches!(item, ravenrs_report::Item::BarcodeMark { .. })),
                "{name}: QR Code"
            );
            assert!(
                page.items.iter().any(|item| matches!(item, ravenrs_report::Item::PrinterCommand { .. })),
                "{name}: cuts at the end"
            );
        }
    }

    #[test]
    fn a_layout_without_the_key_says_so_and_the_notice_comes_back() {
        let stock = common_stocks().into_iter().find(|stock| stock.continuous).expect("a roll");
        let mut report = stock.report("Solto");
        report.detail = Band::new(12.0).control(Control::field("[descricao]").at(0.0, 0.0, 100.0, 12.0));
        assert_eq!(campos_faltando(&report), CAMPOS_OBRIGATORIOS.to_vec());
        com_aviso(&mut report);
        assert!(report.to_json().contains("[aviso]"));
    }
}
