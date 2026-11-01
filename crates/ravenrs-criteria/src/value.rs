//! Values that expressions compute and fields hold.

use std::cmp::Ordering;
use std::fmt;

/// A calendar date with an optional time of day, without a time zone.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DateTime {
    /// Days since 1970-01-01 (negative before).
    pub days: i64,
    /// Seconds since midnight.
    pub seconds: u32,
}

impl DateTime {
    /// The date `year-month-day` at midnight, or `None` if it does not exist.
    pub fn from_ymd(year: i32, month: u32, day: u32) -> Option<Self> {
        if !(1..=12).contains(&month) || day == 0 || day > days_in_month(year, month) {
            return None;
        }
        Some(Self { days: days_from_civil(year, month, day), seconds: 0 })
    }

    /// Year, month (1–12) and day (1–31).
    pub fn ymd(self) -> (i32, u32, u32) {
        civil_from_days(self.days)
    }

    /// Hours, minutes and seconds.
    pub fn hms(self) -> (u32, u32, u32) {
        (self.seconds / 3600, self.seconds / 60 % 60, self.seconds % 60)
    }

    /// Day of the week, 0 = Sunday.
    pub fn weekday(self) -> u32 {
        (self.days + 4).rem_euclid(7) as u32
    }

    /// Parses ISO 8601 dates: `2026-11-01`, `2026-11-01T14:30`, `2026-11-01 14:30:05`
    /// (a trailing zone such as `Z` or `-03:00` is accepted and ignored).
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim();
        let bytes = s.as_bytes();
        if bytes.len() < 10 || bytes[4] != b'-' || bytes[7] != b'-' {
            return None;
        }
        let year: i32 = s.get(0..4)?.parse().ok()?;
        let month: u32 = s.get(5..7)?.parse().ok()?;
        let day: u32 = s.get(8..10)?.parse().ok()?;
        let mut date = Self::from_ymd(year, month, day)?;
        let rest = &s[10..];
        if rest.is_empty() {
            return Some(date);
        }
        let time = rest.strip_prefix('T').or_else(|| rest.strip_prefix(' '))?;
        let h: u32 = time.get(0..2)?.parse().ok()?;
        let m: u32 = time.get(3..5)?.parse().ok()?;
        let sec: u32 = if time.as_bytes().get(5) == Some(&b':') { time.get(6..8)?.parse().ok()? } else { 0 };
        if h > 23 || m > 59 || sec > 59 {
            return None;
        }
        date.seconds = h * 3600 + m * 60 + sec;
        Some(date)
    }

    /// The current local date and time in the browser; the current UTC date
    /// and time elsewhere (the standard library has no time zones).
    pub fn now() -> Self {
        #[cfg(target_arch = "wasm32")]
        let secs = {
            let now = js_sys::Date::new_0();
            // getTimezoneOffset is minutes behind UTC.
            ((now.get_time() / 1000.0) - now.get_timezone_offset() * 60.0) as i64
        };
        #[cfg(not(target_arch = "wasm32"))]
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs() as i64);
        Self { days: secs.div_euclid(86_400), seconds: secs.rem_euclid(86_400) as u32 }
    }

    /// The date `n` days later (earlier for negative `n`).
    pub fn add_days(self, n: i64) -> Self {
        Self { days: self.days + n, ..self }
    }

    /// The same day `n` months later, clamped to the month's last day.
    pub fn add_months(self, n: i64) -> Self {
        let (y, m, d) = self.ymd();
        let total = i64::from(y) * 12 + i64::from(m) - 1 + n;
        let (year, month) = (total.div_euclid(12) as i32, (total.rem_euclid(12) + 1) as u32);
        let day = d.min(days_in_month(year, month));
        Self { days: days_from_civil(year, month, day), seconds: self.seconds }
    }
}

impl fmt::Display for DateTime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (y, m, d) = self.ymd();
        write!(f, "{y:04}-{m:02}-{d:02}")?;
        if self.seconds != 0 {
            let (h, mi, s) = self.hms();
            write!(f, "T{h:02}:{mi:02}:{s:02}")?;
        }
        Ok(())
    }
}

