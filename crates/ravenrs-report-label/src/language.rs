//! Printer languages. Each one gets the label as one raster, with its own polarity and framing; what
//! was confirmed in which manual is in FONTES.md.

use std::fmt::Write as _;

use ravenrs_report::{Document, Item, Page, PrinterAction, PrinterCommand};
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
    /// ESC/POS, from Epson, on receipt printers. Each page is a receipt, and the paper is cut after it.
    EscPos,
}

impl Language {
    /// PPLB is treated as [`Language::Epl`]: see FONTES.md for how far this is confirmed.
    pub const PPLB: Self = Self::Epl;

    /// Every language, to offer as a choice.
    pub fn all() -> &'static [Self] {
        &[Self::Zpl, Self::Epl, Self::Ppla, Self::Tspl, Self::EscPos]
    }

    /// The language with this name, in capitals or not, as [`Self::name`] gives it or as it is known.
    pub fn from_name(name: &str) -> Option<Self> {
        match name.trim().to_ascii_uppercase().as_str() {
            "ZPL" | "ZPL II" => Some(Self::Zpl),
            "EPL" | "EPL2" | "EPL2/PPLB" | "PPLB" => Some(Self::Epl),
            "PPLA" => Some(Self::Ppla),
            "TSPL" => Some(Self::Tspl),
            "ESC/POS" => Some(Self::EscPos),
            _ => None,
        }
    }

    /// The name to show.
    pub fn name(self) -> &'static str {
        match self {
            Self::Zpl => "ZPL II",
            Self::Epl => "EPL2/PPLB",
            Self::Ppla => "PPLA",
            Self::Tspl => "TSPL",
            Self::EscPos => "ESC/POS",
        }
    }

    /// The most copies of a label the language's command takes. ESC/POS has no such command: the
    /// whole receipt is sent again, so the limit only keeps a typo from emptying the roll.
    fn max_copies(self) -> u32 {
        match self {
            Self::Zpl => 99_999_999,
            Self::Epl => 65_535,
            Self::Ppla => 9_999,
            Self::Tspl => 999_999_999,
            Self::EscPos => 99,
        }
    }

    /// The resolutions its printers come in.
    fn resolutions(self) -> &'static [u32] {
        match self {
            Self::EscPos => &[203, 180],
            _ => &[203, 300],
        }
    }
}

/// What to print and how.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LabelJob {
    /// The printer's language.
    pub language: Language,
    /// The printer's resolution: 203 or 300 on label printers, 203 or 180 on receipt printers.
    pub dpi: u32,
    /// How many copies of each label.
    pub copies: u32,
    /// The gap between labels, in millimeters. `None` keeps what the printer sensed. Only EPL and TSPL
    /// have a command for it: ZPL and PPLA printers sense the gap by themselves, and a receipt roll
    /// has none.
    pub gap_mm: Option<f32>,
    /// How dark to burn, from 0 to 15. `None` keeps the printer's setting. ESC/POS ignores it, since
    /// its printers set this in their own configuration.
    pub darkness: Option<u8>,
}

impl LabelJob {
    /// One copy of each label, with the printer's own gap and darkness.
    pub fn new(language: Language, dpi: u32) -> Self {
        Self { language, dpi, copies: 1, gap_mm: None, darkness: None }
    }

