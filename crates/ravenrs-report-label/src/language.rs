//! Printer languages. Each one gets the label as one raster, with its own polarity and framing; what
//! was confirmed in which manual is in FONTES.md.

use std::fmt::Write as _;

use ravenrs_report::Document;
use ravenrs_text::TextSystem;

use crate::bitmap::Bitmap;
use crate::error::LabelError;
use crate::raster::rasterize_page;

/// A printer language.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Language {
    /// ZPL II, from Zebra.
    Zpl,
    /// EPL2, from Eltron and Zebra. Printers that call it PPLB take the same bytes.
    Epl,
    /// PPLA, from Argox.
    Ppla,
    /// TSPL and TSPL2, from TSC.
    Tspl,
}

impl Language {
    /// PPLB is treated as [`Language::Epl`]: see FONTES.md for how far this is confirmed.
    pub const PPLB: Self = Self::Epl;

    /// The name to show.
    pub fn name(self) -> &'static str {
        match self {
            Self::Zpl => "ZPL II",
            Self::Epl => "EPL2/PPLB",
            Self::Ppla => "PPLA",
            Self::Tspl => "TSPL",
        }
    }

    /// The most copies of a label the language's command takes.
    fn max_copies(self) -> u32 {
        match self {
            Self::Zpl => 99_999_999,
            Self::Epl => 65_535,
            Self::Ppla => 9_999,
            Self::Tspl => 999_999_999,
        }
    }
}

/// What to print and how.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LabelJob {
    /// The printer's language.
    pub language: Language,
    /// The printer's resolution: 203 or 300.
    pub dpi: u32,
    /// How many copies of each label.
    pub copies: u32,
    /// The gap between labels, in millimeters. `None` keeps what the printer sensed. Only EPL and TSPL
    /// have a command for it: ZPL and PPLA printers sense the gap by themselves.
    pub gap_mm: Option<f32>,
    /// How dark to burn, from 0 to 15. `None` keeps the printer's setting.
    pub darkness: Option<u8>,
}

impl LabelJob {
    /// One copy of each label, with the printer's own gap and darkness.
    pub fn new(language: Language, dpi: u32) -> Self {
        Self { language, dpi, copies: 1, gap_mm: None, darkness: None }
    }

    fn check(&self) -> Result<(), LabelError> {
        if !matches!(self.dpi, 203 | 300) {
            return Err(LabelError::InvalidJob(format!(
                "resolução de {} dpi; as impressoras têm 203 ou 300",
                self.dpi
            )));
        }
        let most = self.language.max_copies();
        if self.copies == 0 || self.copies > most {
            return Err(LabelError::InvalidJob(format!(
                "{} cópias; {} aceita de 1 a {most}",
                self.copies,
                self.language.name()
            )));
        }
        if self.darkness.is_some_and(|darkness| darkness > 15) {
            return Err(LabelError::InvalidJob("a escuridão vai de 0 a 15".to_owned()));
        }
        if let Some(gap) = self.gap_mm {
            let dots = gap * self.dpi as f32 / 25.4;
            let accepted = match self.language {
                Language::Epl => {
                    let least = if self.dpi == 203 { 16.0 } else { 18.0 };
                    (least..=240.0).contains(&dots.round())
                }
                Language::Tspl => (0.0..=127.0).contains(&gap),
                Language::Zpl | Language::Ppla => gap.is_finite(),
            };
            if !accepted {
                return Err(LabelError::InvalidJob(format!(
                    "espaço de {gap} mm entre etiquetas, fora do que {} aceita",
                    self.language.name()
                )));
            }
        }
        Ok(())
    }
}

/// The bytes that print `document`, one label per page.
pub fn compile(document: &Document, job: &LabelJob) -> Result<Vec<u8>, LabelError> {
    compile_with(document, job, &mut TextSystem::new())
}

/// Same as [`compile`] with a text system of your own, which has the fonts the report was made with.
pub fn compile_with(
    document: &Document,
    job: &LabelJob,
    text: &mut TextSystem,
) -> Result<Vec<u8>, LabelError> {
    job.check()?;
    if document.pages.is_empty() {
        return Err(LabelError::EmptyDocument);
    }
    let mut bytes = Vec::new();
    for (index, page) in document.pages.iter().enumerate() {
        let on_label = |source| LabelError::OnLabel { label: index + 1, source: Box::new(source) };
        let bitmap = rasterize_page(page, job.dpi, text).map_err(on_label)?;
        bytes.extend(encode_bitmap(&bitmap, job).map_err(on_label)?);
    }
    Ok(bytes)
}

