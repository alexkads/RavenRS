//! The time axis and how events share a row, with no UI.

use crate::date::Date;

const MINUTES_PER_DAY: i64 = 24 * 60;

/// A moment on a calendar, to the minute, with no time zone.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TimePoint(i64);

impl TimePoint {
    /// Minutes since the start of 1970-01-01.
    pub const fn from_minutes(minutes: i64) -> Self {
        Self(minutes)
    }

    /// The start of this day.
    pub fn from_date(date: Date) -> Self {
        Self(date.to_days() * MINUTES_PER_DAY)
    }

    /// This hour and minute of this day.
    pub fn at(date: Date, hour: u32, minute: u32) -> Self {
        Self(date.to_days() * MINUTES_PER_DAY + i64::from(hour) * 60 + i64::from(minute))
    }

    /// Minutes since the start of 1970-01-01.
    pub const fn minutes(self) -> i64 {
        self.0
    }

    /// Its day.
    pub fn date(self) -> Date {
        Date::from_days(self.0.div_euclid(MINUTES_PER_DAY))
    }

    /// Hour and minute of its day.
    pub fn hour_minute(self) -> (u32, u32) {
        let minutes = self.0.rem_euclid(MINUTES_PER_DAY);
        ((minutes / 60) as u32, (minutes % 60) as u32)
    }

    /// `minutes` later (earlier when negative).
    pub const fn plus_minutes(self, minutes: i64) -> Self {
        Self(self.0 + minutes)
    }

    /// `days` later (earlier when negative).
    pub const fn plus_days(self, days: i64) -> Self {
        Self(self.0 + days * MINUTES_PER_DAY)
    }
}

/// What one tick of a [`TimeAxis`] is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum TimeScale {
    /// An hour, under its day.
    Hours,
    /// A day, under its month.
    #[default]
    Days,
    /// A week, under its month.
    Weeks,
    /// A month, under its year.
    Months,
}

/// From far to near, as zooming in goes: a scale and the width of a tick.
const ZOOM_LEVELS: [(TimeScale, f32); 8] = [
    (TimeScale::Months, 56.0),
    (TimeScale::Months, 110.0),
    (TimeScale::Weeks, 60.0),
    (TimeScale::Weeks, 110.0),
    (TimeScale::Days, 40.0),
    (TimeScale::Days, 80.0),
    (TimeScale::Hours, 36.0),
    (TimeScale::Hours, 72.0),
];

/// The stretch of time a [`Scheduler`](crate::Scheduler) shows, how wide it is and how it is marked.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TimeAxis {
    /// Where it begins.
    pub start: TimePoint,
    /// Where it ends.
    pub end: TimePoint,
    /// What a tick is.
    pub scale: TimeScale,
    /// The width of a tick, in logical pixels. A month is 30 days wide, so all time runs at one speed.
    pub tick_width: f32,
}

/// A marked stretch of the axis, in pixels from its start.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Tick {
    pub x: f32,
    pub width: f32,
    pub start: TimePoint,
}

impl TimeAxis {
    /// `count` days from `from`, a tick per day.
    pub fn days(from: Date, count: u32) -> Self {
        let start = TimePoint::from_date(from);
        Self {
            start,
            end: start.plus_days(i64::from(count.max(1))),
            scale: TimeScale::Days,
            tick_width: 40.0,
        }
    }

    /// `count` weeks from the week of `from`, a tick per week.
    pub fn weeks(from: Date, count: u32) -> Self {
        let start = TimePoint::from_date(from.add_days(-i32::from(from.weekday())));
        let end = start.plus_days(7 * i64::from(count.max(1)));
        Self { start, end, scale: TimeScale::Weeks, tick_width: 60.0 }
    }

    /// `count` months from the month of `from`, a tick per month.
    pub fn months(from: Date, count: u32) -> Self {
        let first = from.first_of_month();
        let last = first.add_months(i32::try_from(count.max(1)).unwrap_or(1));
        Self {
            start: TimePoint::from_date(first),
            end: TimePoint::from_date(last),
            scale: TimeScale::Months,
            tick_width: 56.0,
        }
    }

    /// One day, a tick per hour.
    pub fn hours(day: Date) -> Self {
        let start = TimePoint::from_date(day);
        Self { start, end: start.plus_days(1), scale: TimeScale::Hours, tick_width: 36.0 }
    }

    /// The same stretch of time, one step nearer. `None` when it cannot go nearer.
    pub fn zoomed_in(&self) -> Option<Self> {
        self.zoomed(1)
    }

    /// The same stretch of time, one step farther. `None` when it cannot go farther.
    pub fn zoomed_out(&self) -> Option<Self> {
        self.zoomed(-1)
    }

    fn zoomed(&self, step: i32) -> Option<Self> {
        let here = ZOOM_LEVELS
            .iter()
            .rposition(|(scale, width)| *scale == self.scale && *width <= self.tick_width)
            .or_else(|| ZOOM_LEVELS.iter().position(|(scale, _)| *scale == self.scale))?;
        let there = i32::try_from(here).ok()? + step;
        let (scale, tick_width) = *ZOOM_LEVELS.get(usize::try_from(there).ok()?)?;
        Some(Self { scale, tick_width, ..*self })
    }

