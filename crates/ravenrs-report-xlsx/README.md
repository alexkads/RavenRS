# ravenrs-report-xlsx

XLSX and CSV export for [RavenRS Reporting](https://github.com/alexkads/RavenRS): lays a report `Document` out on a grid that follows the page, with real numbers and dates in the cells. It also runs in WebAssembly.

```rust
let xlsx = ravenrs_report_xlsx::to_xlsx(&document)?;
let csv = ravenrs_report_xlsx::to_csv(&document);
```

Licensed under MIT or Apache-2.0.
