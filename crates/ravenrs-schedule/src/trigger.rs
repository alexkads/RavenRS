//! When a job comes due: the trigger, the decision of each pass and what a late run does.

use jiff::tz::TimeZone;
use jiff::{SignedDuration, Timestamp};

use crate::cron::Schedule;

/// When a job runs.
#[derive(Clone, Debug, PartialEq)]
pub enum Trigger {
    /// On a cron schedule, read in a time zone, so 09:00 stays 09:00 local across clock changes.
    Cron {
        /// The parsed schedule.
        schedule: Schedule,
        /// The zone its hours are in.
        zone: TimeZone,
    },
    /// Every `period`, counted from `from`: `from + period`, `from + 2 × period`…
    Every {
        /// The time between runs; always positive.
        period: SignedDuration,
        /// Where the count starts. It is not a run itself.
        from: Timestamp,
    },
}

impl Trigger {
    /// A cron schedule in the zone named `zone`, as `"America/Sao_Paulo"` or `"UTC"`.
    pub fn cron(schedule: &str, zone: &str) -> Result<Self, String> {
        let schedule = Schedule::parse(schedule)?;
        let zone = TimeZone::get(zone).map_err(|e| format!("'{zone}' is not a time zone: {e}"))?;
        Ok(Self::Cron { schedule, zone })
    }

    /// Every `period` from `from`. A period that is zero or negative is refused.
    pub fn every(period: SignedDuration, from: Timestamp) -> Result<Self, String> {
        if period <= SignedDuration::ZERO {
            return Err(format!("the period must be positive, not {period:#}"));
        }
        Ok(Self::Every { period, from })
    }

    /// The first run strictly after `after`, or `None` when there is none.
    pub fn next_after(&self, after: Timestamp) -> Option<Timestamp> {
        match self {
            Self::Cron { schedule, zone } => {
                schedule.next_after(&after.to_zoned(zone.clone())).map(|z| z.timestamp())
            }
            Self::Every { period, from } => {
                if after < *from {
                    return from.checked_add(*period).ok();
                }
                let elapsed = after.duration_since(*from).as_nanos();
                let step = period.as_nanos();
                let done = elapsed / step + 1;
                let ahead = i64::try_from(done.checked_mul(step)?).ok()?;
                from.checked_add(SignedDuration::from_nanos(ahead)).ok()
            }
        }
    }
}

/// What to do with a run that came due while the program was not running, or was busy.
///
/// However many runs were missed, at most one happens: the next run is always counted from now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Misfire {
    /// Run it if it is at most this late; later than that, it is recorded as missed.
    Within(SignedDuration),
    /// Run it once, however late.
    Always,
}

/// The decision of one pass of [`check`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Check {
    /// Nothing was stored yet: store `next` and run nothing, so a new job does not run at once.
    Seeded {
        /// The first run.
        next: Option<Timestamp>,
    },
    /// Not due yet: wake again at `until`.
    Wait {
        /// The stored next run.
        until: Timestamp,
    },
    /// Due: store `next`, then run the one that was due at `due`.
    Run {
        /// When this run was due.
        due: Timestamp,
        /// The run after it, to store before starting.
        next: Option<Timestamp>,
    },
    /// Due, but too late to run under [`Misfire::Within`]: store `next` and record a miss.
    Missed {
        /// When the missed run was due.
        due: Timestamp,
        /// The run after it, to store.
        next: Option<Timestamp>,
    },
    /// The trigger has no run left: there is nothing to store or wake for.
    Never,
}

/// Decides one pass for a job, from its `trigger`, the stored `next_run` and the time `now`.
///
/// The caller stores the `next` of [`Check::Seeded`], [`Check::Run`] and [`Check::Missed`] before
/// doing anything else, so a crash right after cannot run the same occurrence twice.
pub fn check(trigger: &Trigger, misfire: Misfire, next_run: Option<Timestamp>, now: Timestamp) -> Check {
    let next = trigger.next_after(now);
    let Some(due) = next_run else {
        return match next {
            Some(_) => Check::Seeded { next },
            None => Check::Never,
        };
    };
    if due > now {
        return Check::Wait { until: due };
    }
    let late = now.duration_since(due);
    match misfire {
        Misfire::Within(grace) if late > grace => Check::Missed { due, next },
        _ => Check::Run { due, next },
    }
}
