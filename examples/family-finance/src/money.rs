//! Money as whole cents, so totals never pick up floating point errors. The reading, writing
//! and splitting are the framework's (`ravenrs_app`); this module only fixes the culture.

use ravenrs_app::{Culture, Remainder, allocate};

/// Writes cents as Brazilian currency: `R$ 1.234,56`.
pub fn format_brl(cents: i64) -> String {
    // The app's texts use a plain space after the symbol.
    Culture::pt_br().format_cents(cents).replace('\u{a0}', " ")
}

/// Writes cents the way a person types them: `1234,56`.
pub fn format_input(cents: i64) -> String {
    Culture::pt_br().input_cents(cents)
}

/// Reads an amount typed as `1.234,56`, `1234.5`, `R$ 12` or `12,`.
///
/// The last separator is the decimal one when one or two digits follow it;
/// with three digits after it, it separates thousands.
pub fn parse_brl(text: &str) -> Result<i64, String> {
    let culture = Culture::pt_br();
    culture.parse_cents(text).map_err(|error| error.message(&culture))
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
    allocate(total, parts, Remainder::First)
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
