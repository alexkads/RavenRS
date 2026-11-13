//! How something repeats: a rule, with no UI, that answers on which days.

use std::fmt;

use crate::date::{Date, days_in_month};
use crate::labels::Labels;

/// How often a [`RecurrenceRule`] comes back.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "variants"))]
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

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl Frequency {
    /// Every frequency, in the order an editor offers them.
    pub const ALL: [Self; 5] = [Self::Once, Self::Daily, Self::Weekly, Self::Monthly, Self::Yearly];
}

/// When a [`RecurrenceRule`] stops.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "variants"))]
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
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
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

#[cfg_attr(feature = "live", ravenrs_live::expose)]
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

/// A [`RecurrenceRule`] with the days taken out of it, iCalendar's `RRULE` with its
/// `EXDATE`s: a series where some occurrences were cancelled or moved elsewhere. An
/// exception still counts toward the rule's `COUNT`, so taking one out never brings
/// another day in at the end.
///
/// ```
/// use ravenrs_widgets::{Date, Frequency, RecurrenceRule, RecurrenceSet};
///
/// let start = Date::new(2026, 11, 4).unwrap();
/// let series = RecurrenceSet::new(RecurrenceRule::new(Frequency::Weekly))
///     .except(Date::new(2026, 11, 11).unwrap());
/// let days = series.occurrences(start, start, Date::new(2026, 11, 18).unwrap());
/// assert_eq!(days, [start, Date::new(2026, 11, 18).unwrap()]);
/// assert_eq!(series.to_text(), "RRULE:FREQ=WEEKLY\nEXDATE;VALUE=DATE:20261111");
/// assert_eq!(RecurrenceSet::parse(&series.to_text()), Some(series));
/// ```
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct RecurrenceSet {
    /// How it repeats.
    pub rule: RecurrenceRule,
    /// The days taken out, in order and without repeats.
    pub exceptions: Vec<Date>,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl RecurrenceSet {
    /// A series of this rule with nothing taken out.
    pub fn new(rule: RecurrenceRule) -> Self {
        Self { rule, exceptions: Vec::new() }
    }

    /// Takes this day out of the series.
    pub fn except(mut self, day: Date) -> Self {
        self.add_exception(day);
        self
    }

    /// Takes this day out of the series, in place; a day already out stays out once.
    pub fn add_exception(&mut self, day: Date) {
        if let Err(at) = self.exceptions.binary_search(&day) {
            self.exceptions.insert(at, day);
        }
    }

    /// Whether this day was taken out.
    pub fn is_exception(&self, day: Date) -> bool {
        self.exceptions.binary_search(&day).is_ok()
    }

    /// The days of the series starting at `start` that fall in `from..=to`, without the
    /// exceptions, in order.
    pub fn occurrences(&self, start: Date, from: Date, to: Date) -> Vec<Date> {
        let mut days = self.rule.occurrences(start, from, to);
        days.retain(|day| !self.is_exception(*day));
        days
    }

    /// The series as iCalendar text: the bare rule when nothing was taken out, and an
    /// `RRULE` line followed by an `EXDATE` line when something was.
    pub fn to_text(&self) -> String {
        if self.exceptions.is_empty() {
            return self.rule.to_text();
        }
        format!("RRULE:{}\nEXDATE;VALUE=DATE:{}", self.rule.to_text(), exceptions_text(&self.exceptions))
    }

    /// Reads what [`Self::to_text`] writes, or a bare rule; `None` when it is neither.
    pub fn parse(text: &str) -> Option<Self> {
        let mut set: Option<Self> = None;
        let mut exceptions = Vec::new();
        for line in text.lines().map(str::trim).filter(|line| !line.is_empty()) {
            if let Some(rule) = line.strip_prefix("RRULE:") {
                set = Some(Self::new(RecurrenceRule::parse(rule)?));
            } else if line.starts_with("EXDATE") {
                let (_, days) = line.split_once(':')?;
                exceptions.extend(parse_exceptions(days)?);
            } else {
                set = Some(Self::new(RecurrenceRule::parse(line)?));
            }
        }
        let mut set = set?;
        for day in exceptions {
            set.add_exception(day);
        }
        Some(set)
    }
}

impl fmt::Display for RecurrenceSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_text())
    }
}

/// Days as the value of an iCalendar `EXDATE`: `20261111,20261118`.
pub fn exceptions_text(days: &[Date]) -> String {
    days.iter()
        .map(|day| format!("{:04}{:02}{:02}", day.year(), day.month(), day.day()))
        .collect::<Vec<_>>()
        .join(",")
}

