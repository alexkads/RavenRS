//! Ready-made reports with sample data: the designer's templates, the
//! benchmarks' workloads and living documentation of the report model.

use serde_json::{Value as Json, json};

use crate::model::{
    Band, Control, ControlKind, DataSource, DataSourceKind, DetailReport, Group, Label, LinkLabel,
    PageSettings, ParameterKind, Report, Style, Table, TableRow, TextAlign, VerticalAlign,
};

// Raven Light tokens: reports are printed on white paper.
const ACCENT: &str = "#3f57db";
const INK: &str = "#12151c";
const MUTED: &str = "#5a6375";
const LINE: &str = "#dadde5";
const STRIPE: &str = "#f4f5f8";

/// A small deterministic random generator, so samples are the same everywhere.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn pick<'a>(&mut self, items: &[&'a str]) -> &'a str {
        items[(self.next() % items.len() as u64) as usize]
    }

    fn range(&mut self, lo: u64, hi: u64) -> u64 {
        lo + self.next() % (hi - lo + 1)
    }
}

const FIRST: [&str; 16] = [
    "Ana", "Bruno", "Carla", "Diego", "Elisa", "Felipe", "Gabriela", "Henrique", "Isabela", "João",
    "Larissa", "Marcos", "Natália", "Otávio", "Paula", "Rafael",
];
const LAST: [&str; 12] = [
    "Silva", "Souza", "Oliveira", "Lima", "Pereira", "Costa", "Almeida", "Ribeiro", "Carvalho", "Gomes",
    "Martins", "Rocha",
];
const CITIES: [(&str, &str); 10] = [
    ("São Paulo", "Sudeste"),
    ("Rio de Janeiro", "Sudeste"),
    ("Belo Horizonte", "Sudeste"),
    ("Curitiba", "Sul"),
    ("Porto Alegre", "Sul"),
    ("Florianópolis", "Sul"),
    ("Recife", "Nordeste"),
    ("Salvador", "Nordeste"),
    ("Fortaleza", "Nordeste"),
    ("Manaus", "Norte"),
];
const PRODUCTS: [(&str, f64); 10] = [
    ("Notebook Pro 14", 7_499.0),
    ("Monitor 27\" 4K", 2_299.0),
    ("Teclado mecânico", 489.9),
    ("Mouse sem fio", 159.9),
    ("Headset", 349.0),
    ("Webcam Full HD", 279.0),
    ("Dock USB-C", 899.0),
    ("SSD 1 TB", 529.0),
    ("Cadeira ergonômica", 1_890.0),
    ("Mesa elevatória", 2_790.0),
];

/// `count` sales rows: date, region, city, customer, product, quantity, price.
pub fn sales_data(count: usize) -> Json {
    let mut rng = Rng(0x5eed_1234_abcd_9876);
    let rows: Vec<Json> = (0..count)
        .map(|i| {
            let (city, region) = CITIES[(rng.next() % CITIES.len() as u64) as usize];
            let (product, price) = PRODUCTS[(rng.next() % PRODUCTS.len() as u64) as usize];
            let month = rng.range(1, 12);
            let day = rng.range(1, 28);
            json!({
                "Id": 10_000 + i,
                "Date": format!("2026-{month:02}-{day:02}"),
                "Region": region,
                "City": city,
                "Customer": format!("{} {}", rng.pick(&FIRST), rng.pick(&LAST)),
                "Product": product,
                "Quantity": rng.range(1, 12),
                "UnitPrice": price,
                "Paid": !rng.next().is_multiple_of(5),
            })
        })
        .collect();
    Json::Array(rows)
}

