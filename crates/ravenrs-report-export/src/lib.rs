//! One way to turn a report [`Document`] into a file. The editor, the server and
//! any other module call this, so a report exports the same from anywhere.
//!
//! ```
//! use ravenrs_report::{Band, Control, Report, ReportEngine};
//! use ravenrs_report_export::{ExportFormat, export};
//! use serde_json::json;
//!
//! let report = Report::new("Vendas").detail(Band::new(18.0).control(Control::field("[N]").at(0.0, 0.0, 200.0, 18.0)));
//! let document = ReportEngine::new().render(&report, &json!([{ "N": "Ana" }]));
//! let file = export(&document, ExportFormat::Pdf, Some("2026-11-01"))?;
//! assert_eq!(file.name, "Vendas 2026-11-01.pdf");
//! assert_eq!(file.mime, "application/pdf");
//! # Ok::<(), String>(())
//! ```

use ravenrs_report::Document;
use serde::{Deserialize, Serialize};

/// A format a document can be exported to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum ExportFormat {
    /// PDF, vector pages with embedded fonts.
    Pdf,
    /// Excel workbook, with real numbers and dates in the cells.
    Xlsx,
    /// Comma- or semicolon-separated text, as printed.
    Csv,
}

impl ExportFormat {
    /// The file extension.
    pub fn extension(self) -> &'static str {
        match self {
            Self::Pdf => "pdf",
            Self::Xlsx => "xlsx",
            Self::Csv => "csv",
        }
    }

    /// The media type.
    pub fn mime(self) -> &'static str {
        match self {
            Self::Pdf => "application/pdf",
            Self::Xlsx => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
            Self::Csv => "text/csv; charset=utf-8",
        }
    }
}

/// A file ready to be saved or sent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExportedFile {
    /// The format.
    pub format: ExportFormat,
    /// A name that is safe on every file system.
    pub name: String,
    /// The media type.
    pub mime: &'static str,
    /// The content.
    pub bytes: Vec<u8>,
}

/// Writes `document` in `format`. `suffix`, such as a date, goes after the title in the name.
pub fn export(
    document: &Document,
    format: ExportFormat,
    suffix: Option<&str>,
) -> Result<ExportedFile, String> {
    let bytes = match format {
        ExportFormat::Pdf => ravenrs_report_pdf::to_pdf(document).map_err(|e| e.to_string())?,
        ExportFormat::Xlsx => ravenrs_report_xlsx::to_xlsx(document).map_err(|e| e.to_string())?,
        // A byte order mark, so Excel reads the accents as UTF-8.
        ExportFormat::Csv => format!("\u{feff}{}", ravenrs_report_xlsx::to_csv(document)).into_bytes(),
    };
    Ok(ExportedFile { format, name: file_name(&document.title, suffix, format), mime: format.mime(), bytes })
}

/// A title as a file name, keeping accents and dropping what file systems refuse.
pub fn file_name(title: &str, suffix: Option<&str>, format: ExportFormat) -> String {
    let clean: String = title
        .chars()
        .map(|c| if c.is_control() || r#"<>:"/\|?*"#.contains(c) { ' ' } else { c })
        .take(100)
        .collect();
    let clean = clean.trim().trim_matches('.').trim();
    let stem = if clean.is_empty() { "report" } else { clean };
    match suffix {
        Some(suffix) => format!("{stem} {suffix}.{}", format.extension()),
        None => format!("{stem}.{}", format.extension()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_names_drop_what_file_systems_refuse() {
        assert_eq!(
            file_name("Vendas: Sul/Norte", Some("2026-11-01"), ExportFormat::Pdf),
            "Vendas  Sul Norte 2026-11-01.pdf"
        );
        assert_eq!(file_name("Relatório.", None, ExportFormat::Xlsx), "Relatório.xlsx");
        assert_eq!(file_name("../..", None, ExportFormat::Csv), "report.csv");
        assert_eq!(file_name("  ?? ", None, ExportFormat::Csv), "report.csv");
    }

    #[test]
    fn each_format_gives_a_file_of_its_kind() -> Result<(), String> {
        let document = Document { title: "T".to_owned(), ..Document::default() };
        assert!(export(&document, ExportFormat::Pdf, None)?.bytes.starts_with(b"%PDF-"));
        assert!(export(&document, ExportFormat::Xlsx, None)?.bytes.starts_with(b"PK"));
        assert!(export(&document, ExportFormat::Csv, None)?.bytes.starts_with("\u{feff}".as_bytes()));
        Ok(())
    }

    #[test]
    fn formats_read_as_lowercase_names() {
        assert_eq!(serde_json::from_str::<ExportFormat>("\"pdf\"").ok(), Some(ExportFormat::Pdf));
        assert!(serde_json::from_str::<ExportFormat>("\"docx\"").is_err());
    }
}
