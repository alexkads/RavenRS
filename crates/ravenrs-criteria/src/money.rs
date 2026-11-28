//! Money as whole cents (`i64`): read, written and split without floating point, so the same
//! text is the same amount everywhere and a split always adds up to the total.
//!
//! ```
//! use ravenrs_criteria::{Culture, Remainder, allocate};
//!
//! let pt = Culture::pt_br();
//! assert_eq!(pt.parse_cents("1.234,56"), Ok(123_456));
//! assert_eq!(pt.format_cents(123_456), "R$\u{a0}1.234,56");
//! assert_eq!(allocate(10_000, 3, Remainder::First), [3_334, 3_333, 3_333]);
//! ```

use std::fmt;

use crate::format::Culture;

/// Why a text is not an amount of money.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum MoneyError {
    /// Nothing was typed.
    Empty,
    /// The text is not an amount; it holds the text, trimmed.
    Invalid(String),
    /// A minus sign where only zero or more is taken.
    Negative,
    /// More than whole cents in an `i64` hold.
    TooLarge,
}

impl MoneyError {
    /// What to tell the person, in the culture's language, with an example in its format.
    ///
    /// ```
    /// use ravenrs_criteria::{Culture, MoneyError};
    ///
    /// let pt = Culture::pt_br();
    /// assert_eq!(
    ///     pt.parse_cents("abc").unwrap_err().message(&pt),
    ///     "\"abc\" não é um valor válido. Use, por exemplo, 1.234,56."
    /// );
    /// ```
    pub fn message(&self, culture: &Culture) -> String {
        let pt = culture.decimal == ',' && culture.name.starts_with("pt");
        let example = format!("1{}234{}56", culture.group, culture.decimal);
        match (self, pt) {
            (Self::Empty, true) => "Informe um valor.".to_owned(),
            (Self::Empty, false) => "Enter an amount.".to_owned(),
            (Self::Invalid(text), true) => {
                format!("\"{text}\" não é um valor válido. Use, por exemplo, {example}.")
            }
            (Self::Invalid(text), false) => {
                format!("\"{text}\" is not an amount. Use, for example, {example}.")
            }
            (Self::Negative, true) => "O valor não pode ser negativo.".to_owned(),
            (Self::Negative, false) => "The amount cannot be negative.".to_owned(),
            (Self::TooLarge, true) => "O valor é grande demais.".to_owned(),
            (Self::TooLarge, false) => "The amount is too large.".to_owned(),
        }
    }
}

impl fmt::Display for MoneyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message(&Culture::en_us()))
    }
}

impl std::error::Error for MoneyError {}

/// Digits grouped by thousands with `group`.
fn grouped(digits: &str, group: char) -> String {
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(group);
        }
        out.push(c);
    }
    out
}

impl Culture {
    /// Cents as this culture writes money: `R$ 1.234,56`, `-R$ 0,05`, `$1,234.56`.
    ///
    /// ```
    /// use ravenrs_criteria::Culture;
    ///
    /// assert_eq!(Culture::en_us().format_cents(-123_456), "-$1,234.56");
    /// assert_eq!(Culture::pt_br().format_cents(5), "R$\u{a0}0,05");
    /// ```
    pub fn format_cents(&self, cents: i64) -> String {
        let abs = cents.unsigned_abs();
        let sign = if cents < 0 { "-" } else { "" };
        let space = if self.currency_space { "\u{a0}" } else { "" };
        let whole = grouped(&(abs / 100).to_string(), self.group);
        format!("{sign}{}{space}{whole}{}{:02}", self.currency, self.decimal, abs % 100)
    }

    /// Cents as a person types them, to fill a field: `1234,56`, `-0,05`.
    ///
    /// ```
    /// use ravenrs_criteria::Culture;
    ///
    /// assert_eq!(Culture::pt_br().input_cents(123_456), "1234,56");
    /// assert_eq!(Culture::en_us().input_cents(-5), "-0.05");
    /// ```
    pub fn input_cents(&self, cents: i64) -> String {
        let abs = cents.unsigned_abs();
        let sign = if cents < 0 { "-" } else { "" };
        format!("{sign}{}{}{:02}", abs / 100, self.decimal, abs % 100)
    }

    /// Reads an amount of zero or more: `1.234,56`, `1234,56`, `1234.56`, `1,234.56`, `R$ 12`,
    /// `12,5` and `,5`. The culture's symbol may be there or not. The last separator is the
    /// decimal one when one or two digits follow it; with three, it separates thousands, so
    /// `1.000` is a thousand in any culture.
    ///
    /// ```
    /// use ravenrs_criteria::{Culture, MoneyError};
    ///
    /// let pt = Culture::pt_br();
    /// assert_eq!(pt.parse_cents("1.000"), Ok(100_000));
    /// assert_eq!(pt.parse_cents("-5"), Err(MoneyError::Negative));
    /// ```
    pub fn parse_cents(&self, text: &str) -> Result<i64, MoneyError> {
        let cents = self.parse_signed_cents(text)?;
        if cents < 0 { Err(MoneyError::Negative) } else { Ok(cents) }
    }