/// An invoice with its items, as a single master row.
pub fn invoice_data() -> Json {
    json!({
        "Number": "NF-2026-00417",
        "IssueDate": "2026-11-01",
        "DueDate": "2026-11-16",
        "Seller": { "Name": "Raven Tecnologia Ltda.", "Document": "12.345.678/0001-90", "City": "Recife - PE", "Email": "financeiro@raven.dev" },
        "Customer": { "Name": "Kestrel Saúde S.A.", "Document": "98.765.432/0001-10", "Address": "Av. Boa Viagem, 1200 - Recife - PE", "Email": "compras@kestrel.com.br" },
        "Discount": 350.0,
        "Notes": "Pagamento via PIX ou boleto. Após o vencimento, multa de 2% e juros de 1% ao mês.",
        "Items": [
            { "Sku": "NB-PRO14", "Description": "Notebook Pro 14, 32 GB, 1 TB", "Quantity": 4, "UnitPrice": 7499.0 },
            { "Sku": "MN-27K", "Description": "Monitor 27\" 4K com ajuste de altura", "Quantity": 4, "UnitPrice": 2299.0 },
            { "Sku": "DK-USBC", "Description": "Dock USB-C com 2 saídas de vídeo e carregamento de 100 W", "Quantity": 4, "UnitPrice": 899.0 },
            { "Sku": "KB-MEC", "Description": "Teclado mecânico ABNT2", "Quantity": 4, "UnitPrice": 489.9 },
            { "Sku": "SV-INST", "Description": "Instalação e configuração no local", "Quantity": 1, "UnitPrice": 1200.0 }
        ]
    })
}

fn title() -> Style {
    Style::new().font_size(20.0).bold().color(INK)
}

fn caption() -> Style {
    Style::new().font_size(7.5).bold().color(MUTED)
}

fn header_cell() -> Style {
    Style::new().font_size(8.0).bold().color("#ffffff").background(ACCENT).padding(4.0)
}

fn cell() -> Style {
    Style::new().font_size(9.0).color(INK).padding(4.0)
}

fn money(style: Style) -> Style {
    style.align(TextAlign::Right)
}

fn page_footer(culture_pt: bool) -> Band {
    let page = if culture_pt { "Página {0} de {1}" } else { "Page {0} of {1}" };
    Band::new(22.0)
        .control(Control::line().at(0.0, 0.0, 510.0, 2.0).style(Style::new().color(LINE)))
        .control(
            Control::field("FormatString('{0:dd/MM/yyyy HH:mm}', Now())")
                .at(0.0, 4.0, 200.0, 16.0)
                .style(Style::new().font_size(7.5).color(MUTED)),
        )
        .control(
            Control::field(format!("FormatString('{page}', PageNumber(), PageCount())"))
                .at(310.0, 4.0, 200.0, 16.0)
                .style(Style::new().font_size(7.5).color(MUTED).align(TextAlign::Right)),
        )
}

