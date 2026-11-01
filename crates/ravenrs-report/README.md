# ravenrs-report

The RavenRS Reporting engine: banded reports defined as data, paginated into documents for viewing and export. It has no GPU or window dependency, so it runs in desktop apps, in the browser and on servers.

This crate is part of [RavenRS](https://github.com/alexkads/RavenRS). See the [report designer](https://alexkads.github.io/RavenRS/designer/) for a live demo and [docs/REPORTING.md](https://github.com/alexkads/RavenRS/blob/master/docs/REPORTING.md) for the design.

```rust
use ravenrs_report::{Band, Control, Group, Report, ReportEngine};
use serde_json::json;

let report = Report::new("Sales")
    .culture("pt-BR")
    .group(
        Group::new("[Region]")
            .header(Band::new(22.0).control(Control::field("[Region]").at(0.0, 0.0, 300.0, 22.0)))
            .footer(Band::new(20.0).control(Control::field("Sum([Amount])").format("C2").at(0.0, 0.0, 300.0, 20.0))),
    )
    .detail(Band::new(18.0).control(Control::field("[Customer]").at(12.0, 0.0, 300.0, 18.0)));

let data = json!([
    { "Region": "Sul", "Customer": "Ana", "Amount": 10.5 },
    { "Region": "Sul", "Customer": "Carla", "Amount": 4.5 },
]);
let document = ReportEngine::new().render(&report, &data);
assert!(document.page_contains(0, "R$\u{a0}15,00"));
```

Reports also load from JSON with `Report::from_json`, data sources included, and `ravenrs_report::wizard` builds a finished table report from a choice of fields.

## License

MIT OR Apache-2.0.