/// The bytes that print one picture as one label.
pub fn encode_bitmap(bitmap: &Bitmap, job: &LabelJob) -> Result<Vec<u8>, LabelError> {
    job.check()?;
    match job.language {
        Language::Zpl => Ok(zpl(bitmap, job)),
        Language::Epl => Ok(epl(bitmap, job)),
        Language::Ppla => ppla(bitmap, job),
        Language::Tspl => Ok(tspl(bitmap, job)),
    }
}

fn push_hex(out: &mut String, bytes: &[u8]) {
    for byte in bytes {
        let _ = write!(out, "{byte:02X}");
    }
}

/// `^GF` takes at most this many bytes in a field, so a tall label goes in several bands.
const ZPL_MAX_FIELD: usize = 99_999;

/// ZPL: `^GFA`, ASCII hexadecimal, black is 1.
fn zpl(bitmap: &Bitmap, job: &LabelJob) -> Vec<u8> {
    let mut out = String::new();
    if let Some(darkness) = job.darkness {
        // ~SD goes from 00 to 30.
        let _ = write!(out, "~SD{:02}", u32::from(darkness) * 2);
    }
    let _ = write!(out, "^XA^PW{}^LL{}^LH0,0", bitmap.width(), bitmap.height());
    let per_row = bitmap.bytes_per_row().max(1);
    let rows_per_band = (ZPL_MAX_FIELD / per_row).max(1);
    for (band, rows) in bitmap.bits().chunks(rows_per_band * per_row).enumerate() {
        let total = rows.len();
        let _ = write!(out, "^FO0,{}^GFA,{total},{total},{per_row},", band * rows_per_band);
        push_hex(&mut out, rows);
        out.push_str("^FS");
    }
    let _ = writeln!(out, "^PQ{}^XZ", job.copies);
    out.into_bytes()
}

/// EPL: `GW`, raw bytes, white is 1.
fn epl(bitmap: &Bitmap, job: &LabelJob) -> Vec<u8> {
    let mut head = format!("\nq{}\n", bitmap.width());
    if let Some(gap) = job.gap_mm {
        let dots = (gap * job.dpi as f32 / 25.4).round() as u32;
        let _ = writeln!(head, "Q{},{dots}", bitmap.height());
    }
    if let Some(darkness) = job.darkness {
        let _ = writeln!(head, "D{darkness}");
    }
    let _ = write!(head, "R0,0\nN\nGW0,0,{},{},", bitmap.bytes_per_row(), bitmap.height());
    let mut out = head.into_bytes();
    out.extend(bitmap.bits().iter().map(|byte| !byte));
    out.extend(format!("\nP{}\n", job.copies).into_bytes());
    out
}

/// The name the picture is stored under, in RAM, until the label is out.
const PPLA_IMAGE: &str = "RAVENRS";
const STX: char = '\u{2}';
/// A data record counts its bytes in two hex digits.
const PPLA_MAX_ROW: usize = 255;

/// PPLA: `<STX>IAF`, the HEX format, black is 1. The first record is the bottom row, since the
/// language's origin is the lower left corner.
fn ppla(bitmap: &Bitmap, job: &LabelJob) -> Result<Vec<u8>, LabelError> {
    let per_row = bitmap.bytes_per_row();
    if per_row > PPLA_MAX_ROW {
        return Err(LabelError::InvalidPage(format!(
            "{} pontos de largura; PPLA aceita até {} por linha",
            bitmap.width(),
            PPLA_MAX_ROW * 8
        )));
    }
    let mut out = format!("{STX}IAF{PPLA_IMAGE}\r");
    for row in bitmap.rows().rev() {
        let _ = write!(out, "80{per_row:02X}");
        push_hex(&mut out, row);
        out.push('\r');
    }
    out.push_str("FFFF\r");
    let _ = write!(out, "{STX}L\rD11\r");
    if let Some(darkness) = job.darkness {
        // H goes from 00 to 20.
        let _ = write!(out, "H{:02}\r", (u32::from(darkness) * 20 + 7) / 15);
    }
    let _ = write!(out, "1Y1100000000000{PPLA_IMAGE}\rQ{:04}\rE\r{STX}xAG{PPLA_IMAGE}\r", job.copies);
    Ok(out.into_bytes())
}

