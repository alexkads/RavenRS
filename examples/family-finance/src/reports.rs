//! The app's reports: definitions from `ravenrs-report`, data from SQLite.
//! A definition is plain data, so the same one renders on screen, to PDF and to Excel.

use ravenrs::prelude::Date;
use ravenrs_report::barcode::Symbology;
use ravenrs_report::chart::ChartKind;
use ravenrs_report::{
    Band, Chart, ChartSeries, Control, ControlKind, Document, Group, PageSettings, Report, ReportEngine,
    Style, Table, TableRow, TextAlign,
};
use serde_json::{Value as Json, json};

use crate::db::{Database, DbResult};
use crate::model::{BillStatus, date_label, month_label, month_short, parse_date};

// Reports are printed on white paper, so these are the Raven Light tokens, as in `ravenrs-report`'s samples.
const ACCENT: &str = "#3f57db";
const INK: &str = "#12151c";
const MUTED: &str = "#5a6375";
const LINE: &str = "#dadde5";
const STRIPE: &str = "#f4f5f8";
const DANGER: &str = "#c93434";
const SUCCESS: &str = "#16875a";

/// The reports the app offers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ReportKind {
    /// The month's transactions by type and category.
    Statement,
    /// Unpaid bills, with their payment slips' barcodes.
    Bills,
    /// Spending against each category's limit.
    Budget,
    /// Twelve months of income and expenses.
    Yearly,
}

impl ReportKind {
    /// Every report, in the order the app shows them.
    pub const ALL: [Self; 4] = [Self::Statement, Self::Bills, Self::Budget, Self::Yearly];

    /// The name shown to the user.
    pub fn label(self) -> &'static str {
        match self {
            Self::Statement => "Extrato do mês",
            Self::Bills => "Contas a pagar",
            Self::Budget => "Orçamento",
            Self::Yearly => "Resumo de 12 meses",
        }
    }
}

/// Reads the data and lays the report out.
pub fn render(
    engine: &mut ReportEngine,
    db: &Database,
    kind: ReportKind,
    month: Date,
    today: Date,
) -> DbResult<Document> {
    let (report, data) = match kind {
        ReportKind::Statement => (statement(month), statement_data(db, month)?),
        ReportKind::Bills => (bills(today), bills_data(db, today)?),
        ReportKind::Budget => (budget(month), budget_data(db, month)?),
        ReportKind::Yearly => (yearly(month), yearly_data(db, month)?),
    };
    Ok(engine.render(&report, &data))
}

fn base(title: String) -> Report {
    Report::new(title)
        .culture("pt-BR")
        .style("th", Style::new().font_size(8.0).bold().color("#ffffff").background(ACCENT).padding(4.0))
        .style("td", Style::new().font_size(9.0).color(INK).padding(4.0))
        .style("odd", Style::new().background("#ffffff"))
        .style("even", Style::new().background(STRIPE))
        .page_footer(page_footer())
}

fn page_footer() -> Band {
    let w = PageSettings::a4().content_width();
    Band::new(22.0)
        .control(Control::line().at(0.0, 0.0, w, 2.0).style(Style::new().color(LINE)))
        .control(
            Control::label("Raven Finanças")
                .at(0.0, 4.0, 200.0, 16.0)
                .style(Style::new().font_size(7.5).color(MUTED)),
        )
        .control(
            Control::field("FormatString('Página {0} de {1}', PageNumber(), PageCount())")
                .at(w - 200.0, 4.0, 200.0, 16.0)
                .style(Style::new().font_size(7.5).color(MUTED).align(TextAlign::Right)),
        )
}

fn heading(height: f32, title: &str, subtitle_expression: &str) -> Band {
    let w = PageSettings::a4().content_width();
    Band::new(height)
        .control(
            Control::label(title).at(0.0, 0.0, w, 30.0).style(Style::new().font_size(20.0).bold().color(INK)),
        )
        .control(
            Control::field(subtitle_expression)
                .at(0.0, 30.0, w, 18.0)
                .style(Style::new().font_size(10.0).color(MUTED)),
        )
}

fn chart(chart: Chart, x: f32, y: f32, width: f32, height: f32) -> Control {
    Control { kind: ControlKind::Chart(chart), ..Control::default() }
        .at(x, y, width, height)
        .style(Style::new().font_size(8.5).color(INK))
}

fn right() -> Style {
    Style::new().align(TextAlign::Right)
}

