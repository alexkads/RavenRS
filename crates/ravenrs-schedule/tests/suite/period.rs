//! Periods written as people write them, and a schedule in the zone of the machine.

use ravenrs_schedule::{SignedDuration, Timestamp, Trigger, parse_period};

#[test]
fn a_period_is_read_with_its_units_and_refused_without_one() {
    assert_eq!(parse_period("30s"), Ok(SignedDuration::from_secs(30)));
    assert_eq!(parse_period("1h30m"), Ok(SignedDuration::from_secs(5400)));
    assert_eq!(parse_period("500ms"), Ok(SignedDuration::from_millis(500)));
    assert_eq!(parse_period(" 5m "), Ok(SignedDuration::from_secs(300)));
    for wrong in ["10", "dez minutos", "", "0s", "-5m"] {
        assert!(parse_period(wrong).is_err(), "{wrong:?}");
    }
    assert!(parse_period("10").unwrap_err().contains("no unit"));
}

#[test]
fn a_local_schedule_comes_due_on_its_minute() {
    let trigger = Trigger::cron_local("*/15 * * * *").expect("a schedule");
    let now = Timestamp::from_second(1_700_000_000).expect("a time");
    let next = trigger.next_after(now).expect("a next run");
    let gap = next.duration_since(now);
    assert!(gap > SignedDuration::ZERO && gap <= SignedDuration::from_mins(15), "{gap:?}");
    assert!(Trigger::cron_local("sessenta").is_err());
}
