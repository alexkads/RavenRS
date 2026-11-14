//! Turns what a [`DataGrid`](ravenrs_widgets::DataGrid) shows into a file. The rows go
//! through the report engine and its exporters, so a grid exports like a report does.
//!
//! ```
//! use ravenrs_grid_export::export;
//! use ravenrs_widgets::grid::{GridQuery, to_table};
//! use ravenrs_widgets::{GridColumn, GridExportFormat};
//!
//! let columns = [GridColumn::text("Name"), GridColumn::number("Total")];
//! let rows = vec![vec!["Ana".into(), 12.5.into()]];
//! let table = to_table("Sales", "en-US", &columns, &rows, &GridQuery::default(), &[0]);
//! let file = export(&table, GridExportFormat::Xlsx, Some("2026-11-02"))?;
//! assert_eq!(file.name, "Sales 2026-11-02.xlsx");
//! # Ok::<(), String>(())
//! ```

use ravenrs_report::wizard::{ColumnWords, FieldInfo, FieldKind, WizardOptions, table_report_with};
use ravenrs_report::{DataSource, DataSourceKind, Document, ReportEngine};
use ravenrs_report_export::{ExportFormat, ExportedFile};
use ravenrs_style::Theme;
use ravenrs_widgets::{GridExportFormat, GridTable, GridTableKind, Value};
use serde_json::{Map, Value as Json};

/// A field name that nothing in a title can break.
fn field(column: usize) -> String {
    format!("c{column}")
}

fn json_of(value: &Value) -> Json {
    match value {
        Value::Text(text) => Json::String(text.to_string()),
        Value::Number(number) => serde_json::Number::from_f64(*number).map_or(Json::Null, Json::Number),
        Value::Bool(on) => Json::Bool(*on),
        Value::Date(date) => {
            Json::String(format!("{:04}-{:02}-{:02}", date.year(), date.month(), date.day()))
        }
        _ => Json::Null,
    }
}

/// The format of a number: its decimals, with its text around them.
fn number_pattern(decimals: usize, prefix: &str, suffix: &str) -> String {
    let digits = if decimals == 0 { "#,##0".to_owned() } else { format!("#,##0.{}", "0".repeat(decimals)) };
    format!("{prefix}{digits}{suffix}")
}

/// What reads well on paper of `theme`: its accent, with its own text, for the headers, and its
/// text where that is dark enough for white paper. A dark theme's light text stays out, as paper is
/// white; then the colors of RavenRS take its place.
pub fn paper_colors(theme: &Theme, options: WizardOptions) -> WizardOptions {
    let hex = |color: ravenrs_core::Color| {
        let byte = |channel: f32| (channel.clamp(0.0, 1.0) * 255.0).round() as u8;
        format!("#{:02x}{:02x}{:02x}", byte(color.r), byte(color.g), byte(color.b))
    };
    let palette = &theme.palette;
    let on_paper =
        |color: ravenrs_core::Color| color.with_alpha(1.0).contrast_ratio(ravenrs_core::Color::WHITE) >= 4.5;
    let light = on_paper(palette.ink);
    WizardOptions {
        accent: hex(palette.accent),
        on_accent: hex(palette.on_accent),
        ink: if light { hex(palette.ink) } else { options.ink.clone() },
        muted: if light && on_paper(palette.ink_muted) {
            hex(palette.ink_muted)
        } else {
            options.muted.clone()
        },
        zebra: if light { hex(palette.bg_300) } else { options.zebra.clone() },
        ..options
    }
}

/// `table` laid out as a document, the same one the files are made from, in the colors of
/// RavenRS. [`document_of_with`] uses those of a theme.
pub fn document_of(table: &GridTable) -> Document {
    document_with(table, WizardOptions::default())
}

/// `table` laid out as a document in the colors of `theme`, as [`paper_colors`] puts them.
pub fn document_of_with(table: &GridTable, theme: &Theme) -> Document {
    document_with(table, paper_colors(theme, WizardOptions::default()))
}

fn document_with(table: &GridTable, colors: WizardOptions) -> Document {
    let rows: Vec<Json> = table
        .rows
        .iter()
        .map(|row| {
            let cells = row.iter().enumerate().map(|(c, value)| (field(c), json_of(value)));
            Json::Object(cells.collect::<Map<_, _>>())
        })
        .collect();
    let mut columns = Vec::with_capacity(table.columns.len());
    let mut words = Vec::with_capacity(table.columns.len());
    let mut totals = Vec::new();
    for (c, column) in table.columns.iter().enumerate() {
        let (kind, pattern) = match &column.kind {
            GridTableKind::Number { decimals, prefix, suffix } => {
                (FieldKind::Number, Some(number_pattern(*decimals, prefix, suffix)))
            }
            GridTableKind::Date => (FieldKind::Date, None),
            GridTableKind::Boolean => (FieldKind::Boolean, None),
            _ => (FieldKind::Text, None),
        };
        columns.push(FieldInfo { path: field(c), kind });
        words.push(ColumnWords { caption: Some(column.title.clone()), format: pattern });
        if column.total {
            totals.push(field(c));
        }
    }
    let options = WizardOptions {
        title: table.title.clone(),
        culture: table.culture.clone(),
        columns,
        group_by: table.group_by.map(field),
        totals,
        ..colors
    };
    let data = Json::Array(rows);
    let source = DataSource { name: "rows".into(), kind: DataSourceKind::Json { data: data.clone() } };
    let report = table_report_with(source, &options, &words);
    ReportEngine::new().render(&report, &data)
}

