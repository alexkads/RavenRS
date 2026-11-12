//! PDF receipts: their text, and the total in it.

use std::fs::{File, remove_file};
use std::io::Write;
use std::process::Command;

pub(super) const MAX_PDF_BYTES: u64 = 14 * 1024 * 1024;

/// Finds the one total in a receipt. The text is read in Rust, the same on every system;
/// when that text has no single total and `pdftotext` happens to be installed, its
/// reading gets a second chance.
#[cfg(test)]
pub(super) fn extract_total_from_pdf(pdf: &[u8]) -> Result<i64, String> {
    match total_in_pdf(pdf)? {
        Total::One(cents) => Ok(cents),
        Total::Several(_) => Err("Encontrei mais de um valor total no PDF.".into()),
    }
}

/// What a receipt says its total is.
pub(super) enum Total {
    One(i64),
    /// The amounts it could be, for the person to choose: never guessed.
    Several(Vec<i64>),
}

/// The most amounts offered as buttons when a receipt has several.
const TOTAL_CHOICES: usize = 6;

/// The receipt's total, read by us or, when that finds no single one, by `pdftotext`.
pub(super) fn total_in_pdf(pdf: &[u8]) -> Result<Total, String> {
    let ours = text_in_pdf(pdf).map(|text| totals_in_text(&text));
    if let Some([one]) = ours.as_deref() {
        return Ok(Total::One(*one));
    }
    let theirs = total_with_pdftotext(pdf);
    if let Some([one]) = theirs.as_deref() {
        return Ok(Total::One(*one));
    }
    let Some(mut values) = ours.filter(|v| !v.is_empty()).or(theirs.filter(|v| !v.is_empty())) else {
        return Err(if text_in_pdf(pdf).is_none() {
            "o PDF não tem texto extraível. Diga o valor: `gastei 22,74 no mercado`.".into()
        } else {
            "Não encontrei um valor total legível no PDF. Diga o valor: `gastei 22,74 no mercado`.".into()
        });
    };
    values.dedup();
    values.truncate(TOTAL_CHOICES);
    Ok(Total::Several(values))
}

/// The PDF's text. A malformed file can make the reader panic; that is no text.
pub(super) fn text_in_pdf(pdf: &[u8]) -> Option<String> {
    std::panic::catch_unwind(|| pdf_extract::extract_text_from_mem(pdf).ok()).ok().flatten()
}

/// The total as `pdftotext` reads it, or `None` when it is not installed or fails.
pub(super) fn total_with_pdftotext(pdf: &[u8]) -> Option<Vec<i64>> {
    let nonce = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).ok()?.as_nanos();
    let path = std::env::temp_dir().join(format!("raven-financas-{}-{nonce}.pdf", std::process::id()));
    let written =
        File::options().create_new(true).write(true).open(&path).and_then(|mut file| file.write_all(pdf));
    let output =
        written.is_ok().then(|| Command::new("pdftotext").arg("-layout").arg(&path).arg("-").output());
    let _ = remove_file(&path);
    let output = output?.ok().filter(|output| output.status.success())?;
    Some(totals_in_text(&String::from_utf8(output.stdout).ok()?))
}

/// The amounts that could be the total, sorted: on a total's line, or else after "R$".
pub(super) fn totals_in_text(text: &str) -> Vec<i64> {
    let mut totals = text.lines().filter(|line| is_total_line(line)).flat_map(amounts_in).collect::<Vec<_>>();
    if totals.is_empty() {
        totals = text.lines().flat_map(amounts_after_currency).collect();
    }
    totals.sort_unstable();
    totals
}

#[cfg(test)]
pub(super) fn extract_total_from_text(text: &str) -> Result<i64, String> {
    match totals_in_text(text).as_slice() {
        [amount] => Ok(*amount),
        [] => Err("Não encontrei um valor total legível no PDF. Envie outro comprovante.".into()),
        _ => Err("Encontrei mais de um valor total no PDF. Envie outro comprovante.".into()),
    }
}

pub(super) fn is_total_line(line: &str) -> bool {
    let line = folded(line);
    !line.contains("tribut")
        && ["valor total", "valor a pagar", "total a pagar", "valor pago"]
            .iter()
            .any(|label| line.contains(label))
}

pub(super) fn amounts_in(text: &str) -> Vec<i64> {
    let mut values = Vec::new();
    let mut token = String::new();
    for character in text.chars().chain(std::iter::once(' ')) {
        if character.is_ascii_digit() || character == '.' || character == ',' {
            token.push(character);
        } else {
            if let Some(value) = cents_in(&token) {
                values.push(value);
            }
            token.clear();
        }
    }
    values
}

pub(super) fn amounts_after_currency(line: &str) -> Vec<i64> {
    line.split("R$").flat_map(|part| amounts_in(part).into_iter().take(1)).collect()
}

pub(super) fn cents_in(token: &str) -> Option<i64> {
    let token = token.trim_matches(|c| c == '.' || c == ',');
    let (whole, cents) = token.rsplit_once(',')?;
    if cents.len() != 2 || !cents.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let whole = whole.chars().filter(char::is_ascii_digit).collect::<String>();
    let whole = whole.parse::<i64>().ok()?;
    let cents = cents.parse::<i64>().ok()?;
    let value = whole.checked_mul(100)?.checked_add(cents)?;
    (value > 0 && value <= 100_000_000).then_some(value)
}

pub(super) fn folded(text: &str) -> String {
    text.chars()
        .flat_map(char::to_lowercase)
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ã' | 'ä' | 'å' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
            'ú' | 'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            other => other,
        })
        .collect()
}
