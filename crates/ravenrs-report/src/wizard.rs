//! The report wizard: a finished table report from a data source and a choice
//! of fields, like the wizards of commercial report designers. Also lists the
//! fields a data source offers, for field lists.

use serde_json::Value as Json;

use crate::model::{
    Band, Control, DataSource, Group, PageSettings, Report, Style, Table, TableRow, TextAlign,
};

/// A field of a data source, as a field list shows it.
#[derive(Clone, Debug, PartialEq)]
pub struct FieldInfo {
    /// The path used in expressions, such as `Customer.Name`.
    pub path: String,
    /// What the field holds.
    pub kind: FieldKind,
}

/// The kind of value a field holds, guessed from sample data.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum FieldKind {
    /// Text.
    Text,
    /// A number.
    Number,
    /// A date (ISO 8601 text).
    Date,
    /// Yes or no.
    Boolean,
    /// A nested list of rows (master-detail).
    Collection,
}

/// The rows of `data` at `data_member` (or the data itself).
pub fn rows_of<'a>(data: &'a Json, data_member: Option<&str>) -> Vec<&'a Json> {
    let base = match data_member.filter(|m| !m.trim().is_empty()) {
        Some(path) => {
            let segs: Vec<String> = path.split('.').map(|s| s.trim().to_owned()).collect();
            crate::expr::lookup(Some(data), &segs)
        }
        None => Some(data),
    };
    match base {
        Some(Json::Array(items)) => items.iter().collect(),
        Some(Json::Null) | None => Vec::new(),
        Some(other) => vec![other],
    }
}

/// The fields of the rows of `data`, from the first few rows, nested objects
/// flattened to `Parent.Child`.
pub fn fields(data: &Json, data_member: Option<&str>) -> Vec<FieldInfo> {
    let mut out: Vec<FieldInfo> = Vec::new();
    for row in rows_of(data, data_member).into_iter().take(20) {
        collect(row, "", &mut out);
    }
    out
}

fn collect(value: &Json, prefix: &str, out: &mut Vec<FieldInfo>) {
    let Json::Object(map) = value else { return };
    for (key, v) in map {
        let path = if prefix.is_empty() { key.clone() } else { format!("{prefix}.{key}") };
        if let Json::Object(_) = v {
            collect(v, &path, out);
            continue;
        }
        let kind = match v {
            Json::Number(_) => FieldKind::Number,
            Json::Bool(_) => FieldKind::Boolean,
            Json::Array(_) => FieldKind::Collection,
            Json::String(s) if crate::value::DateTime::parse(s).is_some() => FieldKind::Date,
            Json::Null => continue,
            _ => FieldKind::Text,
        };
        if !out.iter().any(|f| f.path == path) {
            out.push(FieldInfo { path, kind });
        }
    }
}

/// A readable caption for a field path: `UnitPrice` → `Unit Price`, `Customer.Name` → `Name`.
pub fn caption(path: &str) -> String {
    let last = path.rsplit('.').next().unwrap_or(path);
    let mut out = String::new();
    let mut prev_lower = false;
    for c in last.chars() {
        if c == '_' {
            out.push(' ');
            prev_lower = false;
            continue;
        }
        if c.is_uppercase() && prev_lower {
            out.push(' ');
        }
        prev_lower = c.is_lowercase() || c.is_ascii_digit();
        out.push(c);
    }
    out
}

/// The default format for a field kind in a culture-neutral way.
pub fn default_format(kind: FieldKind) -> Option<&'static str> {
    match kind {
        FieldKind::Date => Some("d"),
        _ => None,
    }
}

/// Options of the wizard.
#[derive(Clone, Debug)]
pub struct WizardOptions {
    /// The report title.
    pub title: String,
    /// Culture for formats (`pt-BR`, `en-US`).
    pub culture: String,
    /// The fields to show, as columns, in order.
    pub columns: Vec<FieldInfo>,
    /// Group rows by this field, with a header and subtotals.
    pub group_by: Option<String>,
    /// Number fields that get totals (group subtotals and a grand total).
    pub totals: Vec<String>,
    /// Paper; landscape when there are many columns.
    pub page: PageSettings,
    /// The accent color of headers.
    pub accent: String,
}