fn total_band(label: &str, expression: &str) -> Band {
    let w = PageSettings::a4().content_width();
    Band::new(24.0)
        .control(Control::label(label).at(0.0, 3.0, w - 130.0, 18.0).style(Style::new().bold().color(INK)))
        .control(
            Control::field(expression)
                .format("C2")
                .at(w - 130.0, 3.0, 130.0, 18.0)
                .style(Style::new().bold().color(INK).align(TextAlign::Right).border("top", 1.0, INK)),
        )
}

// Statement

fn statement_data(db: &Database, month: Date) -> DbResult<Json> {
    Ok(Json::Array(
        db.ledger(month, None)?
            .into_iter()
            .map(|row| {
                json!({
                    "Data": row.date,
                    "Descricao": row.description,
                    "Valor": row.value,
                    "Tipo": row.kind,
                    "Conta": row.account,
                    "Categoria": row.category,
                    "Membro": row.member,
                })
            })
            .collect(),
    ))
}

/// The month's transactions grouped by type and category, with a chart of where the money went.
pub fn statement(month: Date) -> Report {
    let w = PageSettings::a4().content_width();
    let cols = [64.0, 200.0, 110.0, 60.0, w - 434.0];
    let income = "Sum(Iif([Tipo] = 'Receita', [Valor], 0))";
    let expense = "Sum(Iif([Tipo] = 'Despesa', [Valor], 0))";
    let header = TableRow::new(20.0)
        .text(cols[0], "DATA")
        .text(cols[1], "DESCRIÇÃO")
        .text(cols[2], "CONTA")
        .text(cols[3], "QUEM")
        .text(cols[4], "VALOR")
        .cell_style(right())
        .style_name("th");
    let row = TableRow::new(16.0)
        .field(cols[0], "[Data]", Some("dd/MM/yyyy"))
        .field(cols[1], "[Descricao]", None)
        .field(cols[2], "[Conta]", None)
        .field(cols[3], "[Membro]", None)
        .field(cols[4], "[Valor]", Some("C2"))
        .cell_style(right())
        .style_name("td");
    let title = format!("Extrato de {}", month_label(month));
    base(title.clone())
        .sort_by("[Data]", false)
        .report_header(
            heading(
                252.0,
                &title,
                &format!(
                    "FormatString('{{0}} lançamentos · entrou {{1:C2}} · saiu {{2:C2}} · sobrou {{3:C2}}', Count(), {income}, {expense}, {income} - {expense})"
                ),
            )
            .control(chart(
                Chart {
                    kind: ChartKind::Doughnut,
                    title: "Para onde foi o dinheiro".to_owned(),
                    argument: Some("[Categoria]".to_owned()),
                    series: vec![ChartSeries::new("Despesas", "Iif([Tipo] = 'Despesa', [Valor], 0)")],
                    format: Some("C2".to_owned()),
                    ..Chart::default()
                },
                0.0,
                54.0,
                w,
                190.0,
            )),
        )
        .group(
            Group::new("[Tipo]")
                .header(
                    Band::new(30.0)
                        .control(Control::rect(3.0).at(0.0, 10.0, 4.0, 16.0).style(Style::new().background(ACCENT)))
                        .control(Control::bookmark("[Tipo]", 0).at(0.0, 8.0, 80.0, 12.0))
                        .control(
                            Control::field("[Tipo] + 's'")
                                .at(10.0, 8.0, 300.0, 20.0)
                                .style(Style::new().font_size(13.0).bold().color(INK)),
                        )
                        .keep_with_next(),
                )
                .footer(total_band("Total", "Sum([Valor])")),
        )
        .group(
            Group::new("[Categoria]")
                .header(
                    Band::new(44.0)
                        .control(
                            Control::field("[Categoria]")
                                .at(0.0, 2.0, 300.0, 18.0)
                                .style(Style::new().bold().color(MUTED)),
                        )
                        .control(Control::table(Table::new().row(header)).at(0.0, 22.0, w, 20.0))
                        .keep_with_next(),
                )
                .footer(
                    Band::new(22.0).control(
                        Control::field("FormatString('{0}: {1:C2}', [Categoria], Sum([Valor]))")
                            .at(0.0, 2.0, w, 18.0)
                            .style(Style::new().font_size(8.5).color(MUTED).align(TextAlign::Right)),
                    ),
                )
                .repeat_header(),
        )
        .detail(
            Band::new(16.0)
                .control(Control::table(Table::new().row(row)).at(0.0, 0.0, w, 16.0))
                .zebra("odd", "even"),
        )
}

