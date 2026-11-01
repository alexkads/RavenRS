//! How something repeats: a rule, with no UI, that answers on which days.

use std::fmt;

use crate::date::{Date, days_in_month};

/// How often a [`RecurrenceRule`] comes back.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Frequency {
    /// Only on its first day.
    Once,
    /// Every `interval` days.
    Daily,
    /// Every `interval` weeks, on the chosen weekdays.
    Weekly,
    /// Every `interval` months, on the first day's date.
    #[default]
    Monthly,
    /// Every `interval` years, on the first day's date.
    Yearly,
}

impl Frequency {
    /// Every frequency, in the order an editor offers them.
    pub const ALL: [Self; 5] = [Self::Once, Self::Daily, Self::Weekly, Self::Monthly, Self::Yearly];
}

/// When a [`RecurrenceRule`] stops.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum RecurrenceEnd {
    /// It goes on.
    #[default]
    Never,
    /// Its last day can be this one.
    Until(Date),
    /// After this many occurrences, counting the first.
    After(u32),
}

/// A repeating schedule, starting from a date given when asking for its days.
/// A date past the end of a shorter month falls on that month's last day and
/// returns to its own day after: the 31st is February 28, then March 31.
///
/// ```
/// use ravenrs_widgets::{Date, Frequency, RecurrenceEnd, RecurrenceRule};
///
/// let start = Date::new(2026, 1, 31).unwrap();
/// let rule = RecurrenceRule::new(Frequency::Monthly).end(RecurrenceEnd::After(3));
/// let days = rule.occurrences(start, start, start.add_months(12));
/// assert_eq!(days, [start, Date::new(2026, 2, 28).unwrap(), Date::new(2026, 3, 31).unwrap()]);
///
/// // It can be stored as text, in the format of iCalendar.
/// assert_eq!(rule.to_text(), "FREQ=MONTHLY;COUNT=3");
/// assert_eq!(RecurrenceRule::parse(&rule.to_text()), Some(rule));
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RecurrenceRule {
    /// How often.
    pub frequency: Frequency,
    /// Every how many days, weeks, months or years; at least 1.
    pub interval: u32,
    /// For weekly rules, the weekdays from Sunday to Saturday. With none chosen,
    /// the first day's weekday.
    pub weekdays: [bool; 7],
    /// When it stops.
    pub end: RecurrenceEnd,
}

impl Default for RecurrenceRule {
    fn default() -> Self {
        Self::new(Frequency::default())
    }
}

/// iCalendar's names, from Sunday.
const WEEKDAY_CODES: [&str; 7] = ["SU", "MO", "TU", "WE", "TH", "FR", "SA"];

impl RecurrenceRule {
    /// A rule at this frequency, every time, with no end.
    pub fn new(frequency: Frequency) -> Self {
        Self { frequency, interval: 1, weekdays: [false; 7], end: RecurrenceEnd::Never }
    }

    /// Sets the interval; below 1 counts as 1.
    pub fn every(mut self, interval: u32) -> Self {
        self.interval = interval.max(1);
        self
    }

    /// Sets the weekdays of a weekly rule, 0 = Sunday … 6 = Saturday.
    pub fn on_weekdays(mut self, days: impl IntoIterator<Item = u8>) -> Self {
        self.weekdays = [false; 7];
        for day in days {
            if let Some(chosen) = self.weekdays.get_mut(usize::from(day)) {
                *chosen = true;
            }
        }
        self
    }

    /// Sets when it stops.
    pub fn end(mut self, end: RecurrenceEnd) -> Self {
        self.end = end;
        self
    }

    /// The days from `start` on, in order. It never ends by itself when the rule
    /// does not, so limit it, as [`Self::occurrences`] does.
    pub fn days_from(&self, start: Date) -> impl Iterator<Item = Date> + use<> {
        let rule = *self;
        let mut produced = 0_u32;
        Occurrences { rule, start, step: 0, weekday: 0 }.take_while(move |day| {
            produced += 1;
            match rule.end {
                RecurrenceEnd::Never => true,
                RecurrenceEnd::Until(last) => *day <= last,
                RecurrenceEnd::After(count) => produced <= count,
            }
        })
    }

    /// The days of a rule starting at `start` that fall in `from..=to`, in order.
    pub fn occurrences(&self, start: Date, from: Date, to: Date) -> Vec<Date> {
        self.days_from(start).take_while(|day| *day <= to).filter(|day| *day >= from).collect()
    }

