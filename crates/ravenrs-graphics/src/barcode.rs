//! Barcodes: turns text into the modules a scanner reads. Pure logic, so every
//! output draws the same code and it can be tested without drawing.

use std::fmt::Write as _;

use ravenrs_core::{Color, Rect};
use serde::{Deserialize, Serialize};

use crate::vector::{Shape, append_rectangle};

/// A barcode format.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub enum Symbology {
    /// Code 128: any printable ASCII text, compact for digits.
    #[default]
    Code128,
    /// Code 39: capitals, digits and `-. $/+%`.
    Code39,
    /// EAN-13: 12 digits, or 13 with the check digit.
    Ean13,
    /// EAN-8: 7 digits, or 8 with the check digit.
    Ean8,
    /// UPC-A: 11 digits, or 12 with the check digit.
    UpcA,
    /// Interleaved 2 of 5: an even number of digits.
    Itf,
    /// QR Code: any text.
    Qr,
}

/// What a scanner reads.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Pattern {
    /// Modules from left to right; `true` is a bar.
    Linear {
        /// The modules.
        modules: Vec<bool>,
        /// The text printed under the bars, check digits included.
        text: String,
    },
    /// Rows of square modules; `true` is dark.
    Matrix {
        /// How many modules on each side.
        size: usize,
        /// The modules, row by row.
        cells: Vec<bool>,
    },
}

/// Empty modules on each side of a linear code, which scanners need.
const QUIET_LINEAR: f32 = 10.0;
/// Empty modules around a QR Code, as its standard asks.
const QUIET_MATRIX: f32 = 4.0;

/// Encodes `data`, or says why it does not fit the format.
pub fn encode(symbology: Symbology, data: &str) -> Result<Pattern, String> {
    match symbology {
        Symbology::Code128 => code128(data),
        Symbology::Code39 => code39(data),
        Symbology::Ean13 => ean13(data),
        Symbology::Ean8 => ean8(data),
        Symbology::UpcA => upc_a(data),
        Symbology::Itf => itf(data),
        Symbology::Qr => qr(data),
    }
}

/// The shape of a pattern inside `rect`, centered and with its quiet zone.
pub fn shape(pattern: &Pattern, rect: Rect, color: Color) -> Shape {
    let mut path = Vec::new();
    match pattern {
        Pattern::Linear { modules, .. } => {
            let module = rect.width() / (modules.len() as f32 + 2.0 * QUIET_LINEAR);
            let left = rect.x() + QUIET_LINEAR * module;
            for (start, length) in runs(modules) {
                let x = left + start as f32 * module;
                append_rectangle(&mut path, x, rect.y(), x + length as f32 * module, rect.max_y(), 0.0);
            }
        }
        Pattern::Matrix { size, cells } => {
            let module = rect.width().min(rect.height()) / (*size as f32 + 2.0 * QUIET_MATRIX);
            let side = *size as f32 * module;
            let (left, top) =
                (rect.x() + (rect.width() - side) / 2.0, rect.y() + (rect.height() - side) / 2.0);
            for (row, cells) in cells.chunks((*size).max(1)).enumerate() {
                let y = top + row as f32 * module;
                for (start, length) in runs(cells) {
                    let x = left + start as f32 * module;
                    append_rectangle(&mut path, x, y, x + length as f32 * module, y + module, 0.0);
                }
            }
        }
    }
    Shape::filled(path, color)
}

/// The start and length of each run of `true`.
fn runs(modules: &[bool]) -> impl Iterator<Item = (usize, usize)> + '_ {
    let mut at = 0;
    std::iter::from_fn(move || {
        while modules.get(at) == Some(&false) {
            at += 1;
        }
        let start = at;
        while modules.get(at) == Some(&true) {
            at += 1;
        }
        (at > start).then_some((start, at - start))
    })
}

/// Appends alternating bars and spaces, given as widths in modules, bar first.
fn push_widths(modules: &mut Vec<bool>, widths: &str) {
    for (index, width) in widths.bytes().enumerate() {
        modules.extend(std::iter::repeat_n(index % 2 == 0, usize::from(width - b'0')));
    }
}

