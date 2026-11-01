//! Turns a report's values and .NET-style format specifiers into what a
//! spreadsheet stores: a number and an Excel number format.

use ravenrs_report::{Culture, DateTime, Value};

/// Days between Excel's day zero (in its 1900 system) and 1970-01-01.
const EXCEL_EPOCH: i64 = 25_569;

/// The number to store for `value` and its Excel format, or `None` when the
/// cell should keep the printed text.
pub(crate) fn typed(value: &Value, spec: Option<&str>, culture: &Culture) -> Option<(f64, Option<String>)> {
    let spec = spec.unwrap_or_default().trim();
    match value {
        Value::Number(n) if n.is_finite() => Some((*n, Some(number_format(spec, culture)?))),
        Value::Date(d) => date(*d, spec, culture),
        // Text takes number and date formats too, as the engine prints it that way.
        Value::Text(s) if !spec.is_empty() => match DateTime::parse(s) {
            Some(d) => date(d, spec, culture),
            None => {
                let n = s.trim().parse::<f64>().ok().filter(|n| n.is_finite())?;
                Some((n, Some(number_format(spec, culture)?)))
            }
        },
        _ => None,
    }
}

fn date(d: DateTime, spec: &str, culture: &Culture) -> Option<(f64, Option<String>)> {
    // Before March 1900, Excel's calendar disagrees with the real one.
    let day = d.days + EXCEL_EPOCH;
    if day < 61 {
        return None;
    }
    let serial = day as f64 + f64::from(d.seconds) / 86_400.0;
    Some((serial, Some(date_format(spec, culture)?)))
}

/// The Excel format for a number specifier; `None` when Excel has no equivalent.
fn number_format(spec: &str, culture: &Culture) -> Option<String> {
    if spec.is_empty() || spec.eq_ignore_ascii_case("g") {
        return Some("General".to_owned());
    }
    let mut chars = spec.chars();
    let kind = chars.next()?;
    let rest = chars.as_str();
    if rest.is_empty() || rest.parse::<usize>().is_ok() {
        let digits = rest.parse::<usize>().ok();
        let decimals = |default: usize| {
            let n = digits.unwrap_or(default);
            if n == 0 { String::new() } else { format!(".{}", "0".repeat(n)) }
        };
        match kind.to_ascii_uppercase() {
            'C' => {
                let space = if culture.currency_space { "\\ " } else { "" };
                return Some(format!("\"{}\"{space}#,##0{}", culture.currency, decimals(2)));
            }
            'N' => return Some(format!("#,##0{}", decimals(2))),
            'F' => return Some(format!("0{}", decimals(2))),
            'P' => return Some(format!("0{}%", decimals(2))),
            'D' => return Some("0".repeat(digits.unwrap_or(1).max(1))),
            'X' => return None,
            _ => {}
        }
    }
    let sections: Vec<String> = spec.split(';').map(custom_number_section).collect::<Option<_>>()?;
    Some(sections.join(";"))
}

/// A custom section such as `R$ #,##0.00`, with the literal text around the digits quoted.
fn custom_number_section(section: &str) -> Option<String> {
    let is_digit = |c: char| matches!(c, '0' | '#' | ',' | '.');
    let start = section.find(is_digit)?;
    let end = section.rfind(is_digit)? + 1;
    let literal = |s: &str| {
        let mut out = String::new();
        let plain = s.replace(['\'', '"'], "");
        let without_percent = plain.replace('%', "");
        if !without_percent.is_empty() {
            out = format!("\"{without_percent}\"");
        }
        if plain.contains('%') {
            out.push('%');
        }
        out
    };
    Some(format!("{}{}{}", literal(&section[..start]), &section[start..end], literal(&section[end..])))
}

/// The Excel format for a date specifier; `None` when Excel has no equivalent.
fn date_format(spec: &str, culture: &Culture) -> Option<String> {
    let pattern = match spec {
        "" | "d" => culture.short_date.to_owned(),
        "D" => culture.long_date.to_owned(),
        "t" => culture.short_time.to_owned(),
        "T" => format!("{}:ss", culture.short_time),
        "g" => format!("{} {}", culture.short_date, culture.short_time),
        "G" => format!("{} {}:ss", culture.short_date, culture.short_time),
        "f" => format!("{} {}", culture.long_date, culture.short_time),
        "F" => format!("{} {}:ss", culture.long_date, culture.short_time),
        "M" | "m" | "Y" | "y" => return None,
        custom => custom.to_owned(),
    };
    // Month and day names follow the report's language, whatever the machine's is.
    let locale = if culture.name == "pt-BR" { "[$-416]" } else { "" };
    Some(format!("{locale}{}", translate_date(&pattern)))
}