    /// The rule as text, the part of iCalendar's `RRULE` it covers.
    pub fn to_text(&self) -> String {
        let name = match self.frequency {
            Frequency::Once => return "FREQ=ONCE".to_owned(),
            Frequency::Daily => "DAILY",
            Frequency::Weekly => "WEEKLY",
            Frequency::Monthly => "MONTHLY",
            Frequency::Yearly => "YEARLY",
        };
        let mut text = format!("FREQ={name}");
        if self.interval > 1 {
            text.push_str(&format!(";INTERVAL={}", self.interval));
        }
        if self.frequency == Frequency::Weekly && self.weekdays.contains(&true) {
            let days: Vec<&str> = (0..7).filter(|d| self.weekdays[*d]).map(|d| WEEKDAY_CODES[d]).collect();
            text.push_str(&format!(";BYDAY={}", days.join(",")));
        }
        match self.end {
            RecurrenceEnd::Never => {}
            RecurrenceEnd::Until(day) => {
                text.push_str(&format!(";UNTIL={:04}{:02}{:02}", day.year(), day.month(), day.day()));
            }
            RecurrenceEnd::After(count) => text.push_str(&format!(";COUNT={count}")),
        }
        text
    }

    /// Reads what [`Self::to_text`] writes; `None` when it is not a rule.
    pub fn parse(text: &str) -> Option<Self> {
        let mut rule = Self::new(Frequency::Once);
        let mut has_frequency = false;
        for part in text.trim().split(';') {
            let (key, value) = part.split_once('=')?;
            match key {
                "FREQ" => {
                    has_frequency = true;
                    rule.frequency = match value {
                        "ONCE" => Frequency::Once,
                        "DAILY" => Frequency::Daily,
                        "WEEKLY" => Frequency::Weekly,
                        "MONTHLY" => Frequency::Monthly,
                        "YEARLY" => Frequency::Yearly,
                        _ => return None,
                    };
                }
                "INTERVAL" => rule.interval = value.parse::<u32>().ok()?.max(1),
                "BYDAY" => {
                    for code in value.split(',') {
                        let day = WEEKDAY_CODES.iter().position(|c| *c == code)?;
                        rule.weekdays[day] = true;
                    }
                }
                "UNTIL" => {
                    let (year, month, day) = (value.get(..4)?, value.get(4..6)?, value.get(6..8)?);
                    let date = Date::new(year.parse().ok()?, month.parse().ok()?, day.parse().ok()?)?;
                    rule.end = RecurrenceEnd::Until(date);
                }
                "COUNT" => rule.end = RecurrenceEnd::After(value.parse().ok()?),
                _ => return None,
            }
        }
        has_frequency.then_some(rule)
    }
}

impl fmt::Display for RecurrenceRule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_text())
    }
}

struct Occurrences {
    rule: RecurrenceRule,
    start: Date,
    /// Periods since the start.
    step: i64,
    /// For weekly rules, the next weekday to look at in this week.
    weekday: u8,
}

/// `start`, `months` later, on its own day or on the last day of a shorter month.
fn months_after(start: Date, months: i64) -> Option<Date> {
    let months = i32::try_from(months).ok()?;
    let first = start.first_of_month().add_months(months);
    let day = start.day().min(days_in_month(first.year(), first.month()));
    Date::new(first.year(), first.month(), day)
}

impl Iterator for Occurrences {
    type Item = Date;

