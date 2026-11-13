# ravenrs-widgets-scheduler

RavenRS time on screen: the `Scheduler`, a timeline of events by resource, the `EventCalendar` with its `RecurrenceEditor`, and the `Timeline`. The rules of recurrence themselves live in the base.

Applications do not depend on this crate. They use [`ravenrs-widgets`](https://crates.io/crates/ravenrs-widgets) or the [`ravenrs`](https://crates.io/crates/ravenrs) facade, which re-export every name of it by the same path. It is built on [`ravenrs-widgets-base`](https://crates.io/crates/ravenrs-widgets-base) and [`ravenrs-widgets-kit`](https://crates.io/crates/ravenrs-widgets-kit), and on no other family.

This crate is part of [RavenRS](https://github.com/alexkads/RavenRS).

## License

MIT OR Apache-2.0.
