use ravenrs_schedule::{Check, Misfire, SignedDuration, Trigger, check};

use crate::at;

fn hourly() -> Trigger {
    Trigger::cron("0 * * * *", "UTC").unwrap_or_else(|e| panic!("{e}"))
}

const WITHIN_HOUR: Misfire = Misfire::Within(SignedDuration::from_mins(60));

#[test]
fn the_first_pass_only_seeds() {
    let first = check(&hourly(), WITHIN_HOUR, None, at("2026-11-22T09:10:00Z"));
    assert_eq!(first, Check::Seeded { next: Some(at("2026-11-22T10:00:00Z")) });
}

#[test]
fn before_the_hour_it_waits_and_says_until_when() {
    let wait = check(&hourly(), WITHIN_HOUR, Some(at("2026-11-22T10:00:00Z")), at("2026-11-22T09:59:59Z"));
    assert_eq!(wait, Check::Wait { until: at("2026-11-22T10:00:00Z") });
}

#[test]
fn on_the_hour_it_runs_and_gives_the_next() {
    let run = check(&hourly(), WITHIN_HOUR, Some(at("2026-11-22T10:00:00Z")), at("2026-11-22T10:00:05Z"));
    assert_eq!(run, Check::Run { due: at("2026-11-22T10:00:00Z"), next: Some(at("2026-11-22T11:00:00Z")) });
}

/// Stopped from 09:00 to 11:00: the 09:00 and 10:00 runs came due with nobody to run them.
#[test]
fn many_missed_runs_become_one() {
    let stored = Some(at("2026-11-22T09:00:00Z"));
    let back = at("2026-11-22T11:00:30Z");
    let next = Some(at("2026-11-22T12:00:00Z"));
    // Two hours late is more than an hour of grace: one miss, and the next is counted from now.
    assert_eq!(
        check(&hourly(), WITHIN_HOUR, stored, back),
        Check::Missed { due: at("2026-11-22T09:00:00Z"), next }
    );
    // Always runs it, once.
    assert_eq!(
        check(&hourly(), Misfire::Always, stored, back),
        Check::Run { due: at("2026-11-22T09:00:00Z"), next }
    );
}

#[test]
fn within_runs_a_late_one_that_fits_the_grace() {
    let late = check(&hourly(), WITHIN_HOUR, Some(at("2026-11-22T10:00:00Z")), at("2026-11-22T10:30:00Z"));
    assert_eq!(late, Check::Run { due: at("2026-11-22T10:00:00Z"), next: Some(at("2026-11-22T11:00:00Z")) });
    let five = Misfire::Within(SignedDuration::from_mins(5));
    let missed = check(&hourly(), five, Some(at("2026-11-22T10:00:00Z")), at("2026-11-22T10:30:00Z"));
    assert!(matches!(missed, Check::Missed { .. }), "{missed:?}");
}

#[test]
fn a_trigger_with_no_run_left_is_never() {
    // 31 February never comes.
    let never = Trigger::cron("0 0 31 2 *", "UTC").unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(check(&never, Misfire::Always, None, at("2026-11-22T10:00:00Z")), Check::Never);
}

#[test]
fn a_period_works_like_a_schedule() {
    let every = Trigger::every(SignedDuration::from_mins(15), at("2026-11-22T10:00:00Z"))
        .unwrap_or_else(|e| panic!("{e}"));
    let run = check(&every, Misfire::Always, Some(at("2026-11-22T10:15:00Z")), at("2026-11-22T10:15:01Z"));
    assert_eq!(run, Check::Run { due: at("2026-11-22T10:15:00Z"), next: Some(at("2026-11-22T10:30:00Z")) });
}