impl Default for WizardOptions {
    fn default() -> Self {
        Self {
            title: "Report".into(),
            culture: "en-US".into(),
            columns: Vec::new(),
            group_by: None,
            totals: Vec::new(),
            page: PageSettings::a4(),
            accent: "#4c66ee".into(),
        }
    }
}

/// Builds a table report: title, column headers repeated on each page, zebra
/// rows, optional grouping with subtotals, grand totals, page numbers.
pub fn table_report(source: DataSource, options: &WizardOptions) -> Report {
    let page = if options.columns.len() > 6 { options.page.landscape() } else { options.page };
    let width = page.content_width();
    // Text columns get more room than numbers and dates.
    let weights: Vec<f32> =
        options.columns.iter().map(|c| if matches!(c.kind, FieldKind::Text) { 2.0 } else { 1.0 }).collect();
    let total_weight: f32 = weights.iter().sum::<f32>().max(1.0);
    let widths: Vec<f32> = weights.iter().map(|w| (width * w / total_weight).floor()).collect();
    let align = |kind: FieldKind| match kind {
        FieldKind::Number => TextAlign::Right,
        FieldKind::Boolean | FieldKind::Date => TextAlign::Center,
        _ => TextAlign::Left,
    };
    let pt = options.culture.to_lowercase().starts_with("pt");
    // Whole-number columns show no decimals.
    let rows_data = match &source.kind {
        crate::model::DataSourceKind::Json { data } => rows_of(data, None),
        _ => Vec::new(),
    };
    let whole: Vec<String> = options
        .columns
        .iter()
        .filter(|c| {
            c.kind == FieldKind::Number
                && rows_data.iter().take(200).all(|r| {
                    let segs: Vec<String> = c.path.split('.').map(str::to_owned).collect();
                    crate::expr::lookup(Some(r), &segs)
                        .and_then(Json::as_f64)
                        .is_none_or(|n| n.fract() == 0.0)
                })
        })
        .map(|c| c.path.clone())
        .collect();
    let number_format = |path: &str| if whole.iter().any(|w| w == path) { "N0" } else { "N2" };
    let mut header = TableRow::new(22.0);
    let mut detail = TableRow::new(18.0);
    for (c, w) in options.columns.iter().zip(&widths) {
        header =
            header.text(*w, &caption(&c.path).to_uppercase()).cell_style(Style::new().align(align(c.kind)));
        let expr = match c.kind {
            FieldKind::Boolean if pt => format!("Iif([{}], 'Sim', 'Não')", c.path),
            FieldKind::Boolean => format!("Iif([{}], 'Yes', 'No')", c.path),
            _ => format!("[{}]", c.path),
        };
        let format = match c.kind {
            FieldKind::Number => Some(number_format(&c.path)),
            kind => default_format(kind),
        };
        detail = detail.field(*w, &expr, format).cell_style(Style::new().align(align(c.kind)));
    }
    let header = header.style_name("th");
    let detail = detail.style_name("td");
    let totals_row = |label: &str, style: &str| {
        let mut row = TableRow::new(20.0);
        for (i, (c, w)) in options.columns.iter().zip(&widths).enumerate() {
            row = if options.totals.contains(&c.path) {
                row.field(*w, &format!("Sum([{}])", c.path), Some(number_format(&c.path)))
                    .cell_style(Style::new().align(TextAlign::Right))
            } else if i == 0 {
                row.text(*w, label)
            } else {
                row.text(*w, "")
            };
        }
        row.style_name(style)
    };
    let mut report = Report::new(&options.title)
        .culture(&options.culture)
        .page(page)
        .data_source(source)
        .style(
            "th",
            Style::new().font_size(8.0).bold().color("#ffffff").background(&options.accent).padding(4.0),
        )
        .style("td", Style::new().font_size(9.0).color("#1d2433").padding(4.0))
        .style(
            "total",
            Style::new().font_size(9.0).bold().color("#1d2433").padding(4.0).border("top", 1.0, "#1d2433"),
        )
        .style("odd", Style::new().background("#ffffff"))
        .style("even", Style::new().background("#f4f6fb"))
        .report_header(
            Band::new(48.0)
                .control(
                    Control::label(&options.title)
                        .at(0.0, 0.0, width, 30.0)
                        .style(Style::new().font_size(20.0).bold().color("#1d2433")),
                )
                .control(
                    Control::field(if pt {
                        "FormatString('{0:N0} registros', Count())"
                    } else {
                        "FormatString('{0:N0} records', Count())"
                    })
                    .at(0.0, 30.0, width, 16.0)
                    .style(Style::new().font_size(9.0).color("#6b7385")),
                ),
        )
        .page_header(
            Band::new(22.0).control(Control::table(Table::new().row(header)).at(0.0, 0.0, width, 22.0)),
        )
        .detail(
            Band::new(18.0)
                .control(Control::table(Table::new().row(detail)).at(0.0, 0.0, width, 18.0))
                .zebra("odd", "even"),
        )
        .page_footer(
            Band::new(20.0).control(
                Control::field(if pt {
                    "FormatString('Página {0} de {1}', PageNumber(), PageCount())"
                } else {
                    "FormatString('Page {0} of {1}', PageNumber(), PageCount())"
                })
                .at(0.0, 4.0, width, 14.0)
                .style(Style::new().font_size(7.5).color("#6b7385").align(TextAlign::Right)),
            ),
        );
    if let Some(field) = &options.group_by {
        let mut group = Group::new(&format!("[{field}]")).header(
            Band::new(26.0)
                .control(
                    Control::field(format!("[{field}]"))
                        .at(0.0, 6.0, width, 18.0)
                        .style(Style::new().font_size(11.0).bold().color(&options.accent)),
                )
                .keep_with_next(),
        );
        if !options.totals.is_empty() {
            group = group.footer(Band::new(22.0).control(
                Control::table(Table::new().row(totals_row("Subtotal", "total"))).at(0.0, 0.0, width, 20.0),
            ));
        }
        report = report.group(group);
    }
    if !options.totals.is_empty() {
        report = report.report_footer(
            Band::new(28.0).control(
                Control::table(
                    Table::new().row(totals_row(if pt { "Total geral" } else { "Grand total" }, "total")),
                )
                .at(0.0, 6.0, width, 20.0),
            ),
        );
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::DataSourceKind;
    use crate::{ReportEngine, samples};

    #[test]
    fn fields_flatten_objects_and_detect_kinds() {
        let fields = fields(&samples::invoice_data(), None);
        let get = |p: &str| fields.iter().find(|f| f.path == p).map(|f| f.kind);
        assert_eq!(get("Customer.Name"), Some(FieldKind::Text));
        assert_eq!(get("IssueDate"), Some(FieldKind::Date));
        assert_eq!(get("Discount"), Some(FieldKind::Number));
        assert_eq!(get("Items"), Some(FieldKind::Collection));
        assert_eq!(caption("UnitPrice"), "Unit Price");
        assert_eq!(caption("Customer.Name"), "Name");
    }

    #[test]
    fn wizard_builds_a_grouped_report_with_totals() {
        let data = samples::sales_data(120);
        let all = fields(&data, None);
        let pick = |p: &str| all.iter().find(|f| f.path == p).cloned().expect(p);
        let options = WizardOptions {
            title: "Vendas".into(),
            culture: "pt-BR".into(),
            columns: vec![pick("Customer"), pick("Product"), pick("Quantity"), pick("Paid")],
            group_by: Some("Region".into()),
            totals: vec!["Quantity".into()],
            ..WizardOptions::default()
        };
        let report = table_report(
            DataSource { name: "Vendas".into(), kind: DataSourceKind::Json { data: data.clone() } },
            &options,
        );
        let doc = ReportEngine::new().render(&report, report.embedded_data().unwrap_or(&data));
        assert!(doc.errors.is_empty(), "{:?}", doc.errors);
        assert!(doc.page_contains(0, "CUSTOMER") && doc.page_contains(0, "Vendas"));
        assert!(doc.page_contains(0, "120 registros"));
        let last = doc.pages.len() - 1;
        assert!(doc.page_contains(last, "Total geral"));
        assert!(doc.page_contains(last, &format!("Página {} de {}", last + 1, doc.pages.len())));
    }
}