    fn next(&mut self) -> Option<Date> {
        let interval = i64::from(self.rule.interval.max(1));
        match self.rule.frequency {
            Frequency::Once => {
                self.step += 1;
                (self.step == 1).then_some(self.start)
            }
            Frequency::Daily => {
                let day = Date::from_days(self.start.to_days() + self.step * interval);
                self.step += 1;
                Some(day)
            }
            Frequency::Monthly | Frequency::Yearly => {
                let per_step = if self.rule.frequency == Frequency::Yearly { 12 } else { 1 };
                let day = months_after(self.start, self.step * interval * per_step);
                self.step += 1;
                day
            }
            Frequency::Weekly => {
                let own = usize::from(self.start.weekday());
                let chosen = self.rule.weekdays;
                let any = chosen.contains(&true);
                let week_start = self.start.to_days() - own as i64;
                // At most one empty pass over a week, since some weekday is always chosen.
                loop {
                    if self.weekday > 6 {
                        self.weekday = 0;
                        self.step += 1;
                    }
                    let weekday = usize::from(self.weekday);
                    self.weekday += 1;
                    if if any { chosen[weekday] } else { weekday == own } {
                        let day = Date::from_days(week_start + self.step * interval * 7 + weekday as i64);
                        // The first week may have chosen days before the start.
                        if day >= self.start {
                            return Some(day);
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(y: i32, m: u8, d: u8) -> Date {
        Date::new(y, m, d).unwrap_or_else(|| panic!("invalid date"))
    }

    fn in_a_year(rule: RecurrenceRule, start: Date) -> Vec<Date> {
        rule.occurrences(start, start, start.add_months(12))
    }

    #[test]
    fn once_is_only_the_first_day() {
        let start = date(2026, 3, 10);
        assert_eq!(in_a_year(RecurrenceRule::new(Frequency::Once), start), [start]);
    }

    #[test]
    fn daily_counts_by_interval() {
        let start = date(2026, 12, 30);
        let days = RecurrenceRule::new(Frequency::Daily).every(2).occurrences(start, start, date(2027, 1, 3));
        assert_eq!(days, [date(2026, 12, 30), date(2027, 1, 1), date(2027, 1, 3)]);
    }

    #[test]
    fn weekly_falls_on_the_chosen_days_and_skips_weeks() {
        // A Wednesday; Mondays and Fridays, every other week.
        let start = date(2026, 11, 4);
        let rule = RecurrenceRule::new(Frequency::Weekly).every(2).on_weekdays([1, 5]);
        let days = rule.occurrences(start, start, date(2026, 11, 30));
        assert_eq!(days, [date(2026, 11, 6), date(2026, 11, 16), date(2026, 11, 20), date(2026, 11, 30)]);
    }

    #[test]
    fn weekly_with_no_day_chosen_keeps_its_own() {
        let start = date(2026, 11, 4);
        let days = RecurrenceRule::new(Frequency::Weekly).occurrences(start, start, date(2026, 11, 18));
        assert_eq!(days, [date(2026, 11, 4), date(2026, 11, 11), date(2026, 11, 18)]);
    }

    #[test]
    fn a_long_month_day_returns_after_a_short_month() {
        let start = date(2026, 1, 31);
        let days = RecurrenceRule::new(Frequency::Monthly).occurrences(start, start, date(2026, 4, 30));
        assert_eq!(days, [start, date(2026, 2, 28), date(2026, 3, 31), date(2026, 4, 30)]);
    }

    #[test]
    fn february_29_comes_back_in_leap_years() {
        let start = date(2024, 2, 29);
        let days = RecurrenceRule::new(Frequency::Yearly).occurrences(start, start, date(2028, 3, 1));
        assert_eq!(days, [start, date(2025, 2, 28), date(2026, 2, 28), date(2027, 2, 28), date(2028, 2, 29)]);
    }

    #[test]
    fn the_end_counts_from_the_start_whatever_range_is_asked() {
        let start = date(2026, 1, 10);
        let rule = RecurrenceRule::new(Frequency::Monthly).end(RecurrenceEnd::After(3));
        assert_eq!(rule.occurrences(start, date(2026, 3, 1), date(2027, 1, 1)), [date(2026, 3, 10)]);
        assert!(rule.occurrences(start, date(2026, 4, 1), date(2027, 1, 1)).is_empty());
        let until = RecurrenceRule::new(Frequency::Monthly).end(RecurrenceEnd::Until(date(2026, 2, 10)));
        assert_eq!(in_a_year(until, start), [start, date(2026, 2, 10)]);
    }

    #[test]
    fn nothing_falls_before_the_start() {
        let start = date(2026, 6, 15);
        let rule = RecurrenceRule::new(Frequency::Monthly);
        assert_eq!(rule.occurrences(start, date(2026, 1, 1), date(2026, 6, 14)), []);
    }

    #[test]
    fn rules_survive_being_stored_as_text() {
        for rule in [
            RecurrenceRule::new(Frequency::Once),
            RecurrenceRule::new(Frequency::Daily).every(3),
            RecurrenceRule::new(Frequency::Weekly).on_weekdays([0, 3, 6]).end(RecurrenceEnd::After(9)),
            RecurrenceRule::new(Frequency::Monthly).every(2).end(RecurrenceEnd::Until(date(2030, 12, 31))),
            RecurrenceRule::new(Frequency::Yearly),
        ] {
            assert_eq!(RecurrenceRule::parse(&rule.to_text()), Some(rule), "{rule}");
        }
        for broken in ["", "nope", "FREQ=SOMETIMES", "INTERVAL=2", "FREQ=DAILY;UNTIL=2026", "FREQ=DAILY;X=1"]
        {
            assert_eq!(RecurrenceRule::parse(broken), None, "{broken}");
        }
    }

    #[test]
    fn a_zero_interval_cannot_hang() {
        let mut rule = RecurrenceRule::new(Frequency::Daily);
        rule.interval = 0;
        let start = date(2026, 1, 1);
        assert_eq!(rule.occurrences(start, start, date(2026, 1, 3)).len(), 3);
    }
}