/// Writes `table` in `format`, in the colors of `theme`, as [`export`] does.
pub fn export_with(
    table: &GridTable,
    format: GridExportFormat,
    suffix: Option<&str>,
    theme: &Theme,
) -> Result<ExportedFile, String> {
    file_of(&document_of_with(table, theme), format, suffix)
}

fn file_of(
    document: &Document,
    format: GridExportFormat,
    suffix: Option<&str>,
) -> Result<ExportedFile, String> {
    let format = match format {
        GridExportFormat::Xlsx => ExportFormat::Xlsx,
        GridExportFormat::Pdf => ExportFormat::Pdf,
        _ => ExportFormat::Csv,
    };
    ravenrs_report_export::export(document, format, suffix)
}

/// Writes `table` in `format`. `suffix`, such as a date, goes after the title in the name.
pub fn export(
    table: &GridTable,
    format: GridExportFormat,
    suffix: Option<&str>,
) -> Result<ExportedFile, String> {
    file_of(&document_of(table), format, suffix)
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use ravenrs_widgets::grid::{GridQuery, to_table};
    use ravenrs_widgets::{Date, GridColumn, Summary};

    use super::*;

    fn sample() -> GridTable {
        let columns = [
            GridColumn::text("Conta. Principal"),
            GridColumn::currency("Valor", "R$ ", 2).summary(Summary::Sum),
            GridColumn::date("Data"),
            GridColumn::text("Oculta").hidden(),
        ];
        let date = Date::new(2026, 11, 2).expect("a date");
        let rows = vec![
            vec!["Ana".into(), 1234.5.into(), Value::Date(date), "x".into()],
            vec!["Bia".into(), 10.0.into(), Value::Date(date), "y".into()],
        ];
        let query = GridQuery {
            hidden: columns.iter().map(|c| c.title() == "Oculta").collect(),
            ..GridQuery::default()
        };
        to_table("Contas", "pt-BR", &columns, &rows, &query, &[1, 0])
    }

    #[test]
    fn the_table_is_what_the_grid_shows() {
        let table = sample();
        assert_eq!(table.columns.len(), 3, "the hidden column stays out");
        assert_eq!(table.rows[0][0], Value::from("Bia"), "in the grid's order");
        assert!(table.columns[1].total);
        assert_eq!(table.columns[2].kind, GridTableKind::Date);
    }

    #[test]
    fn numbers_keep_their_text_around_them() {
        assert_eq!(number_pattern(2, "R$ ", ""), "R$ #,##0.00");
        assert_eq!(number_pattern(0, "", " un"), "#,##0 un");
    }

    fn fills(document: &Document) -> Vec<ravenrs_core::Color> {
        let items = document.pages.iter().flat_map(|page| page.items.iter());
        items
            .filter_map(|item| if let ravenrs_report::Item::Rect { fill, .. } = item { *fill } else { None })
            .collect()
    }

    #[test]
    fn the_headers_take_the_theme_s_accent() {
        let accent = ravenrs_core::Color::hex(0x0f5e66);
        let mut theme = Theme { name: "Morning Star", ..Theme::raven_light() };
        theme.palette.accent = accent;
        let close = |a: ravenrs_core::Color, b: ravenrs_core::Color| {
            (a.r - b.r).abs() < 0.01 && (a.g - b.g).abs() < 0.01 && (a.b - b.b).abs() < 0.01
        };
        assert!(fills(&document_of_with(&sample(), &theme)).iter().any(|fill| close(*fill, accent)));
        let raven = ravenrs_core::Color::hex(0x3f57db);
        assert!(
            fills(&document_of(&sample())).iter().any(|fill| close(*fill, raven)),
            "without a theme, as before"
        );
    }

    #[test]
    fn a_dark_theme_keeps_dark_text_on_white_paper() {
        let colors = paper_colors(&Theme::raven_dark(), WizardOptions::default());
        assert_eq!(colors.ink, WizardOptions::default().ink);
        assert_eq!(colors.zebra, WizardOptions::default().zebra);
        let light = paper_colors(&Theme::raven_light(), WizardOptions::default());
        assert_ne!(light.zebra, WizardOptions::default().zebra, "a light theme's own ground");
        assert_eq!(light.on_accent, "#ffffff");
    }

    #[test]
    fn every_format_makes_a_file() {
        let table = sample();
        let pdf = export(&table, GridExportFormat::Pdf, None).expect("a pdf");
        assert_eq!(pdf.name, "Contas.pdf");
        assert!(pdf.bytes.starts_with(b"%PDF"));
        let xlsx = export(&table, GridExportFormat::Xlsx, None).expect("a workbook");
        assert!(xlsx.bytes.starts_with(b"PK"), "a workbook is a zip");
        let csv = export(&table, GridExportFormat::Csv, None).expect("a csv");
        let text = String::from_utf8(csv.bytes).expect("text");
        assert!(text.contains("CONTA. PRINCIPAL"), "a title with a dot is a caption: {text}");
        assert!(text.contains("Bia"));
        assert!(text.contains("R$ 1.234,50"), "the column's format, in pt-BR: {text}");
    }
}