/// Sales grouped by region and city, with subtotals, a grand total, zebra
/// rows and a highlighting rule. `rows` sample rows are embedded.
pub fn sales_by_region(rows: usize) -> Report {
    let w = PageSettings::a4().content_width();
    let cols = [70.0, 150.0, 140.0, 40.0, 50.0, w - 450.0];
    let header = TableRow::new(20.0)
        .text(cols[0], "DATA")
        .text(cols[1], "CLIENTE")
        .text(cols[2], "PRODUTO")
        .text(cols[3], "QTD")
        .cell_style(Style::new().align(TextAlign::Right))
        .text(cols[4], "PAGO")
        .cell_style(Style::new().align(TextAlign::Center))
        .text(cols[5], "TOTAL")
        .cell_style(Style::new().align(TextAlign::Right))
        .style_name("th");
    let detail = TableRow::new(16.0)
        .field(cols[0], "[Date]", Some("dd/MM/yyyy"))
        .field(cols[1], "[Customer]", None)
        .field(cols[2], "[Product]", None)
        .field(cols[3], "[Quantity]", None)
        .cell_style(Style::new().align(TextAlign::Right))
        .field(cols[4], "Iif([Paid], 'Sim', 'Não')", None)
        .cell_style(Style::new().align(TextAlign::Center))
        .field(cols[5], "[Total]", Some("C2"))
        .cell_style(Style::new().align(TextAlign::Right))
        .style_name("td");
    Report::new("Vendas por região")
        .culture("pt-BR")
        .data_source(DataSource {
            name: "Vendas".into(),
            kind: DataSourceKind::Json { data: sales_data(rows) },
        })
        .calculated("Total", "[Quantity] * [UnitPrice]")
        .parameter("MinTotal", ParameterKind::Number, json!(0))
        .parameter_label("Valor mínimo")
        .filter("[Total] >= ?MinTotal")
        .sort_by("[Date]", false)
        .style("th", header_cell())
        .style("td", cell())
        .style("odd", Style::new().background("#ffffff"))
        .style("even", Style::new().background(STRIPE))
        .report_header(
            Band::new(64.0)
                .control(Control::label("Vendas por região").at(0.0, 0.0, 360.0, 30.0).style(title()))
                .control(
                    Control::field("FormatString('{0:N0} pedidos · {1:C2}', Count(), Sum([Total]))")
                        .at(0.0, 30.0, 360.0, 18.0)
                        .style(Style::new().font_size(10.0).color(MUTED)),
                )
                .control(
                    Control::label("RAVEN TECNOLOGIA")
                        .at(w - 160.0, 4.0, 160.0, 14.0)
                        .style(caption().align(TextAlign::Right).color(ACCENT)),
                ),
        )
        .page_footer(page_footer(true))
        .group(
            Group::new("[Region]")
                .header(
                    Band::new(30.0)
                        .control(
                            Control::rect(3.0)
                                .at(0.0, 10.0, 4.0, 16.0)
                                .style(Style::new().background(ACCENT)),
                        )
                        .control(Control::bookmark("[Region]", 0).at(0.0, 8.0, 80.0, 12.0))
                        .control(
                            Control::field("'Região ' + [Region]")
                                .at(10.0, 8.0, 300.0, 20.0)
                                .style(Style::new().font_size(13.0).bold().color(INK)),
                        )
                        .keep_with_next(),
                )
                .footer(
                    Band::new(26.0)
                        .control(
                            Control::field("'Total ' + [Region]")
                                .at(0.0, 4.0, 300.0, 18.0)
                                .style(Style::new().bold().color(INK)),
                        )
                        .control(
                            Control::field("Sum([Total])")
                                .format("C2")
                                .at(w - 160.0, 4.0, 160.0, 18.0)
                                .style(
                                    Style::new()
                                        .bold()
                                        .color(INK)
                                        .align(TextAlign::Right)
                                        .border("top", 1.0, INK),
                                ),
                        ),
                )
                .repeat_header(),
        )
        .group(
            Group::new("[City]")
                .header(
                    Band::new(44.0)
                        .control(
                            Control::field("[City]")
                                .at(0.0, 2.0, 300.0, 18.0)
                                .style(Style::new().bold().color(MUTED)),
                        )
                        .control(Control::table(Table::new().row(header)).at(0.0, 22.0, w, 20.0))
                        .keep_with_next(),
                )
                .footer(
                    Band::new(24.0).control(
                        Control::field(
                            "FormatString('{0} pedidos em {1}: {2:C2}', Count(), [City], Sum([Total]))",
                        )
                        .at(0.0, 2.0, w, 18.0)
                        .style(Style::new().font_size(8.5).color(MUTED).align(TextAlign::Right)),
                    ),
                )
                .repeat_header(),
        )
        .detail(
            Band::new(16.0)
                .control(
                    Control::table(Table::new().row(detail))
                        .at(0.0, 0.0, w, 16.0)
                        .rule("[Total] >= 20000", Style::new().color("#c93434").bold()),
                )
                .zebra("odd", "even"),
        )
        .report_footer(
            Band::new(40.0).control(
                Control::field("FormatString('Total geral: {0:C2}', Sum([Total]))")
                    .at(w - 260.0, 12.0, 260.0, 22.0)
                    .style(
                        Style::new()
                            .font_size(12.0)
                            .bold()
                            .color("#ffffff")
                            .background(INK)
                            .padding(5.0)
                            .align(TextAlign::Right),
                    ),
            ),
        )
}