/// TSPL: `BITMAP` in overwrite mode, raw bytes, black is 0.
fn tspl(bitmap: &Bitmap, job: &LabelJob) -> Vec<u8> {
    let millimeters = |dots: u32| dots as f32 * 25.4 / job.dpi as f32;
    let mut head =
        format!("SIZE {:.1} mm,{:.1} mm\r\n", millimeters(bitmap.width()), millimeters(bitmap.height()));
    if let Some(gap) = job.gap_mm {
        let _ = write!(head, "GAP {gap:.1} mm,0 mm\r\n");
    }
    if let Some(darkness) = job.darkness {
        let _ = write!(head, "DENSITY {darkness}\r\n");
    }
    let _ =
        write!(head, "REFERENCE 0,0\r\nCLS\r\nBITMAP 0,0,{},{},0,", bitmap.bytes_per_row(), bitmap.height());
    let mut out = head.into_bytes();
    out.extend(bitmap.bits().iter().map(|byte| !byte));
    out.extend(format!("\r\nPRINT {},1\r\n", job.copies).into_bytes());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 16 x 2 dots: the top row starts black, the bottom row ends black.
    fn small() -> Bitmap {
        let mut bitmap = Bitmap::new(16, 2);
        bitmap.fill(0, 0, 4, 1, true);
        bitmap.fill(12, 1, 16, 2, true);
        bitmap
    }

    fn job(language: Language) -> LabelJob {
        LabelJob { copies: 2, ..LabelJob::new(language, 203) }
    }

    fn encoded(language: Language) -> Vec<u8> {
        encode_bitmap(&small(), &job(language)).unwrap_or_else(|e| panic!("{e}"))
    }

    #[test]
    fn zpl_bytes() {
        assert_eq!(
            String::from_utf8_lossy(&encoded(Language::Zpl)),
            "^XA^PW16^LL2^LH0,0^FO0,0^GFA,4,4,2,F000000F^FS^PQ2^XZ\n"
        );
    }

    #[test]
    fn epl_bytes_have_white_as_one() {
        let mut expected = b"\nq16\nR0,0\nN\nGW0,0,2,2,".to_vec();
        expected.extend([0x0F, 0xFF, 0xFF, 0xF0]);
        expected.extend(b"\nP2\n");
        assert_eq!(encoded(Language::Epl), expected);
    }

    #[test]
    fn tspl_bytes_have_black_as_zero() {
        let mut expected = b"SIZE 2.0 mm,0.3 mm\r\nREFERENCE 0,0\r\nCLS\r\nBITMAP 0,0,2,2,0,".to_vec();
        expected.extend([0x0F, 0xFF, 0xFF, 0xF0]);
        expected.extend(b"\r\nPRINT 2,1\r\n");
        assert_eq!(encoded(Language::Tspl), expected);
    }

    #[test]
    fn ppla_bytes_start_at_the_bottom_row() {
        assert_eq!(
            String::from_utf8_lossy(&encoded(Language::Ppla)),
            "\u{2}IAFRAVENRS\r8002000F\r8002F000\rFFFF\r\u{2}L\rD11\r1Y1100000000000RAVENRS\rQ0002\rE\r\u{2}xAGRAVENRS\r"
        );
    }

    #[test]
    fn darkness_and_gap_reach_each_language_in_its_own_scale() {
        let tuned = |language| {
            let job = LabelJob { gap_mm: Some(3.0), darkness: Some(15), ..LabelJob::new(language, 203) };
            String::from_utf8_lossy(&encode_bitmap(&small(), &job).unwrap_or_else(|e| panic!("{e}")))
                .into_owned()
        };
        assert!(tuned(Language::Zpl).starts_with("~SD30^XA"));
        assert!(tuned(Language::Ppla).contains("\rH20\r"));
        let epl = tuned(Language::Epl);
        assert!(epl.contains("Q2,24\n") && epl.contains("D15\n"), "{epl}");
        let tspl = tuned(Language::Tspl);
        assert!(tspl.contains("GAP 3.0 mm,0 mm\r\n") && tspl.contains("DENSITY 15\r\n"), "{tspl}");
    }

    #[test]
    fn settings_out_of_range_are_refused() {
        let check = |job: LabelJob| encode_bitmap(&small(), &job);
        let base = LabelJob::new(Language::Ppla, 203);
        assert!(matches!(check(LabelJob { dpi: 600, ..base }), Err(LabelError::InvalidJob(_))));
        assert!(matches!(check(LabelJob { copies: 0, ..base }), Err(LabelError::InvalidJob(_))));
        assert!(matches!(check(LabelJob { copies: 10_000, ..base }), Err(LabelError::InvalidJob(_))));
        assert!(matches!(check(LabelJob { darkness: Some(16), ..base }), Err(LabelError::InvalidJob(_))));
        let epl = LabelJob { gap_mm: Some(0.5), ..LabelJob::new(Language::Epl, 203) };
        assert!(matches!(check(epl), Err(LabelError::InvalidJob(_))));
    }

    #[test]
    fn a_row_too_wide_for_ppla_is_refused() {
        let wide = Bitmap::new(2048, 1);
        let job = LabelJob::new(Language::Ppla, 203);
        assert!(matches!(encode_bitmap(&wide, &job), Err(LabelError::InvalidPage(_))));
    }

    #[test]
    fn an_empty_document_is_an_error() {
        let job = LabelJob::new(Language::Zpl, 203);
        assert_eq!(compile(&Document::default(), &job), Err(LabelError::EmptyDocument));
    }
}
