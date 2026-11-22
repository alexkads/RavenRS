use ravenrs_schedule::{SignedDuration, Trigger};

use crate::at;

#[test]
fn cron_hours_stay_local_in_their_zone() {
    let nine = Trigger::cron("0 9 * * *", "America/Sao_Paulo").unwrap_or_else(|e| panic!("{e}"));
    // 13:00 UTC is 10:00 in São Paulo, so the next 09:00 local is tomorrow at 12:00 UTC.
    assert_eq!(nine.next_after(at("2026-11-22T13:00:00Z")), Some(at("2026-11-23T12:00:00Z")));
}

#[test]
fn a_bad_cron_or_zone_is_an_error_not_a_panic() {
    assert!(Trigger::cron("61 * * * *", "UTC").is_err());
    let Err(zone) = Trigger::cron("0 9 * * *", "America/Atlantida") else {
        panic!("an unknown zone was accepted")
    };
    assert!(zone.contains("America/Atlantida"), "{zone}");
}

#[test]
fn every_counts_from_its_start() {
    let quarter = Trigger::every(SignedDuration::from_mins(15), at("2026-11-22T10:00:00Z"))
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(quarter.next_after(at("2026-11-22T09:00:00Z")), Some(at("2026-11-22T10:15:00Z")));
    assert_eq!(quarter.next_after(at("2026-11-22T10:00:00Z")), Some(at("2026-11-22T10:15:00Z")));
    assert_eq!(quarter.next_after(at("2026-11-22T10:15:00Z")), Some(at("2026-11-22T10:30:00Z")));
    assert_eq!(quarter.next_after(at("2026-11-22T10:29:59Z")), Some(at("2026-11-22T10:30:00Z")));
    assert_eq!(quarter.next_after(at("2026-11-23T10:00:01Z")), Some(at("2026-11-23T10:15:00Z")));
}

#[test]
fn every_refuses_a_period_that_is_not_positive() {
    let start = at("2026-11-22T10:00:00Z");
    assert!(Trigger::every(SignedDuration::ZERO, start).is_err());
    assert!(Trigger::every(SignedDuration::from_secs(-60), start).is_err());
}