// Bills

fn bills_data(db: &Database, today: Date) -> DbResult<Json> {
    Ok(Json::Array(
        db.open_bills()?
            .into_iter()
            .map(|bill| {
                let status = bill.status(today);
                json!({
                    "Vencimento": bill.due.to_string(),
                    "Descricao": bill.title(),
                    "Categoria": bill.category,
                    "Membro": bill.member,
                    "Valor": bill.amount_cents as f64 / 100.0,
                    "Situacao": status.label(),
                    "Ordem": match status {
                        BillStatus::Overdue => 0,
                        BillStatus::DueToday => 1,
                        _ => 2,
                    },
                    "Codigo": bill.barcode,
                })
            })
            .collect(),
    ))
}

/// Unpaid bills, overdue first. A bill with a payment slip prints its barcode, ready to scan.
pub fn bills(today: Date) -> Report {
    let w = PageSettings::a4().content_width();
    let cols = [64.0, 230.0, 100.0, w - 394.0];
    let row = TableRow::new(16.0)
        .field(cols[0], "[Vencimento]", Some("dd/MM/yyyy"))
        .field(cols[1], "[Descricao]", None)
        .field(cols[2], "[Categoria]", None)
        .field(cols[3], "[Valor]", Some("C2"))
        .cell_style(right())
        .style_name("td");
    let header = TableRow::new(20.0)
        .text(cols[0], "VENCE")
        .text(cols[1], "CONTA")
        .text(cols[2], "CATEGORIA")
        .text(cols[3], "VALOR")
        .cell_style(right())
        .style_name("th");
    let title = format!("Contas a pagar em {}", date_label(today));
    base(title.clone())
        .sort_by("[Vencimento]", false)
        .report_header(heading(
            58.0,
            &title,
            "FormatString('{0} contas em aberto · {1:C2}', Count(), Sum([Valor]))",
        ))
        .group(
            Group::new("[Ordem]")
                .header(
                    Band::new(50.0)
                        .control(
                            Control::rect(3.0)
                                .at(0.0, 10.0, 4.0, 16.0)
                                .style(Style::new().background(DANGER)),
                        )
                        .control(
                            Control::field(
                                "Iif([Ordem] = 0, 'Vencidas', Iif([Ordem] = 1, 'Vencem hoje', 'A vencer'))",
                            )
                            .at(10.0, 8.0, 300.0, 20.0)
                            .style(Style::new().font_size(13.0).bold().color(INK)),
                        )
                        .control(Control::table(Table::new().row(header)).at(0.0, 30.0, w, 20.0))
                        .keep_with_next(),
                )
                .footer(total_band("Total", "Sum([Valor])"))
                .repeat_header(),
        )
        .detail(
            Band::new(58.0).control(Control::table(Table::new().row(row)).at(0.0, 0.0, w, 16.0)).control(
                Control::barcode(Symbology::Itf, "[Codigo]")
                    .at(cols[0], 19.0, 300.0, 34.0)
                    .visible("Len([Codigo]) > 0"),
            ),
        )
}

// Budget

fn budget_data(db: &Database, month: Date) -> DbResult<Json> {
    Ok(Json::Array(
        db.budget_lines(month)?
            .into_iter()
            .map(|line| {
                json!({
                    "Categoria": line.category,
                    "Limite": line.budget_cents as f64 / 100.0,
                    "Gasto": line.spent_cents as f64 / 100.0,
                })
            })
            .collect(),
    ))
}