    /// The same length of time, moved by a whole `steps` of its length.
    pub fn shifted(&self, steps: i64) -> Self {
        let length = self.end.minutes() - self.start.minutes();
        match self.scale {
            TimeScale::Months => {
                let months = (length as f64 / (30.44 * MINUTES_PER_DAY as f64)).round() as i32;
                let by = i32::try_from(steps).unwrap_or(0) * months.max(1);
                Self {
                    start: TimePoint::from_date(self.start.date().add_months(by)),
                    end: TimePoint::from_date(self.end.date().add_months(by)),
                    ..*self
                }
            }
            _ => Self {
                start: self.start.plus_minutes(length * steps),
                end: self.end.plus_minutes(length * steps),
                ..*self
            },
        }
    }

    fn pixels_per_minute(&self) -> f64 {
        let tick_minutes = match self.scale {
            TimeScale::Hours => 60.0,
            TimeScale::Days => MINUTES_PER_DAY as f64,
            TimeScale::Weeks => 7.0 * MINUTES_PER_DAY as f64,
            TimeScale::Months => 30.0 * MINUTES_PER_DAY as f64,
        };
        f64::from(self.tick_width.max(1.0)) / tick_minutes
    }

    /// The whole width, in logical pixels.
    pub fn width(&self) -> f32 {
        self.x_of(self.end).max(0.0)
    }

    /// Where a moment falls, in logical pixels from the start.
    pub fn x_of(&self, time: TimePoint) -> f32 {
        ((time.minutes() - self.start.minutes()) as f64 * self.pixels_per_minute()) as f32
    }

    /// The moment at this position.
    pub fn time_at(&self, x: f32) -> TimePoint {
        let minutes = (f64::from(x) / self.pixels_per_minute()).round() as i64;
        self.start.plus_minutes(minutes)
    }

    /// The smallest step an event moves by: a quarter of an hour on the hour
    /// scale, a day on the others.
    pub fn snap_minutes(&self) -> i64 {
        match self.scale {
            TimeScale::Hours => 15,
            _ => MINUTES_PER_DAY,
        }
    }

    /// The nearest moment an event can start or end at.
    pub fn snap(&self, time: TimePoint) -> TimePoint {
        let step = self.snap_minutes();
        TimePoint::from_minutes((time.minutes() + step / 2).div_euclid(step) * step)
    }

    fn next_tick(&self, tick: TimePoint) -> TimePoint {
        match self.scale {
            TimeScale::Hours => tick.plus_minutes(60),
            TimeScale::Days => tick.plus_days(1),
            TimeScale::Weeks => tick.plus_days(7),
            TimeScale::Months => TimePoint::from_date(tick.date().first_of_month().add_months(1)),
        }
    }

    /// The ticks overlapping `from..to` pixels.
    pub(crate) fn ticks(&self, from: f32, to: f32) -> Vec<Tick> {
        let mut ticks = Vec::new();
        let mut tick = self.start;
        // Every step moves forward, so this stops at the end of the axis.
        while tick < self.end {
            let next = self.next_tick(tick).min(self.end);
            let (x, end) = (self.x_of(tick), self.x_of(next));
            if end > from && x < to {
                ticks.push(Tick { x, width: end - x, start: tick });
            }
            if x >= to {
                break;
            }
            tick = next;
        }
        ticks
    }

    /// Whether this tick falls on the top row's change: a new day, month or year.
    pub(crate) fn starts_group(&self, tick: TimePoint, previous: Option<TimePoint>) -> bool {
        let Some(previous) = previous else { return true };
        let (now, then) = (tick.date(), previous.date());
        match self.scale {
            TimeScale::Hours => now != then,
            TimeScale::Days | TimeScale::Weeks => now.month() != then.month() || now.year() != then.year(),
            TimeScale::Months => now.year() != then.year(),
        }
    }
}