/// An invoice: seller and customer blocks, an item table from a nested
/// collection (master-detail), totals, notes.
pub fn invoice() -> Report {
    let w = PageSettings::a4().content_width();
    let cols = [70.0, w - 300.0, 50.0, 90.0, 90.0];
    let party = |label: &str, field: &str, x: f32| {
        Control::table(
            Table::new()
                .row(TableRow::new(14.0).text(190.0, label).style_name("caption"))
                .row(
                    TableRow::new(18.0)
                        .field(190.0, &format!("[{field}.Name]"), None)
                        .cell_style(Style::new().bold().font_size(11.0).color(INK).padding(2.0)),
                )
                .row(
                    TableRow::new(14.0)
                        .field(190.0, &format!("'CNPJ ' + [{field}.Document]"), None)
                        .style_name("small"),
                )
                .row(
                    TableRow::new(14.0)
                        .field(190.0, &format!("IsNull([{field}.Address], [{field}.City])"), None)
                        .style_name("small"),
                )
                .row(TableRow::new(14.0).field(190.0, &format!("[{field}.Email]"), None).style_name("small")),
        )
        .at(x, 70.0, 190.0, 74.0)
    };
    let total_line = |label: &str, expr: &str, y: f32, strong: bool| {
        let style = if strong {
            Style::new().font_size(12.0).bold().color("#ffffff").background(ACCENT).padding(5.0)
        } else {
            Style::new().color(INK).padding(3.0)
        };
        [
            Control::label(label)
                .at(w - 260.0, y, 130.0, if strong { 26.0 } else { 18.0 })
                .style(style.clone()),
            Control::field(expr)
                .format("C2")
                .at(w - 130.0, y, 130.0, if strong { 26.0 } else { 18.0 })
                .style(money(style)),
        ]
    };
    let [s1, s2] = total_line("Subtotal", "[Items][].Sum([Quantity] * [UnitPrice])", 8.0, false);
    let [d1, d2] = total_line("Desconto", "-[Discount]", 26.0, false);
    let [t1, t2] =
        total_line("Total a pagar", "[Items][].Sum([Quantity] * [UnitPrice]) - [Discount]", 48.0, true);
    Report::new("Fatura")
        .culture("pt-BR")
        .data_source(DataSource {
            name: "Fatura".into(),
            kind: DataSourceKind::Json { data: invoice_data() },
        })
        .style("caption", caption())
        .style("small", Style::new().font_size(8.5).color(MUTED).padding(2.0))
        .style("th", header_cell())
        .style("td", cell().border("bottom", 0.5, LINE))
        .report_header(
            Band::new(160.0)
                .control(Control::rect(6.0).at(0.0, 0.0, 34.0, 34.0).style(Style::new().background(ACCENT)))
                .control(
                    Control::label("R").at(0.0, 0.0, 34.0, 34.0).style(
                        Style::new()
                            .font_size(18.0)
                            .bold()
                            .color("#ffffff")
                            .align(TextAlign::Center)
                            .valign(VerticalAlign::Middle),
                    ),
                )
                .control(
                    Control::field("[Seller.Name]")
                        .at(44.0, 0.0, 260.0, 18.0)
                        .style(Style::new().bold().font_size(11.0).color(INK)),
                )
                .control(
                    Control {
                        kind: ControlKind::Link(LinkLabel {
                            label: Label {
                                expression: Some("[Seller.Email]".to_owned()),
                                ..Label::default()
                            },
                            url_expression: Some("'mailto:' + [Seller.Email]".to_owned()),
                            ..LinkLabel::default()
                        }),
                        ..Control::default()
                    }
                    .at(44.0, 18.0, 260.0, 14.0)
                    .style(Style::new().font_size(8.5).color(MUTED)),
                )
                .control(
                    Control::label("FATURA")
                        .at(w - 200.0, 0.0, 200.0, 26.0)
                        .style(title().align(TextAlign::Right)),
                )
                .control(
                    Control::field("[Number]")
                        .at(w - 200.0, 26.0, 200.0, 14.0)
                        .style(Style::new().color(MUTED).align(TextAlign::Right)),
                )
                .control(Control::line().at(0.0, 52.0, w, 2.0).style(Style::new().color(LINE)))
                .control(party("EMITENTE", "Seller", 0.0))
                .control(party("CLIENTE", "Customer", 200.0))
                .control(
                    Control::table(
                        Table::new()
                            .row(TableRow::new(14.0).text(110.0, "EMISSÃO").style_name("caption"))
                            .row(
                                TableRow::new(18.0)
                                    .field(110.0, "[IssueDate]", Some("dd/MM/yyyy"))
                                    .style_name("td"),
                            )
                            .row(TableRow::new(14.0).text(110.0, "VENCIMENTO").style_name("caption"))
                            .row(
                                TableRow::new(18.0)
                                    .field(110.0, "[DueDate]", Some("dd/MM/yyyy"))
                                    .cell_style(cell().bold().color(ACCENT)),
                            ),
                    )
                    .at(w - 110.0, 70.0, 110.0, 64.0),
                ),
        )
        .detail_report(
            DetailReport::new("Items")
                .header(
                    Band::new(22.0).control(
                        Control::table(
                            Table::new().row(
                                TableRow::new(22.0)
                                    .text(cols[0], "CÓDIGO")
                                    .text(cols[1], "DESCRIÇÃO")
                                    .text(cols[2], "QTD")
                                    .cell_style(Style::new().align(TextAlign::Right))
                                    .text(cols[3], "PREÇO UNIT.")
                                    .cell_style(Style::new().align(TextAlign::Right))
                                    .text(cols[4], "TOTAL")
                                    .cell_style(Style::new().align(TextAlign::Right))
                                    .style_name("th"),
                            ),
                        )
                        .at(0.0, 0.0, w, 22.0),
                    ),
                )
                .detail(
                    Band::new(20.0).control(
                        Control::table(
                            Table::new().row(
                                TableRow::new(20.0)
                                    .field(cols[0], "[Sku]", None)
                                    .field(cols[1], "[Description]", None)
                                    .field(cols[2], "[Quantity]", None)
                                    .cell_style(Style::new().align(TextAlign::Right))
                                    .field(cols[3], "[UnitPrice]", Some("C2"))
                                    .cell_style(Style::new().align(TextAlign::Right))
                                    .field(cols[4], "[Quantity] * [UnitPrice]", Some("C2"))
                                    .cell_style(Style::new().align(TextAlign::Right))
                                    .style_name("td"),
                            ),
                        )
                        .at(0.0, 0.0, w, 20.0),
                    ),
                ),
        )
        .report_footer(
            Band::new(150.0)
                .control(s1)
                .control(s2)
                .control(d1)
                .control(d2)
                .control(t1)
                .control(t2)
                .control(Control::label("OBSERVAÇÕES").at(0.0, 90.0, 300.0, 14.0).style(caption()))
                .control(
                    Control::field("[Notes]")
                        .at(0.0, 104.0, w, 30.0)
                        .style(Style::new().font_size(8.5).color(MUTED).valign(VerticalAlign::Top)),
                ),
        )
        .page_footer(page_footer(true))
}

