//! A small civil-date type for calendars and date pickers (proleptic Gregorian,
//! no time zones), so date components need no external dependency.

use std::fmt;

/// A calendar date (year, month, day), always valid.
///
/// ```
/// use ravenrs_widgets::Date;
///
/// let d = Date::new(2026, 1, 31).unwrap();
/// assert_eq!(d.add_months(1).to_string(), "2026-02-28");
/// assert_eq!(d.add_days(1).short(), "Feb 1, 2026");
/// assert!(Date::new(2026, 2, 30).is_none());
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Date {
    year: i32,
    month: u8,
    day: u8,
}

pub const MONTH_NAMES: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

impl Date {
    /// Returns `None` for impossible dates (e.g. February 30).
    pub fn new(year: i32, month: u8, day: u8) -> Option<Self> {
        (1..=12)
            .contains(&month)
            .then(|| days_in_month(year, month))
            .filter(|&n| day >= 1 && day <= n)
            .map(|_| Self { year, month, day })
    }

    /// The year, e.g. 2026.
    pub fn year(self) -> i32 {
        self.year
    }

    /// The month, 1 = January … 12 = December.
    pub fn month(self) -> u8 {
        self.month
    }

    /// The day of the month, starting at 1.
    pub fn day(self) -> u8 {
        self.day
    }

    /// Day of the week, 0 = Sunday … 6 = Saturday.
    pub fn weekday(self) -> u8 {
        // Sakamoto's method.
        const T: [i32; 12] = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
        let y = if self.month < 3 { self.year - 1 } else { self.year };
        let w = (y + y.div_euclid(4) - y.div_euclid(100)
            + y.div_euclid(400)
            + T[usize::from(self.month - 1)]
            + i32::from(self.day))
        .rem_euclid(7);
        w as u8
    }

    /// The first day of this date's month.
    pub fn first_of_month(self) -> Self {
        Self { day: 1, ..self }
    }

    /// Moves by whole months, clamping the day (Jan 31 + 1 month = Feb 28/29).
    pub fn add_months(self, months: i32) -> Self {
        let total = self.year * 12 + i32::from(self.month) - 1 + months;
        let (year, month) = (total.div_euclid(12), (total.rem_euclid(12) + 1) as u8);
        Self { year, month, day: self.day.min(days_in_month(year, month)) }
    }

    /// Moves by `days` (negative goes back).
    pub fn add_days(self, days: i32) -> Self {
        Self::from_days(self.to_days() + i64::from(days))
    }

    /// Days since 1970-01-01.
    pub fn to_days(self) -> i64 {
        // Howard Hinnant's days_from_civil.
        let y = i64::from(self.year) - i64::from(self.month <= 2);
        let era = y.div_euclid(400);
        let yoe = y - era * 400;
        let m = i64::from(self.month);
        let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + i64::from(self.day) - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        era * 146_097 + doe - 719_468
    }

    /// The date `days` days after 1970-01-01 (inverse of [`Date::to_days`]).
    pub fn from_days(days: i64) -> Self {
        let z = days + 719_468;
        let era = z.div_euclid(146_097);
        let doe = z - era * 146_097;
        let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let day = (doy - (153 * mp + 2) / 5 + 1) as u8;
        let month = if mp < 10 { mp + 3 } else { mp - 9 } as u8;
        let year = (yoe + era * 400 + i64::from(month <= 2)) as i32;
        Self { year, month, day }
    }

    /// "October 2026".
    pub fn month_title(self) -> String {
        format!("{} {}", MONTH_NAMES[usize::from(self.month - 1)], self.year)
    }

    /// "Oct 31, 2026".
    pub fn short(self) -> String {
        format!("{} {}, {}", &MONTH_NAMES[usize::from(self.month - 1)][..3], self.day, self.year)
    }
}

impl fmt::Display for Date {
    /// ISO 8601: `2026-10-31`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
}

/// Whether `year` has a February 29 (Gregorian rules).
pub fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

/// Number of days in `month` (1–12) of `year`.
pub fn days_in_month(year: i32, month: u8) -> u8 {
    match month {
        2 if is_leap_year(year) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_and_formats() {
        assert!(Date::new(2026, 2, 29).is_none());
        assert!(Date::new(2024, 2, 29).is_some());
        let d = Date::new(2026, 10, 31).unwrap();
        assert_eq!(d.to_string(), "2026-10-31");
        assert_eq!(d.short(), "Oct 31, 2026");
        assert_eq!(d.month_title(), "October 2026");
    }

    #[test]
    fn weekdays_and_arithmetic() {
        assert_eq!(Date::new(2026, 10, 31).unwrap().weekday(), 6, "a Saturday");
        assert_eq!(Date::new(2000, 1, 1).unwrap().weekday(), 6);
        assert_eq!(Date::new(2026, 1, 31).unwrap().add_months(1), Date::new(2026, 2, 28).unwrap());
        assert_eq!(Date::new(2026, 12, 15).unwrap().add_months(1), Date::new(2027, 1, 15).unwrap());
        assert_eq!(Date::new(2026, 3, 1).unwrap().add_days(-1), Date::new(2026, 2, 28).unwrap());
        assert_eq!(Date::new(1970, 1, 1).unwrap().to_days(), 0);
        for days in [-1_000_000_i64, -1, 0, 59, 60, 19_000, 2_000_000] {
            assert_eq!(Date::from_days(days).to_days(), days);
        }
    }
}