/// Bar and space widths of each Code 128 symbol, by value; 106 is the stop.
const CODE128: [&str; 107] = [
    "212222", "222122", "222221", "121223", "121322", "131222", "122213", "122312", "132212", "221213",
    "221312", "231212", "112232", "122132", "122231", "113222", "123122", "123221", "223211", "221132",
    "221231", "213212", "223112", "312131", "311222", "321122", "321221", "312212", "322112", "322211",
    "212123", "212321", "232121", "111323", "131123", "131321", "112313", "132113", "132311", "211313",
    "231113", "231311", "112133", "112331", "132131", "113123", "113321", "133121", "313121", "211331",
    "231131", "213113", "213311", "213131", "311123", "311321", "331121", "312113", "312311", "332111",
    "314111", "221411", "431111", "111224", "111422", "121124", "121421", "141122", "141221", "112214",
    "112412", "122114", "122411", "142112", "142211", "241211", "221114", "413111", "241112", "134111",
    "111242", "121142", "121241", "114212", "124112", "124211", "411212", "421112", "421211", "212141",
    "214121", "412121", "111143", "111341", "131141", "114113", "114311", "411113", "411311", "113141",
    "114131", "311141", "411131", "211412", "211214", "211232", "2331112",
];
const CODE128_START_B: usize = 104;
const CODE128_START_C: usize = 105;
const CODE128_TO_C: usize = 99;
const CODE128_TO_B: usize = 100;
const CODE128_STOP: usize = 106;
/// A run of this many digits is shorter in subset C even with the switches.
const CODE128_DIGITS_FOR_C: usize = 6;

fn code128(data: &str) -> Result<Pattern, String> {
    if data.is_empty() {
        return Err("Code 128 needs some text".to_owned());
    }
    let bytes = data.as_bytes();
    if !bytes.iter().all(|b| (32..=126).contains(b)) {
        return Err("Code 128 takes printable ASCII text".to_owned());
    }
    let digits_at = |at: usize| bytes[at..].iter().take_while(|b| b.is_ascii_digit()).count();
    let mut symbols = Vec::new();
    let mut in_c = digits_at(0) >= 4 && digits_at(0) % 2 == 0;
    symbols.push(if in_c { CODE128_START_C } else { CODE128_START_B });
    let mut at = 0;
    while at < bytes.len() {
        let digits = digits_at(at);
        if in_c {
            if digits >= 2 {
                symbols.push(usize::from((bytes[at] - b'0') * 10 + (bytes[at + 1] - b'0')));
                at += 2;
                continue;
            }
            symbols.push(CODE128_TO_B);
            in_c = false;
        } else if digits >= CODE128_DIGITS_FOR_C {
            // An odd digit goes first in B, so the pairs finish the run exactly.
            if digits % 2 == 1 {
                symbols.push(usize::from(bytes[at] - 32));
                at += 1;
            }
            symbols.push(CODE128_TO_C);
            in_c = true;
            continue;
        }
        symbols.push(usize::from(bytes[at] - 32));
        at += 1;
    }
    let check = symbols.iter().enumerate().map(|(i, s)| s * i.max(1)).sum::<usize>() % 103;
    symbols.push(check);
    symbols.push(CODE128_STOP);
    let mut modules = Vec::new();
    for symbol in symbols {
        push_widths(&mut modules, CODE128[symbol]);
    }
    Ok(Pattern::Linear { modules, text: data.to_owned() })
}

const CODE39_CHARS: &str = "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ-. $/+%*";
/// Narrow and wide elements of each character above: bar, space, bar, and so on.
const CODE39: [&str; 44] = [
    "nnnwwnwnn",
    "wnnwnnnnw",
    "nnwwnnnnw",
    "wnwwnnnnn",
    "nnnwwnnnw",
    "wnnwwnnnn",
    "nnwwwnnnn",
    "nnnwnnwnw",
    "wnnwnnwnn",
    "nnwwnnwnn",
    "wnnnnwnnw",
    "nnwnnwnnw",
    "wnwnnwnnn",
    "nnnnwwnnw",
    "wnnnwwnnn",
    "nnwnwwnnn",
    "nnnnnwwnw",
    "wnnnnwwnn",
    "nnwnnwwnn",
    "nnnnwwwnn",
    "wnnnnnnww",
    "nnwnnnnww",
    "wnwnnnnwn",
    "nnnnwnnww",
    "wnnnwnnwn",
    "nnwnwnnwn",
    "nnnnnnwww",
    "wnnnnnwwn",
    "nnwnnnwwn",
    "nnnnwnwwn",
    "wwnnnnnnw",
    "nwwnnnnnw",
    "wwwnnnnnn",
    "nwnnwnnnw",
    "wwnnwnnnn",
    "nwwnwnnnn",
    "nwnnnnwnw",
    "wwnnnnwnn",
    "nwwnnnwnn",
    "nwnwnwnnn",
    "nwnwnnnwn",
    "nwnnnwnwn",
    "nnnwnwnwn",
    "nwnnwnwnn",
];
/// How much wider a wide element is.
const CODE39_WIDE: usize = 3;