    /// [`Self::parse_cents`] that also takes a minus sign, for refunds and adjustments.
    ///
    /// ```
    /// use ravenrs_criteria::Culture;
    ///
    /// assert_eq!(Culture::pt_br().parse_signed_cents("-R$ 1,50"), Ok(-150));
    /// ```
    pub fn parse_signed_cents(&self, text: &str) -> Result<i64, MoneyError> {
        let trimmed = text.trim();
        let invalid = || MoneyError::Invalid(trimmed.to_owned());
        let (negative, rest) = match trimmed.strip_prefix('-') {
            Some(rest) => (true, rest.trim_start()),
            None => (false, trimmed),
        };
        let rest = rest.strip_prefix(self.currency).unwrap_or(rest);
        let cleaned: String = rest.chars().filter(|c| !c.is_whitespace()).collect();
        if cleaned.is_empty() {
            return Err(if negative { invalid() } else { MoneyError::Empty });
        }
        if !cleaned.chars().all(|c| c.is_ascii_digit() || c == '.' || c == ',') {
            return Err(invalid());
        }
        let (whole, fraction) = match cleaned.rfind(['.', ',']) {
            Some(at) if cleaned.len() - at - 1 <= 2 => (&cleaned[..at], &cleaned[at + 1..]),
            _ => (cleaned.as_str(), ""),
        };
        // A separator left in the fraction, or two in a row, is a typo, not an amount.
        if fraction.contains(['.', ','])
            || whole.ends_with(['.', ','])
            || whole.contains(",,")
            || whole.contains("..")
        {
            return Err(invalid());
        }
        let digits: String = whole.chars().filter(char::is_ascii_digit).collect();
        if digits.is_empty() && fraction.is_empty() {
            return Err(invalid());
        }
        let units: i64 =
            if digits.is_empty() { 0 } else { digits.parse().map_err(|_| MoneyError::TooLarge)? };
        let cents: i64 = match fraction.len() {
            0 => 0,
            1 => fraction.parse::<i64>().map_err(|_| invalid())? * 10,
            _ => fraction.parse().map_err(|_| invalid())?,
        };
        let total = units.checked_mul(100).and_then(|u| u.checked_add(cents)).ok_or(MoneyError::TooLarge)?;
        Ok(if negative { -total } else { total })
    }
}

/// Where the cents left over by [`allocate`] go.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Remainder {
    /// One cent more in each of the first parts.
    First,
    /// All the rest in the last part, as many banks do.
    Last,
}

/// Splits `total` into `parts` amounts that add up exactly to it; `0` parts count as one.
///
/// ```
/// use ravenrs_criteria::{Remainder, allocate};
///
/// assert_eq!(allocate(10_000, 3, Remainder::Last), [3_333, 3_333, 3_334]);
/// assert_eq!(allocate(-10, 3, Remainder::First), [-4, -3, -3]);
/// ```
pub fn allocate(total: i64, parts: u32, rest: Remainder) -> Vec<i64> {
    let parts = i64::from(parts.max(1));
    let (each, extra) = (total / parts, total % parts);
    let step = extra.signum();
    (0..parts)
        .map(|i| match rest {
            Remainder::First => each + if i < extra.abs() { step } else { 0 },
            Remainder::Last => each + if i == parts - 1 { extra } else { 0 },
        })
        .collect()
}