pub(crate) fn is_leap(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

pub(crate) fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap(year) => 29,
        2 => 28,
        _ => 0,
    }
}

// Howard Hinnant's civil date algorithms.
fn days_from_civil(y: i32, m: u32, d: u32) -> i64 {
    let y = i64::from(y) - i64::from(m <= 2);
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let m = i64::from(m);
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + i64::from(d) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn civil_from_days(z: i64) -> (i32, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    ((y + i64::from(m <= 2)) as i32, m, d)
}

/// A value: what fields hold and expressions compute.
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub enum Value {
    /// No value (a missing field, a JSON `null`).
    #[default]
    Null,
    /// True or false.
    Bool(bool),
    /// A number.
    Number(f64),
    /// Text.
    Text(String),
    /// A date and time.
    Date(DateTime),
}

impl Value {
    /// Converts a JSON scalar; arrays and objects become `Null` (they are
    /// collections, reached with `[Items][].Sum(...)`).
    #[cfg(feature = "json")]
    pub fn from_json(json: &serde_json::Value) -> Self {
        match json {
            serde_json::Value::Bool(b) => Self::Bool(*b),
            serde_json::Value::Number(n) => n.as_f64().map_or(Self::Null, Self::Number),
            serde_json::Value::String(s) => Self::Text(s.clone()),
            _ => Self::Null,
        }
    }

    /// True for `Null`.
    pub fn is_null(&self) -> bool {
        matches!(self, Self::Null)
    }

    /// The value as a number: numbers, booleans (1/0) and numeric text.
    pub fn as_number(&self) -> Option<f64> {
        match self {
            Self::Number(n) => Some(*n),
            Self::Bool(b) => Some(f64::from(u8::from(*b))),
            Self::Text(s) => s.trim().parse().ok(),
            _ => None,
        }
    }

    /// The value as a date: dates and ISO 8601 text.
    pub fn as_date(&self) -> Option<DateTime> {
        match self {
            Self::Date(d) => Some(*d),
            Self::Text(s) => DateTime::parse(s),
            _ => None,
        }
    }

    /// Truthiness for conditions: `true`, non-zero numbers and non-empty text.
    pub fn is_true(&self) -> bool {
        match self {
            Self::Bool(b) => *b,
            Self::Number(n) => *n != 0.0 && !n.is_nan(),
            Self::Text(s) => !s.is_empty(),
            Self::Date(_) => true,
            Self::Null => false,
        }
    }

    /// Orders values for sorting and comparisons: nulls first, then by kind;
    /// numbers numerically, text case-insensitively, dates chronologically.
    pub fn compare(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Self::Null, Self::Null) => Ordering::Equal,
            (Self::Null, _) => Ordering::Less,
            (_, Self::Null) => Ordering::Greater,
            (Self::Number(a), Self::Number(b)) => a.total_cmp(b),
            (Self::Date(a), Self::Date(b)) => a.cmp(b),
            (Self::Bool(a), Self::Bool(b)) => a.cmp(b),
            (Self::Text(a), Self::Text(b)) => {
                let o = cmp_ignore_case(a, b);
                if o == Ordering::Equal { a.cmp(b) } else { o }
            }
            (a, b) => match (a.as_date(), b.as_date()) {
                (Some(x), Some(y)) => x.cmp(&y),
                _ => match (a.as_number(), b.as_number()) {
                    (Some(x), Some(y)) => x.total_cmp(&y),
                    _ => a.to_text().cmp(&b.to_text()),
                },
            },
        }
    }

    /// Equality as the `=` operator sees it (`1 = '1'` is false, `1 = 1.0` is true).
    pub fn loose_eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Text(a), Self::Text(b)) => a == b,
            (Self::Date(_), Self::Text(_)) | (Self::Text(_), Self::Date(_)) => {
                self.as_date().is_some() && self.as_date() == other.as_date()
            }
            _ => {
                self.compare(other) == Ordering::Equal
                    && std::mem::discriminant(self) == std::mem::discriminant(other)
            }
        }
    }

    /// Text for display without a format: numbers in invariant form.
    pub fn to_text(&self) -> String {
        match self {
            Self::Null => String::new(),
            Self::Bool(b) => b.to_string(),
            Self::Number(n) => format_plain(*n),
            Self::Text(s) => s.clone(),
            Self::Date(d) => d.to_string(),
        }
    }
}