/// Reads days written by [`exceptions_text`], also as `2026-11-11`, and ignores a time
/// after the day (`20261111T090000`); `None` when one of them is not a day. Empty text is
/// no day at all.
pub fn parse_exceptions(text: &str) -> Option<Vec<Date>> {
    text.split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(|part| {
            let digits: String = part.split('T').next()?.chars().filter(|c| *c != '-').collect();
            if digits.len() != 8 {
                return None;
            }
            Date::new(
                digits.get(..4)?.parse().ok()?,
                digits.get(4..6)?.parse().ok()?,
                digits.get(6..8)?.parse().ok()?,
            )
        })
        .collect()
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

// The words for a rule live with the rule, so `labels` does not depend on `recurrence`.
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "part2"))]
impl Labels {
    /// The word for one period of this frequency; `count` picks singular or plural.
    pub fn recurrence_unit(&self, frequency: Frequency, count: u32) -> &str {
        let unit = match frequency {
            Frequency::Daily => &self.recurrence_units[0],
            Frequency::Weekly => &self.recurrence_units[1],
            Frequency::Yearly => &self.recurrence_units[3],
            _ => &self.recurrence_units[2],
        };
        if count > 1 { &unit.1 } else { &unit.0 }
    }

    /// A rule in words, with its next days: `Every month, 3 times. Next: 05/11/2026, 05/12/2026`.
    pub fn recurrence_summary(&self, rule: &RecurrenceRule, start: Date) -> String {
        let how = match (rule.frequency, rule.interval.max(1)) {
            (Frequency::Daily, 1) => self.recurrence_each[0].clone(),
            (Frequency::Weekly, 1) => self.recurrence_each[1].clone(),
            (Frequency::Monthly, 1) => self.recurrence_each[2].clone(),
            (Frequency::Yearly, 1) => self.recurrence_each[3].clone(),
            (Frequency::Daily | Frequency::Weekly | Frequency::Monthly | Frequency::Yearly, n) => {
                format!("{} {n} {}", self.recurrence_every, self.recurrence_unit(rule.frequency, n))
            }
            _ => self.recurrence_frequencies[0].clone(),
        };
        let until = match rule.end {
            RecurrenceEnd::Until(day) => format!(" {} {}", self.recurrence_until, self.short_date(day)),
            RecurrenceEnd::After(count) => format!(", {count} {}", self.recurrence_times),
            _ => String::new(),
        };
        let next: Vec<String> = rule.days_from(start).take(3).map(|day| self.short_date(day)).collect();
        format!("{how}{until}. {}: {}", self.recurrence_next, next.join(", "))
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
    #[test]
    fn an_exception_takes_one_day_out_and_still_counts() {
        let start = date(2026, 11, 4);
        let rule = RecurrenceRule::new(Frequency::Weekly).end(RecurrenceEnd::After(3));
        let series = RecurrenceSet::new(rule).except(date(2026, 11, 11));
        assert_eq!(
            series.occurrences(start, start, date(2026, 12, 31)),
            [date(2026, 11, 4), date(2026, 11, 18)],
            "the third is still the last: COUNT counts the day taken out"
        );
        assert!(series.is_exception(date(2026, 11, 11)));
        assert!(!series.is_exception(date(2026, 11, 18)));
    }

    #[test]
    fn exceptions_stay_in_order_and_once() {
        let mut series = RecurrenceSet::new(RecurrenceRule::new(Frequency::Daily));
        for day in [date(2026, 1, 9), date(2026, 1, 3), date(2026, 1, 9)] {
            series.add_exception(day);
        }
        assert_eq!(series.exceptions, [date(2026, 1, 3), date(2026, 1, 9)]);
    }

    #[test]
    fn a_series_survives_being_stored_as_text() {
        let rule = RecurrenceRule::new(Frequency::Weekly).on_weekdays([1, 3]).end(RecurrenceEnd::After(9));
        let plain = RecurrenceSet::new(rule);
        assert_eq!(plain.to_text(), rule.to_text(), "with nothing taken out it is the bare rule");
        let series = plain.clone().except(date(2026, 11, 9)).except(date(2026, 11, 16));
        assert_eq!(
            series.to_text(),
            "RRULE:FREQ=WEEKLY;BYDAY=MO,WE;COUNT=9\nEXDATE;VALUE=DATE:20261109,20261116"
        );
        for set in [plain, series] {
            assert_eq!(RecurrenceSet::parse(&set.to_text()), Some(set.clone()), "{set}");
        }
        for broken in ["", "EXDATE:20261109", "RRULE:FREQ=NEVER", "RRULE:FREQ=DAILY\nEXDATE:2026"] {
            assert_eq!(RecurrenceSet::parse(broken), None, "{broken}");
        }
    }

    #[test]
    fn exception_days_read_in_both_ways_of_writing_them() {
        assert_eq!(
            parse_exceptions("20261109, 2026-11-16,20261123T090000"),
            Some(vec![date(2026, 11, 9), date(2026, 11, 16), date(2026, 11, 23)])
        );
        assert_eq!(parse_exceptions(""), Some(Vec::new()));
        assert_eq!(parse_exceptions("20261332"), None);
        assert_eq!(exceptions_text(&[date(2026, 1, 2), date(2027, 3, 4)]), "20260102,20270304");
    }
}