/// Places overlapping stretches of time on lanes, the earliest first, so none
/// overlaps another on its lane. Returns each one's lane, in the order given, and
/// how many lanes there are; at least one.
pub(crate) fn assign_lanes(spans: &[(TimePoint, TimePoint)]) -> (Vec<usize>, usize) {
    let mut order: Vec<usize> = (0..spans.len()).collect();
    order.sort_by_key(|i| (spans[*i].0, spans[*i].1));
    let mut lanes = vec![0; spans.len()];
    let mut ends: Vec<TimePoint> = Vec::new();
    for i in order {
        let (start, end) = spans[i];
        // A moment with no length still takes room on its lane.
        let end = end.max(start.plus_minutes(1));
        match ends.iter().position(|free_at| *free_at <= start) {
            Some(lane) => {
                ends[lane] = end;
                lanes[i] = lane;
            }
            None => {
                lanes[i] = ends.len();
                ends.push(end);
            }
        }
    }
    (lanes, ends.len().max(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(y: i32, m: u8, d: u8) -> Date {
        Date::new(y, m, d).unwrap_or_else(|| panic!("invalid date"))
    }

    fn at(d: u8, h: u32) -> TimePoint {
        TimePoint::at(date(2026, 11, d), h, 0)
    }

    #[test]
    fn a_moment_knows_its_day_and_hour() {
        let time = TimePoint::at(date(1969, 12, 31), 23, 30);
        assert_eq!(time.date(), date(1969, 12, 31), "before 1970 too");
        assert_eq!(time.hour_minute(), (23, 30));
    }

    #[test]
    fn positions_and_moments_are_inverses() {
        let axis = TimeAxis::days(date(2026, 11, 1), 30);
        assert_eq!(axis.width(), 30.0 * 40.0);
        assert_eq!(axis.x_of(at(2, 0)), 40.0);
        assert_eq!(axis.x_of(at(2, 12)), 60.0, "time runs evenly through a day");
        assert_eq!(axis.time_at(60.0), at(2, 12));
    }

    #[test]
    fn events_snap_to_days_and_to_quarters_of_an_hour() {
        let days = TimeAxis::days(date(2026, 11, 1), 30);
        assert_eq!(days.snap(at(3, 11)), at(3, 0));
        assert_eq!(days.snap(at(3, 13)), at(4, 0));
        let hours = TimeAxis::hours(date(2026, 11, 3));
        assert_eq!(
            hours.snap(TimePoint::at(date(2026, 11, 3), 9, 7)),
            TimePoint::at(date(2026, 11, 3), 9, 0)
        );
        assert_eq!(
            hours.snap(TimePoint::at(date(2026, 11, 3), 9, 8)),
            TimePoint::at(date(2026, 11, 3), 9, 15)
        );
    }

    #[test]
    fn months_have_their_own_lengths() {
        let axis = TimeAxis::months(date(2026, 2, 15), 2);
        let ticks = axis.ticks(0.0, axis.width());
        assert_eq!(ticks.len(), 2);
        assert_eq!(ticks[0].start.date(), date(2026, 2, 1));
        assert_eq!(ticks[1].start.date(), date(2026, 3, 1));
        assert!(ticks[0].width < ticks[1].width, "February is shorter than March");
    }

    #[test]
    fn only_the_ticks_in_view_are_made() {
        let axis = TimeAxis::days(date(2026, 1, 1), 365);
        let ticks = axis.ticks(400.0, 500.0);
        assert_eq!(ticks.len(), 3, "from the 11th to the 13th");
        assert_eq!(ticks[0].x, 400.0);
    }

    #[test]
    fn weeks_start_on_sunday() {
        // November 4, 2026 is a Wednesday.
        let axis = TimeAxis::weeks(date(2026, 11, 4), 2);
        assert_eq!(axis.start.date(), date(2026, 11, 1));
        assert_eq!(axis.end.date(), date(2026, 11, 15));
    }

    #[test]
    fn zoom_walks_through_the_levels_and_stops_at_the_ends() {
        let mut axis = TimeAxis::months(date(2026, 1, 1), 12);
        let range = (axis.start, axis.end);
        let mut steps = 0;
        while let Some(nearer) = axis.zoomed_in() {
            assert!(nearer.width() > axis.width(), "nearer is always wider");
            assert_eq!((nearer.start, nearer.end), range, "over the same time");
            axis = nearer;
            steps += 1;
        }
        assert_eq!(steps, ZOOM_LEVELS.len() - 1);
        assert_eq!(axis.scale, TimeScale::Hours);
        while let Some(farther) = axis.zoomed_out() {
            axis = farther;
        }
        assert_eq!(axis.scale, TimeScale::Months);
    }

    #[test]
    fn an_axis_moves_by_its_own_length() {
        let days = TimeAxis::days(date(2026, 11, 1), 7).shifted(1);
        assert_eq!(days.start.date(), date(2026, 11, 8));
        let months = TimeAxis::months(date(2026, 11, 1), 3).shifted(-1);
        assert_eq!(months.start.date(), date(2026, 8, 1));
        assert_eq!(months.end.date(), date(2026, 11, 1));
    }

    #[test]
    fn overlapping_events_take_separate_lanes() {
        let spans = [(at(1, 0), at(3, 0)), (at(2, 0), at(4, 0)), (at(3, 0), at(5, 0)), (at(1, 0), at(6, 0))];
        let (lanes, count) = assign_lanes(&spans);
        assert_eq!(count, 3);
        assert_ne!(lanes[0], lanes[1]);
        assert_ne!(lanes[0], lanes[3]);
        assert_eq!(lanes[2], lanes[0], "the third starts as the first ends, so it can reuse its lane");
    }

    #[test]
    fn a_row_with_nothing_still_has_a_lane() {
        assert_eq!(assign_lanes(&[]), (vec![], 1));
    }

    #[test]
    fn moments_with_no_length_do_not_land_on_each_other() {
        let (lanes, count) = assign_lanes(&[(at(1, 0), at(1, 0)), (at(1, 0), at(1, 0))]);
        assert_eq!(count, 2);
        assert_ne!(lanes[0], lanes[1]);
    }
}