fn code39(data: &str) -> Result<Pattern, String> {
    if data.is_empty() {
        return Err("Code 39 needs some text".to_owned());
    }
    let upper = data.to_ascii_uppercase();
    if let Some(bad) = upper.chars().find(|c| *c == '*' || !CODE39_CHARS.contains(*c)) {
        return Err(format!("Code 39 does not have '{bad}'"));
    }
    let mut modules = Vec::new();
    for c in std::iter::once('*').chain(upper.chars()).chain(std::iter::once('*')) {
        let index = CODE39_CHARS.find(c).unwrap_or(0);
        for (element, kind) in CODE39[index].bytes().enumerate() {
            let width = if kind == b'w' { CODE39_WIDE } else { 1 };
            modules.extend(std::iter::repeat_n(element % 2 == 0, width));
        }
        // The narrow gap between characters.
        modules.push(false);
    }
    modules.pop();
    Ok(Pattern::Linear { modules, text: upper })
}

/// The check digit of a GS1 number: weights 3 and 1 from the right.
fn gs1_check(digits: &[u8]) -> u8 {
    let sum: u32 =
        digits.iter().rev().enumerate().map(|(i, d)| u32::from(*d) * if i % 2 == 0 { 3 } else { 1 }).sum();
    ((10 - sum % 10) % 10) as u8
}

/// The digits of a GS1 number of `length`, adding the check digit or verifying it.
fn gs1_digits(name: &str, data: &str, length: usize) -> Result<Vec<u8>, String> {
    if !data.bytes().all(|b| b.is_ascii_digit()) {
        return Err(format!("{name} takes only digits"));
    }
    let mut digits: Vec<u8> = data.bytes().map(|b| b - b'0').collect();
    if digits.len() == length - 1 {
        digits.push(gs1_check(&digits));
    } else if digits.len() != length {
        return Err(format!("{name} takes {} or {length} digits", length - 1));
    } else if gs1_check(&digits[..length - 1]) != digits[length - 1] {
        return Err(format!("{name}: the check digit of {data} is wrong"));
    }
    Ok(digits)
}

/// Widths of each digit's symbol in the odd-parity left set; the right set has
/// the same widths, and the even-parity set the same widths reversed.
const EAN_L: [&str; 10] = ["3211", "2221", "2122", "1411", "1132", "1231", "1114", "1312", "1213", "3112"];
/// Which left digits use the even-parity set, by the first digit of an EAN-13.
const EAN13_PARITY: [&str; 10] =
    ["LLLLLL", "LLGLGG", "LLGGLG", "LLGGGL", "LGLLGG", "LGGLLG", "LGGGLL", "LGLGLG", "LGLGGL", "LGGLGL"];

fn push_symbol(modules: &mut Vec<bool>, widths: &str, starts_dark: bool, reversed: bool) {
    let mut ordered: Vec<u8> = widths.bytes().collect();
    if reversed {
        ordered.reverse();
    }
    for (index, width) in ordered.iter().enumerate() {
        modules.extend(std::iter::repeat_n((index % 2 == 0) == starts_dark, usize::from(width - b'0')));
    }
}

fn ean_modules(left: &[u8], parity: &str, right: &[u8]) -> Vec<bool> {
    let mut modules = vec![true, false, true];
    for (digit, set) in left.iter().zip(parity.bytes()) {
        // Left symbols start with a space.
        push_symbol(&mut modules, EAN_L[usize::from(*digit)], false, set == b'G');
    }
    modules.extend([false, true, false, true, false]);
    for digit in right {
        // Right symbols start with a bar.
        push_symbol(&mut modules, EAN_L[usize::from(*digit)], true, false);
    }
    modules.extend([true, false, true]);
    modules
}