/// Translates a .NET date pattern to Excel's letters.
fn translate_date(pattern: &str) -> String {
    let mut out = String::new();
    let mut chars = pattern.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\'' | '"' => {
                let literal: String = chars.by_ref().take_while(|x| *x != c).collect();
                out.push('"');
                out.push_str(&literal);
                out.push('"');
            }
            'M' => out.push('m'),
            'H' => out.push('h'),
            'y' | 'd' | 'h' | 'm' | 's' => out.push(c),
            't' => {
                while chars.peek() == Some(&'t') {
                    chars.next();
                }
                out.push_str("AM/PM");
            }
            '/' | ':' | '-' | ' ' | ',' | '.' => out.push(c),
            other => {
                out.push('"');
                out.push(other);
                out.push('"');
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn number(spec: &str) -> Option<String> {
        number_format(spec, &Culture::pt_br())
    }

    #[test]
    fn standard_number_formats() {
        assert_eq!(number("C2").as_deref(), Some("\"R$\"\\ #,##0.00"));
        assert_eq!(number("N0").as_deref(), Some("#,##0"));
        assert_eq!(number("N").as_deref(), Some("#,##0.00"));
        assert_eq!(number("F1").as_deref(), Some("0.0"));
        assert_eq!(number("P1").as_deref(), Some("0.0%"));
        assert_eq!(number("D4").as_deref(), Some("0000"));
        assert_eq!(number("").as_deref(), Some("General"));
        assert_eq!(number("X2"), None);
        assert_eq!(number_format("C2", &Culture::en_us()).as_deref(), Some("\"$\"#,##0.00"));
    }

    #[test]
    fn custom_number_formats_quote_their_literals() {
        assert_eq!(number("#,##0.00").as_deref(), Some("#,##0.00"));
        assert_eq!(number("R$ #,##0.00").as_deref(), Some("\"R$ \"#,##0.00"));
        assert_eq!(number("0.0%").as_deref(), Some("0.0%"));
        assert_eq!(number("#,##0.00;(#,##0.00)").as_deref(), Some("#,##0.00;\"(\"#,##0.00\")\""));
        assert_eq!(number("abc"), None);
    }

    #[test]
    fn date_formats_translate_letters_and_literals() {
        let br = Culture::pt_br();
        assert_eq!(date_format("dd/MM/yyyy", &br).as_deref(), Some("[$-416]dd/mm/yyyy"));
        assert_eq!(date_format("d", &br).as_deref(), Some("[$-416]dd/mm/yyyy"));
        assert_eq!(date_format("g", &br).as_deref(), Some("[$-416]dd/mm/yyyy hh:mm"));
        assert_eq!(date_format("D", &br).as_deref(), Some("[$-416]dddd, d \"de\" mmmm \"de\" yyyy"));
        assert_eq!(date_format("h:mm tt", &Culture::en_us()).as_deref(), Some("h:mm AM/PM"));
        assert_eq!(date_format("M", &br), None);
    }

    #[test]
    fn dates_become_excel_serial_numbers() {
        let br = Culture::pt_br();
        let date = DateTime::from_ymd(2026, 11, 1).map(Value::Date).unwrap_or_default();
        // 2026-11-01 is serial 46327 in Excel.
        assert_eq!(typed(&date, Some("dd/MM/yyyy"), &br).map(|t| t.0), Some(46_327.0));
        let early = DateTime::from_ymd(1899, 1, 1).map(Value::Date).unwrap_or_default();
        assert_eq!(typed(&early, None, &br), None, "before 1900 stays text");
    }

    #[test]
    fn text_is_typed_only_with_a_format() {
        let br = Culture::pt_br();
        assert_eq!(typed(&Value::Text("12.5".into()), Some("N2"), &br).map(|t| t.0), Some(12.5));
        assert_eq!(typed(&Value::Text("12.5".into()), None, &br), None);
        assert_eq!(typed(&Value::Text("abc".into()), Some("N2"), &br), None);
        assert_eq!(typed(&Value::Bool(true), None, &br), None);
    }
}