/// Spending against each category's limit, with the ones over it in red.
pub fn budget(month: Date) -> Report {
    let w = PageSettings::a4().content_width();
    let cols = [190.0, 100.0, 100.0, w - 390.0];
    let header = TableRow::new(20.0)
        .text(cols[0], "CATEGORIA")
        .text(cols[1], "LIMITE")
        .cell_style(right())
        .text(cols[2], "GASTO")
        .cell_style(right())
        .text(cols[3], "SOBRA")
        .cell_style(right())
        .style_name("th");
    let row = TableRow::new(18.0)
        .field(cols[0], "[Categoria]", None)
        .field(cols[1], "[Limite]", Some("C2"))
        .cell_style(right())
        .field(cols[2], "[Gasto]", Some("C2"))
        .cell_style(right())
        .field(cols[3], "[Limite] - [Gasto]", Some("C2"))
        .cell_style(right())
        .style_name("td");
    let title = format!("Orçamento de {}", month_label(month));
    base(title.clone())
        .report_header(
            heading(
                316.0,
                &title,
                "FormatString('Limite {0:C2} · gasto {1:C2}', Sum([Limite]), Sum([Gasto]))",
            )
            .control(chart(
                Chart {
                    kind: ChartKind::Bar,
                    title: "Limite e gasto por categoria".to_owned(),
                    argument: Some("[Categoria]".to_owned()),
                    series: vec![
                        ChartSeries::new("Limite", "[Limite]"),
                        ChartSeries::new("Gasto", "[Gasto]"),
                    ],
                    format: Some("N0".to_owned()),
                    ..Chart::default()
                },
                0.0,
                54.0,
                w,
                230.0,
            ))
            .control(Control::table(Table::new().row(header)).at(0.0, 296.0, w, 20.0)),
        )
        .detail(
            Band::new(18.0)
                .control(
                    Control::table(Table::new().row(row))
                        .at(0.0, 0.0, w, 18.0)
                        .rule("[Limite] > 0 And [Gasto] > [Limite]", Style::new().color(DANGER).bold()),
                )
                .zebra("odd", "even"),
        )
        .report_footer(total_band("Total gasto", "Sum([Gasto])"))
}

// Twelve months

fn yearly_data(db: &Database, month: Date) -> DbResult<Json> {
    Ok(Json::Array(
        db.monthly_totals(month, 12)?
            .into_iter()
            .enumerate()
            .map(|(i, totals)| {
                let label = parse_date(&format!("{}-01", totals.month)).map_or_else(String::new, month_short);
                json!({
                    "Ordem": i,
                    "Mes": label,
                    "Receitas": totals.income_cents as f64 / 100.0,
                    "Despesas": totals.expense_cents as f64 / 100.0,
                })
            })
            .collect(),
    ))
}

/// Twelve months of income and expenses, and what was left of each.
pub fn yearly(month: Date) -> Report {
    let w = PageSettings::a4().content_width();
    let cols = [130.0, 120.0, 120.0, w - 370.0];
    let header = TableRow::new(20.0)
        .text(cols[0], "MÊS")
        .text(cols[1], "RECEITAS")
        .cell_style(right())
        .text(cols[2], "DESPESAS")
        .cell_style(right())
        .text(cols[3], "SOBROU")
        .cell_style(right())
        .style_name("th");
    let row = TableRow::new(18.0)
        .field(cols[0], "[Mes]", None)
        .field(cols[1], "[Receitas]", Some("C2"))
        .cell_style(right())
        .field(cols[2], "[Despesas]", Some("C2"))
        .cell_style(right())
        .field(cols[3], "[Receitas] - [Despesas]", Some("C2"))
        .cell_style(right())
        .style_name("td");
    let title = format!("Resumo de 12 meses até {}", month_label(month));
    base(title.clone())
        .report_header(
            heading(
                316.0,
                &title,
                "FormatString('Entrou {0:C2} · saiu {1:C2} · sobrou {2:C2}', Sum([Receitas]), Sum([Despesas]), Sum([Receitas]) - Sum([Despesas]))",
            )
            .control(chart(
                Chart {
                    kind: ChartKind::Line,
                    title: "Receitas e despesas".to_owned(),
                    argument: Some("[Mes]".to_owned()),
                    series: vec![
                        ChartSeries { color: Some(SUCCESS.to_owned()), ..ChartSeries::new("Receitas", "[Receitas]") },
                        ChartSeries { color: Some(DANGER.to_owned()), ..ChartSeries::new("Despesas", "[Despesas]") },
                    ],
                    format: Some("N0".to_owned()),
                    ..Chart::default()
                },
                0.0,
                54.0,
                w,
                230.0,
            ))
            .control(Control::table(Table::new().row(header)).at(0.0, 296.0, w, 20.0)),
        )
        .detail(
            Band::new(18.0)
                .control(
                    Control::table(Table::new().row(row))
                        .at(0.0, 0.0, w, 18.0)
                        .rule("[Receitas] < [Despesas]", Style::new().color(DANGER)),
                )
                .zebra("odd", "even"),
        )
}