    fn check(&self) -> Result<(), LabelError> {
        let resolutions = self.language.resolutions();
        if !resolutions.contains(&self.dpi) {
            return Err(LabelError::InvalidJob(format!(
                "resolução de {} dpi; {} aceita {resolutions:?}",
                self.dpi,
                self.language.name()
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
                Language::Zpl | Language::Ppla | Language::EscPos => gap.is_finite(),
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
        let encoded = match job.language {
            Language::EscPos => escpos(&bitmap, &printer_commands(page, &bitmap, job.dpi), job),
            _ => encode_bitmap(&bitmap, job),
        };
        bytes.extend(encoded.map_err(on_label)?);
    }
    Ok(bytes)
}

/// The page's printer commands in the order they come down the paper, each with the row it comes
/// before. Those on the same row keep the order they have on the page.
fn printer_commands(page: &Page, bitmap: &Bitmap, dpi: u32) -> Vec<(usize, PrinterCommand)> {
    let mut commands: Vec<(usize, PrinterCommand)> = page
        .items
        .iter()
        .filter_map(|item| match item {
            Item::PrinterCommand { y, command } => {
                let row = (y * dpi as f32 / 72.0).round().clamp(0.0, bitmap.height() as f32);
                Some((row as usize, *command))
            }
            _ => None,
        })
        .collect();
    commands.sort_by_key(|(row, _)| *row);
    commands
}

/// The bytes that print one picture as one label. A page's printer commands only reach the printer
/// through [`compile`].
pub fn encode_bitmap(bitmap: &Bitmap, job: &LabelJob) -> Result<Vec<u8>, LabelError> {
    job.check()?;
    match job.language {
        Language::Zpl => Ok(zpl(bitmap, job)),
        Language::Epl => Ok(epl(bitmap, job)),
        Language::Ppla => ppla(bitmap, job),
        Language::Tspl => Ok(tspl(bitmap, job)),
        Language::EscPos => escpos(bitmap, &[], job),
    }
}

/// A block of `GS v 0` carries at most this many rows. Far below what the manuals allow, since the
/// limit varies by model and a block must fit the printer's buffer.
const ESCPOS_BLOCK_ROWS: usize = 1024;
/// `GS v 0` counts a row's bytes in two bytes, but Epson's reference stops at this.
const ESCPOS_MAX_ROW: usize = 255;
/// How far past the cutting position to feed before cutting, in vertical motion units.
const ESCPOS_FEED: u8 = 24;
/// `ESC p`: pin 2, 50 ms on, 500 ms off.
const ESCPOS_DRAWER: [u8; 5] = [0x1B, b'p', 0, 25, 250];
/// `ESC ( A` with function 97 as Epson has it: pattern 1, once. There is no beep all printers share,
/// see FONTES.md. The command states its own length, so a printer that has another form can skip it.
const ESCPOS_BEEP: [u8; 8] = [0x1B, b'(', b'A', 3, 0, 97, 1, 1];

/// Appends what a printer command does.
fn push_command(receipt: &mut Vec<u8>, command: PrinterCommand) {
    match command.action {
        // GS V 65 n and 66 n feed to the cutting position and n further, then cut.
        PrinterAction::Cut => receipt.extend([0x1D, b'V', 65, ESCPOS_FEED]),
        PrinterAction::PartialCut => receipt.extend([0x1D, b'V', 66, ESCPOS_FEED]),
        PrinterAction::OpenDrawer => receipt.extend(ESCPOS_DRAWER),
        PrinterAction::Beep => receipt.extend(ESCPOS_BEEP),
        PrinterAction::Feed => receipt.extend([0x1B, b'd', command.lines]),
        _ => {}
    }
}

/// Appends rows in `GS v 0` blocks. `rows` holds whole rows and can be empty.
fn push_rows(receipt: &mut Vec<u8>, rows: &[u8], per_row: usize) {
    for block in rows.chunks(ESCPOS_BLOCK_ROWS * per_row) {
        let count = block.len() / per_row;
        receipt.extend([0x1D, b'v', b'0', 0]);
        receipt.extend([(per_row % 256) as u8, (per_row / 256) as u8]);
        receipt.extend([(count % 256) as u8, (count / 256) as u8]);
        receipt.extend_from_slice(block);
    }
}

/// The same picture on a width of whole bytes, with the spare dots split between the sides.
fn on_whole_bytes(bitmap: &Bitmap) -> Bitmap {
    let width = bitmap.bytes_per_row() as u32 * 8;
    let left = (width - bitmap.width()) / 2;
    let mut padded = Bitmap::new(width, bitmap.height());
    for y in 0..bitmap.height() {
        for x in 0..bitmap.width() {
            if bitmap.get(x, y) {
                padded.set(left + x, y, true);
            }
        }
    }
    padded
}

/// ESC/POS: `GS v 0` in normal mode, raw bytes, black is 1, centered with `ESC a`. A long receipt goes
/// in several blocks, which the printer joins with no gap. Each command goes between the rows above
/// and below it. The paper is cut at the end, unless the layout's last cut has nothing printed after it.
fn escpos(
    bitmap: &Bitmap,
    commands: &[(usize, PrinterCommand)],
    job: &LabelJob,
) -> Result<Vec<u8>, LabelError> {
    let per_row = bitmap.bytes_per_row();
    if per_row == 0 || bitmap.height() == 0 {
        return Err(LabelError::InvalidPage("o cupom não tem nenhum ponto".to_owned()));
    }
    if per_row > ESCPOS_MAX_ROW {
        return Err(LabelError::InvalidPage(format!(
            "{} pontos de largura; ESC/POS aceita até {} por linha",
            bitmap.width(),
            ESCPOS_MAX_ROW * 8
        )));
    }
    let padded;
    let whole = if bitmap.width().is_multiple_of(8) {
        bitmap
    } else {
        padded = on_whole_bytes(bitmap);
        &padded
    };
    // ESC @ then ESC a 1.
    let mut receipt = vec![0x1B, b'@', 0x1B, b'a', 1];
    let bits = whole.bits();
    let is_cut = |action| matches!(action, PrinterAction::Cut | PrinterAction::PartialCut);
    // Blank paper after the last cut would hang out of the printer, so it does not go.
    let finished = commands
        .iter()
        .rev()
        .find(|(_, command)| is_cut(command.action))
        .is_some_and(|(row, _)| bits[row * per_row..].iter().all(|byte| *byte == 0));
    let mut printed = 0;
    for (row, command) in commands {
        push_rows(&mut receipt, &bits[printed * per_row..row * per_row], per_row);
        push_command(&mut receipt, *command);
        printed = *row;
    }
    if !finished {
        push_rows(&mut receipt, &bits[printed * per_row..], per_row);
        receipt.extend([0x1D, b'V', 66, ESCPOS_FEED]);
    }
    Ok(receipt.repeat(job.copies as usize))
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
    fn every_language_is_found_by_its_names() {
        assert_eq!(Language::all().len(), 5);
        for language in Language::all() {
            assert_eq!(Language::from_name(language.name()), Some(*language));
        }
        for (name, language) in [
            ("ESC/POS", Language::EscPos),
            ("ZPL", Language::Zpl),
            ("ZPL II", Language::Zpl),
            ("EPL", Language::Epl),
            ("EPL2/PPLB", Language::Epl),
            ("PPLB", Language::Epl),
            ("PPLA", Language::Ppla),
            ("TSPL", Language::Tspl),
        ] {
            assert_eq!(Language::from_name(name), Some(language), "{name}");
        }
        assert_eq!(Language::from_name(" zpl "), Some(Language::Zpl));
        assert_eq!(Language::from_name("DPL"), None);
        assert_eq!(Language::EscPos.name(), "ESC/POS");
    }

    #[test]
    fn escpos_bytes_have_black_as_one_and_cut_at_the_end() {
        let job = LabelJob::new(Language::EscPos, 203);
        let bytes = encode_bitmap(&small(), &job).unwrap_or_else(|e| panic!("{e}"));
        let expected = [
            0x1B, b'@', 0x1B, b'a', 1, 0x1D, b'v', b'0', 0, 2, 0, 2, 0, 0xF0, 0x00, 0x00, 0x0F, 0x1D, b'V',
            66, 24,
        ];
        assert_eq!(bytes, expected);
    }

    #[test]
    fn escpos_centers_a_picture_on_whole_bytes() {
        // 12 dots become 16, with 2 spare dots on each side.
        let mut bitmap = Bitmap::new(12, 1);
        bitmap.fill(0, 0, 12, 1, true);
        let job = LabelJob::new(Language::EscPos, 203);
        let bytes = encode_bitmap(&bitmap, &job).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(bytes[9..15], [2, 0, 1, 0, 0x3F, 0xFC]);
    }

    #[test]
    fn escpos_takes_its_own_resolutions_and_copies_repeat_the_receipt() {
        let check = |dpi, copies| {
            let job = LabelJob { copies, ..LabelJob::new(Language::EscPos, dpi) };
            encode_bitmap(&small(), &job)
        };
        let one = check(203, 1).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(check(180, 1), Ok(one.clone()));
        assert_eq!(check(203, 2), Ok(one.repeat(2)));
        assert!(matches!(check(300, 1), Err(LabelError::InvalidJob(_))));
        assert!(matches!(check(203, 100), Err(LabelError::InvalidJob(_))));
        let at_180 = LabelJob::new(Language::Zpl, 180);
        assert!(matches!(encode_bitmap(&small(), &at_180), Err(LabelError::InvalidJob(_))));
    }

    #[test]
    fn a_row_too_wide_for_escpos_and_an_empty_receipt_are_refused() {
        let job = LabelJob::new(Language::EscPos, 203);
        assert!(matches!(encode_bitmap(&Bitmap::new(2048, 1), &job), Err(LabelError::InvalidPage(_))));
        assert!(matches!(encode_bitmap(&Bitmap::new(0, 0), &job), Err(LabelError::InvalidPage(_))));
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
