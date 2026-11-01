//! Money as whole cents, so totals never pick up floating point errors.

/// Writes cents as Brazilian currency: `R$ 1.234,56`.
pub fn format_brl(cents: i64) -> String {
    let abs = cents.unsigned_abs();
    let digits = (abs / 100).to_string();
    let mut grouped = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            grouped.push('.');
        }
        grouped.push(c);
    }
    let sign = if cents < 0 { "-" } else { "" };
    format!("{sign}R$ {grouped},{:02}", abs % 100)
}

/// Writes cents the way a person types them: `1234,56`.
pub fn format_input(cents: i64) -> String {
    format!("{},{:02}", cents / 100, (cents % 100).abs())
}

/// Reads an amount typed as `1.234,56`, `1234.5`, `R$ 12` or `12,`.
///
/// The last separator is the decimal one when one or two digits follow it;
/// with three digits after it, it separates thousands.
pub fn parse_brl(text: &str) -> Result<i64, String> {
    let invalid = || format!("\"{}\" não é um valor válido. Use, por exemplo, 1.234,56.", text.trim());
    let cleaned: String =
        text.trim().trim_start_matches("R$").chars().filter(|c| !c.is_whitespace()).collect();
    if cleaned.is_empty() {
        return Err("Informe um valor.".to_owned());
    }
    if !cleaned.chars().all(|c| c.is_ascii_digit() || c == '.' || c == ',') {
        return Err(invalid());
    }
    let (whole, fraction) = match cleaned.rfind(['.', ',']) {
        Some(at) if cleaned.len() - at - 1 <= 2 => (&cleaned[..at], &cleaned[at + 1..]),
        _ => (cleaned.as_str(), ""),
    };
    let digits: String = whole.chars().filter(char::is_ascii_digit).collect();
    if digits.is_empty() && fraction.is_empty() {
        return Err(invalid());
    }
    let reais: i64 = if digits.is_empty() { 0 } else { digits.parse().map_err(|_| invalid())? };
    let centavos: i64 = match fraction.len() {
        0 => 0,
        1 => fraction.parse::<i64>().map_err(|_| invalid())? * 10,
        _ => fraction.parse().map_err(|_| invalid())?,
    };
    reais.checked_mul(100).and_then(|r| r.checked_add(centavos)).ok_or_else(invalid)
}

/// Accepts only amounts above zero; the rule behind the amount fields.
pub fn validate_positive(text: &str) -> Result<(), String> {
    match parse_brl(text)? {
        0 => Err("O valor precisa ser maior que zero.".to_owned()),
        _ => Ok(()),
    }
}

/// Splits `total` into `parts` amounts that add up exactly; the first ones take the leftover cents.
pub fn split_installments(total: i64, parts: u32) -> Vec<i64> {
    let parts = i64::from(parts.max(1));
    let (each, extra) = (total / parts, total % parts);
    (0..parts).map(|i| each + i64::from(i < extra)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_currency() {
        assert_eq!(format_brl(123_456), "R$ 1.234,56");
        assert_eq!(format_brl(5), "R$ 0,05");
        assert_eq!(format_brl(-100_000_000), "-R$ 1.000.000,00");
        assert_eq!(format_input(123_456), "1234,56");
    }

    #[test]
    fn parses_what_people_type() {
        for (text, cents) in [
            ("1.234,56", 123_456),
            ("1234,56", 123_456),
            ("1234.56", 123_456),
            ("1,234.56", 123_456),
            ("R$ 12", 1_200),
            ("12,5", 1_250),
            ("1.000", 100_000),
            ("0,99", 99),
            (",5", 50),
        ] {
            assert_eq!(parse_brl(text), Ok(cents), "{text}");
        }
        for text in ["", "  ", "abc", "-5", "1,2,x", ".", "99999999999999999999"] {
            assert!(parse_brl(text).is_err(), "{text}");
        }
        assert!(validate_positive("0,00").is_err());
    }

    #[test]
    fn installments_add_up_exactly() {
        let parts = split_installments(100_00, 3);
        assert_eq!(parts, [33_34, 33_33, 33_33]);
        assert_eq!(parts.iter().sum::<i64>(), 100_00);
        assert_eq!(split_installments(500, 0), [500]);
    }
}