fn digits_text(digits: &[u8]) -> String {
    let mut text = String::with_capacity(digits.len());
    for digit in digits {
        let _ = write!(text, "{digit}");
    }
    text
}

fn ean13(data: &str) -> Result<Pattern, String> {
    let digits = gs1_digits("EAN-13", data, 13)?;
    let modules = ean_modules(&digits[1..7], EAN13_PARITY[usize::from(digits[0])], &digits[7..]);
    Ok(Pattern::Linear { modules, text: digits_text(&digits) })
}

fn ean8(data: &str) -> Result<Pattern, String> {
    let digits = gs1_digits("EAN-8", data, 8)?;
    let modules = ean_modules(&digits[..4], "LLLL", &digits[4..]);
    Ok(Pattern::Linear { modules, text: digits_text(&digits) })
}

fn upc_a(data: &str) -> Result<Pattern, String> {
    let digits = gs1_digits("UPC-A", data, 12)?;
    let modules = ean_modules(&digits[..6], "LLLLLL", &digits[6..]);
    Ok(Pattern::Linear { modules, text: digits_text(&digits) })
}

/// Narrow and wide bars of each digit in Interleaved 2 of 5.
const ITF: [&str; 10] =
    ["nnwwn", "wnnnw", "nwnnw", "wwnnn", "nnwnw", "wnwnn", "nwwnn", "nnnww", "wnnwn", "nwnwn"];
const ITF_WIDE: usize = 3;

fn itf(data: &str) -> Result<Pattern, String> {
    if data.is_empty() || !data.bytes().all(|b| b.is_ascii_digit()) {
        return Err("Interleaved 2 of 5 takes only digits".to_owned());
    }
    if !data.len().is_multiple_of(2) {
        return Err("Interleaved 2 of 5 takes an even number of digits".to_owned());
    }
    let mut modules = vec![true, false, true, false];
    let width = |kind: u8| if kind == b'w' { ITF_WIDE } else { 1 };
    for pair in data.as_bytes().chunks(2) {
        let bars = ITF[usize::from(pair[0] - b'0')].as_bytes();
        let spaces = ITF[usize::from(pair[1] - b'0')].as_bytes();
        for (bar, space) in bars.iter().zip(spaces) {
            modules.extend(std::iter::repeat_n(true, width(*bar)));
            modules.extend(std::iter::repeat_n(false, width(*space)));
        }
    }
    modules.extend(std::iter::repeat_n(true, ITF_WIDE));
    modules.extend([false, true]);
    Ok(Pattern::Linear { modules, text: data.to_owned() })
}

