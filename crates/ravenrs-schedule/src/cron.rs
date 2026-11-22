//! Cron schedules: five fields (minute, hour, day of month, month, day of week),
//! with `*`, lists, ranges, steps and English month and day names.
//!
//! As in standard cron, when both day fields are restricted a day matches if
//! either does: `0 8 13 * FRI` runs on every 13th and on every Friday.

use jiff::civil::Date;
use jiff::{ToSpan, Zoned};

/// How far ahead to look for a run; a schedule with none in this time never runs.
const SEARCH_DAYS: u32 = 366 * 8 + 2;

const MONTHS: [&str; 12] =
    ["JAN", "FEB", "MAR", "APR", "MAY", "JUN", "JUL", "AUG", "SEP", "OCT", "NOV", "DEC"];
const DAYS: [&str; 7] = ["SUN", "MON", "TUE", "WED", "THU", "FRI", "SAT"];

/// A parsed schedule.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Schedule {
    minutes: u64,
    hours: u32,
    days: u32,
    months: u16,
    weekdays: u8,
    any_day: bool,
    any_weekday: bool,
}

impl Schedule {
    /// Parses five cron fields.
    pub fn parse(text: &str) -> Result<Self, String> {
        let fields: Vec<&str> = text.split_whitespace().collect();
        let [minute, hour, day, month, weekday] = fields[..] else {
            return Err(format!("'{text}' must have five fields: minute hour day month weekday"));
        };
        // Sunday is both 0 and 7.
        let weekdays = field(weekday, 0, 7, &DAYS, 0)?;
        let weekdays = (weekdays & 0x7f) | (weekdays >> 7);
        Ok(Self {
            minutes: field(minute, 0, 59, &[], 0)?,
            hours: field(hour, 0, 23, &[], 0)? as u32,
            days: field(day, 1, 31, &[], 0)? as u32,
            months: field(month, 1, 12, &MONTHS, 1)? as u16,
            weekdays: weekdays as u8,
            any_day: day.starts_with('*'),
            any_weekday: weekday.starts_with('*'),
        })
    }

    fn matches_date(&self, date: Date) -> bool {
        if self.months & (1 << date.month()) == 0 {
            return false;
        }
        let day = self.days & (1 << date.day()) != 0;
        let weekday = self.weekdays & (1 << date.weekday().to_sunday_zero_offset()) != 0;
        match (self.any_day, self.any_weekday) {
            (false, false) => day || weekday,
            _ => day && weekday,
        }
    }

    /// The first run after `after`, in the same time zone.
    ///
    /// A local time that clocks skip over runs at the same moment shifted
    /// forward, and one that clocks repeat runs once, at its first occurrence.
    pub fn next_after(&self, after: &Zoned) -> Option<Zoned> {
        let zone = after.time_zone();
        let start = after.datetime();
        let mut date = start.date();
        for _ in 0..SEARCH_DAYS {
            if self.matches_date(date) {
                for hour in 0..24_i8 {
                    if self.hours & (1 << hour) == 0 {
                        continue;
                    }
                    for minute in 0..60_i8 {
                        if self.minutes & (1 << minute) == 0 {
                            continue;
                        }
                        let candidate = date.at(hour, minute, 0, 0);
                        if candidate < start.date().at(start.hour(), start.minute(), 0, 0) {
                            continue;
                        }
                        let zoned = zone.to_ambiguous_zoned(candidate).compatible().ok()?;
                        if zoned.timestamp() > after.timestamp() {
                            return Some(zoned);
                        }
                    }
                }
            }
            date = date.checked_add(1.day()).ok()?;
        }
        None
    }
}

/// One field as a bit set. `offset` is what the first name stands for.
fn field(text: &str, min: u8, max: u8, names: &[&str], offset: u8) -> Result<u64, String> {
    let mut bits = 0_u64;
    for part in text.split(',') {
        let (range, step) = match part.split_once('/') {
            Some((range, step)) => {
                let step: u8 = step.parse().map_err(|_| format!("bad step in '{part}'"))?;
                if step == 0 {
                    return Err(format!("the step in '{part}' cannot be zero"));
                }
                (range, step)
            }
            None => (part, 1),
        };
        let (low, high) = if range == "*" {
            (min, max)
        } else if let Some((low, high)) = range.split_once('-') {
            (value(low, names, offset)?, value(high, names, offset)?)
        } else {
            let low = value(range, names, offset)?;
            // `5/15` means from 5 to the end, every 15.
            (low, if part.contains('/') { max } else { low })
        };
        if low < min || high > max || low > high {
            return Err(format!("'{part}' is outside {min}-{max}"));
        }
        let mut current = low;
        while current <= high {
            bits |= 1 << current;
            match current.checked_add(step) {
                Some(next) => current = next,
                None => break,
            }
        }
    }
    Ok(bits)
}