fn cmp_ignore_case(a: &str, b: &str) -> Ordering {
    a.chars().flat_map(char::to_lowercase).cmp(b.chars().flat_map(char::to_lowercase))
}

/// A number without trailing zeros (`2`, `2.5`, `0.1`).
pub(crate) fn format_plain(n: f64) -> String {
    if n.fract() == 0.0 && n.abs() < 1e15 {
        format!("{}", n as i64)
    } else {
        let s = format!("{n:.10}");
        s.trim_end_matches('0').trim_end_matches('.').to_owned()
    }
}

impl From<f64> for Value {
    fn from(n: f64) -> Self {
        Self::Number(n)
    }
}

impl From<i64> for Value {
    fn from(n: i64) -> Self {
        Self::Number(n as f64)
    }
}

impl From<bool> for Value {
    fn from(b: bool) -> Self {
        Self::Bool(b)
    }
}

impl From<&str> for Value {
    fn from(s: &str) -> Self {
        Self::Text(s.to_owned())
    }
}

impl From<String> for Value {
    fn from(s: String) -> Self {
        Self::Text(s)
    }
}

impl From<DateTime> for Value {
    fn from(d: DateTime) -> Self {
        Self::Date(d)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn civil_dates_round_trip() {
        for days in [-800_000, -1, 0, 1, 19_000, 20_393, 2_000_000] {
            let d = DateTime { days, seconds: 0 };
            let (y, m, dd) = d.ymd();
            assert_eq!(DateTime::from_ymd(y, m, dd), Some(d));
        }
        assert_eq!(DateTime::from_ymd(1970, 1, 1).map(|d| d.days), Some(0));
        assert_eq!(DateTime::from_ymd(2026, 2, 29), None);
        assert_eq!(DateTime::from_ymd(2024, 2, 29).map(|d| d.weekday()), Some(4), "a Thursday");
    }

    #[test]
    fn parses_iso_dates_and_times() {
        let d = DateTime::parse("2026-11-01T14:30:05Z").expect("valid");
        assert_eq!((d.ymd(), d.hms()), ((2026, 11, 1), (14, 30, 5)));
        assert_eq!(DateTime::parse("2026-11-01 08:15").map(|d| d.hms()), Some((8, 15, 0)));
        assert!(DateTime::parse("01/11/2026").is_none());
        assert!(DateTime::parse("2026-13-01").is_none());
    }

    #[test]
    fn month_arithmetic_clamps_to_the_last_day() {
        let jan31 = DateTime::from_ymd(2026, 1, 31).expect("valid");
        assert_eq!(jan31.add_months(1).ymd(), (2026, 2, 28));
        assert_eq!(jan31.add_months(-2).ymd(), (2025, 11, 30));
        assert_eq!(jan31.add_days(1).ymd(), (2026, 2, 1));
    }

    #[test]
    fn comparison_and_equality() {
        assert_eq!(Value::from("apple").compare(&Value::from("Banana")), Ordering::Less);
        assert_eq!(Value::Null.compare(&Value::from(0.0)), Ordering::Less);
        assert!(Value::from(1.0).loose_eq(&Value::from(1_i64)));
        assert!(!Value::from(1.0).loose_eq(&Value::from("1")));
        assert!(
            Value::from("2026-01-02").loose_eq(&Value::Date(DateTime::from_ymd(2026, 1, 2).expect("valid")))
        );
        assert_eq!(format_plain(2.50), "2.5");
        assert_eq!(format_plain(-3.0), "-3");
    }
}
