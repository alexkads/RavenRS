# ravenrs-report-export

One way to turn a [RavenRS](https://github.com/alexkads/RavenRS) report `Document` into a file: PDF, XLSX or CSV, with a safe name and its media type. Viewers, editors and servers all call it, so a report exports the same from anywhere.

```rust
let file = ravenrs_report_export::export(&document, ExportFormat::Pdf, None)?;
```

Licensed under MIT or Apache-2.0.
