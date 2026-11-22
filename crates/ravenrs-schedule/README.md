# ravenrs-schedule

Decides when a job runs, for RavenRS apps and servers. It has no runtime of its own: the report server calls it from a tokio timer, an app from `cx.wake_at`, and a test with a fixed time. The report server uses it today. The app framework's scheduled tasks (#66), the Raven Flows triggers (#537) and the Raven CI `cron` plugin (#803) are to use the same engine instead of copying it.

## What it does

| Piece | What it does |
|---|---|
| `cron::Schedule` | Five cron fields (minute, hour, day of month, month, weekday) with `*`, lists, ranges, steps and English names. Restricting both day fields matches either, as in standard cron. |
| `Trigger::cron` | A schedule read in a time zone, so 09:00 stays 09:00 local across clock changes. A local time the clock skips runs shifted forward; one the clock repeats runs once. |
| `Trigger::every` | A fixed period counted from a start. Zero or negative is refused. |
| `check` | One pass: from the trigger, the stored next run and the time now, returns `Seeded`, `Wait { until }`, `Run { due, next }`, `Missed { due, next }` or `Never`. |
| `Misfire` | A run that came due while the program was down: `Within(grace)` runs it if it is at most that late and records a miss otherwise; `Always` runs it once. However many were missed, at most one happens. |
| `Running` | Keeps one key from running twice at once. The key is free again when the guard drops, also if the run panics. |

## The one rule for callers

Store the `next` that `check` returns **before** starting the run. A crash right after then finds the next occurrence stored and does not run the same one twice.

## Example

```rust
use ravenrs_schedule::{Check, Misfire, SignedDuration, Trigger, check};

let trigger = Trigger::cron("0 9 * * MON-FRI", "America/Sao_Paulo")?;
let mut stored = None;
let now = "2026-11-23T12:00:30Z".parse()?;
match check(&trigger, Misfire::Within(SignedDuration::from_mins(60)), stored, now) {
    Check::Seeded { next } => stored = next,
    Check::Wait { until } => println!("wake at {until}"),
    Check::Run { due, next } => {
        stored = next;
        println!("run the one due at {due}");
    }
    Check::Missed { due, next } => {
        stored = next;
        println!("record that {due} was missed");
    }
    Check::Never => {}
}
# let _ = stored;
# Ok::<(), Box<dyn std::error::Error>>(())
```