fn value(text: &str, names: &[&str], offset: u8) -> Result<u8, String> {
    if let Ok(number) = text.parse() {
        return Ok(number);
    }
    let upper = text.to_ascii_uppercase();
    names
        .iter()
        .position(|name| *name == upper)
        .map(|i| i as u8 + offset)
        .ok_or_else(|| format!("'{text}' is not a value of this field"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(text: &str) -> Zoned {
        text.parse().unwrap_or_else(|e| panic!("{text}: {e}"))
    }

    fn next(schedule: &str, after: &str) -> String {
        let next = Schedule::parse(schedule).unwrap_or_else(|e| panic!("{e}")).next_after(&at(after));
        next.map_or("never".to_owned(), |z| z.strftime("%Y-%m-%d %H:%M %:z").to_string())
    }

    const RECIFE: &str = "[America/Recife]";

    #[test]
    fn simple_schedules() {
        let now = format!("2026-11-01T10:15:30-03:00{RECIFE}");
        assert_eq!(next("* * * * *", &now), "2026-11-01 10:16 -03:00");
        assert_eq!(next("0 8 * * *", &now), "2026-11-02 08:00 -03:00");
        assert_eq!(next("30 10 * * *", &now), "2026-11-01 10:30 -03:00");
        assert_eq!(next("*/20 * * * *", &now), "2026-11-01 10:20 -03:00");
        assert_eq!(next("0 8 1 * *", &now), "2026-12-01 08:00 -03:00");
    }

    #[test]
    fn a_run_at_this_exact_minute_is_not_repeated() {
        assert_eq!(
            next("15 10 * * *", &format!("2026-11-01T10:15:00-03:00{RECIFE}")),
            "2026-11-02 10:15 -03:00"
        );
        // But it still runs when the clock has not reached it.
        assert_eq!(
            next("15 10 * * *", &format!("2026-11-01T10:14:59-03:00{RECIFE}")),
            "2026-11-01 10:15 -03:00"
        );
    }

    #[test]
    fn names_ranges_lists_and_sunday_as_seven() {
        // 2026-11-01 is a Sunday.
        let now = format!("2026-11-01T10:00:00-03:00{RECIFE}");
        assert_eq!(next("0 8 * * MON", &now), "2026-11-02 08:00 -03:00");
        assert_eq!(next("0 8 * * mon-fri", &now), "2026-11-02 08:00 -03:00");
        assert_eq!(next("0 12 * * 7", &now), "2026-11-01 12:00 -03:00");
        assert_eq!(next("0 12 * * 0", &now), "2026-11-01 12:00 -03:00");
        assert_eq!(next("0 8 1 JAN,JUL *", &now), "2027-01-01 08:00 -03:00");
        assert_eq!(next("5/15 10 * * *", &now), "2026-11-01 10:05 -03:00");
    }

    #[test]
    fn restricted_day_fields_match_either() {
        // The 13th is a Friday in Nov 2026; the next Monday comes first.
        let now = format!("2026-11-03T10:00:00-03:00{RECIFE}");
        assert_eq!(next("0 8 13 * MON", &now), "2026-11-09 08:00 -03:00");
    }

    #[test]
    fn leap_days_and_impossible_dates() {
        let now = format!("2026-11-01T10:00:00-03:00{RECIFE}");
        assert_eq!(next("0 0 29 2 *", &now), "2028-02-29 00:00 -03:00");
        assert_eq!(next("0 0 31 2 *", &now), "never");
    }

    #[test]
    fn a_local_time_clocks_skip_runs_shifted_forward() {
        // On 2026-03-08 clocks in New York go from 02:00 to 03:00.
        let now = "2026-03-08T01:00:00-05:00[America/New_York]";
        assert_eq!(next("30 2 * * *", now), "2026-03-08 03:30 -04:00");
    }

    #[test]
    fn a_local_time_clocks_repeat_runs_once() {
        // On 2026-11-01 clocks in New York go from 02:00 back to 01:00.
        let first = "2026-11-01T00:00:00-04:00[America/New_York]";
        assert_eq!(next("30 1 * * *", first), "2026-11-01 01:30 -04:00");
        let after_first = "2026-11-01T01:30:00-04:00[America/New_York]";
        assert_eq!(next("30 1 * * *", after_first), "2026-11-02 01:30 -05:00");
    }

    #[test]
    fn bad_schedules_are_refused() {
        for bad in [
            "",
            "* * * *",
            "* * * * * *",
            "60 * * * *",
            "* 24 * * *",
            "* * 0 * *",
            "* * * 13 *",
            "*/0 * * * *",
            "5-1 * * * *",
            "x * * * *",
            "* * * * FUNDAY",
        ] {
            assert!(Schedule::parse(bad).is_err(), "{bad}");
        }
    }
}