fn qr(data: &str) -> Result<Pattern, String> {
    if data.is_empty() {
        return Err("a QR Code needs some text".to_owned());
    }
    let code = qrcodegen::QrCode::encode_text(data, qrcodegen::QrCodeEcc::Medium)
        .map_err(|_| "the text is too long for a QR Code".to_owned())?;
    let size = code.size();
    let cells =
        (0..size).flat_map(|y| (0..size).map(move |x| (x, y))).map(|(x, y)| code.get_module(x, y)).collect();
    Ok(Pattern::Matrix { size: size as usize, cells })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vector::PathSegment;

    fn modules(symbology: Symbology, data: &str) -> Vec<bool> {
        match encode(symbology, data) {
            Ok(Pattern::Linear { modules, .. }) => modules,
            other => panic!("{other:?}"),
        }
    }

    fn text(symbology: Symbology, data: &str) -> String {
        match encode(symbology, data) {
            Ok(Pattern::Linear { text, .. }) => text,
            other => panic!("{other:?}"),
        }
    }

    fn bits(modules: &[bool]) -> String {
        modules.iter().map(|m| if *m { '1' } else { '0' }).collect()
    }

    #[test]
    fn the_code128_table_is_consistent() {
        let mut seen = std::collections::HashSet::new();
        for (value, widths) in CODE128.iter().enumerate() {
            let total: u32 = widths.bytes().map(|b| u32::from(b - b'0')).sum();
            assert_eq!(total, if value == CODE128_STOP { 13 } else { 11 }, "{value}");
            assert!(seen.insert(widths), "{value} is repeated");
        }
    }

    #[test]
    fn code128_has_the_known_check_symbol() {
        // 104 + 48*1 + 42*2 + 42*3 + 17*4 + 18*5 + 19*6 + 35*7 = 879, and 879 mod 103 = 55.
        let symbols_end = modules(Symbology::Code128, "PJJ123C");
        let expected: String = [CODE128_START_B, 48, 42, 42, 17, 18, 19, 35, 55, CODE128_STOP]
            .iter()
            .flat_map(|s| {
                let mut part = Vec::new();
                push_widths(&mut part, CODE128[*s]);
                part
            })
            .map(|m| if m { '1' } else { '0' })
            .collect();
        assert_eq!(bits(&symbols_end), expected);
    }

    #[test]
    fn code128_uses_pairs_for_digits() {
        assert!(
            modules(Symbology::Code128, "12345678").len() < modules(Symbology::Code128, "1234567A").len()
        );
        // Odd, mixed and long runs all encode without leaving digits behind.
        for data in ["1", "12", "123", "1234", "12345", "A123456", "A1234567", "1234567", "12A34", "123456A"]
        {
            assert!(encode(Symbology::Code128, data).is_ok(), "{data}");
        }
    }

    #[test]
    fn check_digits_are_added_or_verified() {
        assert_eq!(text(Symbology::Ean13, "400638133393"), "4006381333931");
        assert_eq!(text(Symbology::Ean13, "4006381333931"), "4006381333931");
        assert!(encode(Symbology::Ean13, "4006381333932").is_err());
        assert_eq!(text(Symbology::Ean8, "9638507"), "96385074");
        assert_eq!(text(Symbology::UpcA, "03600029145"), "036000291452");
        assert!(encode(Symbology::Ean13, "12345").is_err());
        assert!(encode(Symbology::Ean13, "abcdefghijkl").is_err());
    }

    #[test]
    fn gs1_codes_have_their_standard_lengths() {
        assert_eq!(modules(Symbology::Ean13, "4006381333931").len(), 95);
        assert_eq!(modules(Symbology::Ean8, "96385074").len(), 67);
        assert_eq!(modules(Symbology::UpcA, "036000291452").len(), 95);
    }

    #[test]
    fn code39_refuses_what_it_does_not_have() {
        assert_eq!(text(Symbology::Code39, "abc-1"), "ABC-1");
        assert!(encode(Symbology::Code39, "a*b").is_err());
        assert!(encode(Symbology::Code39, "ç").is_err());
        assert!(encode(Symbology::Code39, "").is_err());
    }

    #[test]
    fn itf_needs_an_even_number_of_digits() {
        assert!(encode(Symbology::Itf, "1234").is_ok());
        assert!(encode(Symbology::Itf, "123").is_err());
        assert!(encode(Symbology::Itf, "12a4").is_err());
    }

    #[test]
    fn qr_codes_are_square_with_finder_patterns() {
        let Ok(Pattern::Matrix { size, cells }) = encode(Symbology::Qr, "https://example.com") else {
            panic!()
        };
        assert_eq!(cells.len(), size * size);
        assert_eq!((size - 21) % 4, 0, "a valid version");
        assert!(cells[..7].iter().all(|c| *c), "top of the finder pattern");
        assert!(encode(Symbology::Qr, "").is_err());
        assert!(encode(Symbology::Qr, &"x".repeat(5000)).is_err());
    }

    #[test]
    fn shapes_stay_inside_their_box_and_keep_the_quiet_zone() {
        let rect = Rect::new(10.0, 20.0, 200.0, 50.0);
        for (symbology, data) in
            [(Symbology::Code128, "ABC123"), (Symbology::Qr, "x"), (Symbology::Ean13, "400638133393")]
        {
            let pattern = encode(symbology, data).map_err(|e| e.to_string());
            let drawn = shape(&pattern.unwrap_or_else(|e| panic!("{e}")), rect, Color::BLACK);
            assert!(!drawn.path.is_empty());
            for segment in &drawn.path {
                if let PathSegment::Move(p) | PathSegment::Line(p) = segment {
                    assert!(p.x > rect.x() && p.x < rect.max_x() + 0.001, "{p:?}");
                    assert!(p.y >= rect.y() - 0.001 && p.y <= rect.max_y() + 0.001, "{p:?}");
                }
            }
        }
    }
}