/// Splits `total` in proportion to `weights`, giving the cents left over to the largest
/// remainders, so the parts add up exactly to it. Weights of zero or less take nothing;
/// with no positive weight, every part is zero but the first, which takes it all.
///
/// ```
/// use ravenrs_criteria::allocate_by;
///
/// assert_eq!(allocate_by(100, &[1, 1, 1]), [34, 33, 33]);
/// assert_eq!(allocate_by(1_000, &[50, 30, 20]), [500, 300, 200]);
/// ```
pub fn allocate_by(total: i64, weights: &[i64]) -> Vec<i64> {
    if weights.is_empty() {
        return Vec::new();
    }
    let weights: Vec<i128> = weights.iter().map(|w| i128::from((*w).max(0))).collect();
    let sum: i128 = weights.iter().sum();
    if sum == 0 {
        let mut parts = vec![0; weights.len()];
        parts[0] = total;
        return parts;
    }
    let total = i128::from(total);
    let mut parts: Vec<i128> = weights.iter().map(|w| total * w / sum).collect();
    let mut order: Vec<usize> = (0..weights.len()).collect();
    // Largest remainder first, the earlier part on a tie.
    order.sort_by_key(|&i| std::cmp::Reverse((total * weights[i] % sum).abs()));
    let left = total - parts.iter().sum::<i128>();
    for &i in order.iter().cycle().take(usize::try_from(left.unsigned_abs()).unwrap_or(0)) {
        parts[i] += left.signum();
    }
    parts.into_iter().map(|p| i64::try_from(p).unwrap_or(if p < 0 { i64::MIN } else { i64::MAX })).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Value;

    #[test]
    fn reads_what_people_type() {
        let pt = Culture::pt_br();
        for (text, cents) in [
            ("1.234,56", 123_456),
            ("1234,56", 123_456),
            ("1234.56", 123_456),
            ("1,234.56", 123_456),
            ("R$ 12", 1_200),
            ("R$12", 1_200),
            ("12,5", 1_250),
            ("1.000", 100_000),
            ("1,000", 100_000),
            ("0,99", 99),
            (",5", 50),
            ("12,", 1_200),
        ] {
            assert_eq!(pt.parse_cents(text), Ok(cents), "{text}");
        }
        assert_eq!(Culture::en_us().parse_cents("$1,234.56"), Ok(123_456));
        for (text, error) in [
            ("", MoneyError::Empty),
            ("  ", MoneyError::Empty),
            ("abc", MoneyError::Invalid("abc".into())),
            ("1,2,x", MoneyError::Invalid("1,2,x".into())),
            (".", MoneyError::Invalid(".".into())),
            ("1,,5", MoneyError::Invalid("1,,5".into())),
            ("-5", MoneyError::Negative),
            ("99999999999999999999", MoneyError::TooLarge),
            ("92233720368547758,08", MoneyError::TooLarge),
        ] {
            assert_eq!(pt.parse_cents(text), Err(error), "{text}");
        }
        assert_eq!(pt.parse_signed_cents("-1.234,56"), Ok(-123_456));
        assert_eq!(pt.parse_signed_cents("- R$ 0,05"), Ok(-5));
        assert_eq!(pt.parse_signed_cents("-"), Err(MoneyError::Invalid("-".into())));
    }

    #[test]
    fn writes_money_in_the_culture() {
        let (pt, en) = (Culture::pt_br(), Culture::en_us());
        assert_eq!(pt.format_cents(123_456), "R$\u{a0}1.234,56");
        assert_eq!(pt.format_cents(-5), "-R$\u{a0}0,05");
        assert_eq!(pt.format_cents(-100_000_000), "-R$\u{a0}1.000.000,00");
        assert_eq!(en.format_cents(123_456), "$1,234.56");
        assert_eq!(en.format_cents(i64::MIN), "-$92,233,720,368,547,758.08", "no panic at the edge");
        assert_eq!(pt.input_cents(123_456), "1234,56");
        assert_eq!(pt.parse_cents(&pt.input_cents(987_654_321)), Ok(987_654_321), "what it writes, it reads");
        assert_eq!(pt.parse_signed_cents(&pt.format_cents(-123_456)), Ok(-123_456));
    }

    #[test]
    fn messages_say_what_to_fix() {
        let (pt, en) = (Culture::pt_br(), Culture::en_us());
        assert_eq!(MoneyError::Empty.message(&pt), "Informe um valor.");
        assert_eq!(
            MoneyError::Invalid("1,2,x".into()).message(&en),
            "\"1,2,x\" is not an amount. Use, for example, 1,234.56."
        );
        assert_eq!(MoneyError::TooLarge.to_string(), "The amount is too large.");
    }

    #[test]
    fn splits_add_up_exactly() {
        for total in [0, 1, 99, 10_000, 123_457, -123_457, i64::MAX / 2] {
            for parts in 1..=120 {
                for rest in [Remainder::First, Remainder::Last] {
                    let split = allocate(total, parts, rest);
                    assert_eq!(split.len(), parts as usize);
                    assert_eq!(split.iter().sum::<i64>(), total, "{total} in {parts}, {rest:?}");
                }
            }
        }
        assert_eq!(allocate(10_000, 3, Remainder::First), [3_334, 3_333, 3_333]);
        assert_eq!(allocate(10_000, 3, Remainder::Last), [3_333, 3_333, 3_334]);
        assert_eq!(allocate(500, 0, Remainder::First), [500]);
        assert_eq!(allocate(-100, 3, Remainder::Last), [-33, -33, -34]);
    }

    #[test]
    fn proportional_splits_add_up_exactly() {
        assert_eq!(allocate_by(100, &[1, 1, 1]), [34, 33, 33]);
        assert_eq!(allocate_by(-100, &[1, 1, 1]), [-34, -33, -33]);
        assert_eq!(allocate_by(1_000, &[2, 0, -5, 1]), [667, 0, 0, 333]);
        assert_eq!(allocate_by(77, &[0, 0]), [77, 0]);
        assert_eq!(allocate_by(77, &[]), Vec::<i64>::new());
        for total in [1, 999, 123_457, -5_003] {
            let weights = [3, 7, 11, 13, 1];
            assert_eq!(allocate_by(total, &weights).iter().sum::<i64>(), total, "{total}");
        }
    }

    #[test]
    fn values_give_whole_cents_only() {
        assert_eq!(Value::Number(1250.0).cents(), Some(1250));
        assert_eq!(Value::Number(12.5).cents(), None);
        assert_eq!(Value::Number(1e30).cents(), None);
        assert_eq!(Value::Number(f64::NAN).cents(), None);
        assert_eq!(Value::Text("12".into()).cents(), None);
    }
}