/// A customer list grouped by the first letter, the simplest kind of report.
pub fn customer_list() -> Report {
    let w = PageSettings::a4().content_width();
    Report::new("Clientes")
        .culture("pt-BR")
        .data_source(DataSource {
            name: "Vendas".into(),
            kind: DataSourceKind::Json { data: sales_data(60) },
        })
        .sort_by("[Customer]", false)
        .report_header(
            Band::new(40.0).control(Control::label("Clientes").at(0.0, 0.0, 300.0, 30.0).style(title())),
        )
        .group(
            Group::new("Substring([Customer], 0, 1)").header(
                Band::new(26.0)
                    .control(
                        Control::field("Substring([Customer], 0, 1)")
                            .at(0.0, 4.0, 30.0, 20.0)
                            .style(Style::new().font_size(14.0).bold().color(ACCENT)),
                    )
                    .keep_with_next(),
            ),
        )
        .detail(
            Band::new(18.0)
                .control(Control::field("[Customer]").at(20.0, 0.0, 200.0, 18.0).style(cell()))
                .control(
                    Control::field("[City] + ' · ' + [Region]")
                        .at(220.0, 0.0, 200.0, 18.0)
                        .style(cell().color(MUTED)),
                )
                .control(
                    Control::check_box("[Paid]", "Em dia")
                        .at(w - 80.0, 0.0, 80.0, 18.0)
                        .style(Style::new().font_size(8.5).color(MUTED)),
                ),
        )
        .page_footer(page_footer(true))
}
