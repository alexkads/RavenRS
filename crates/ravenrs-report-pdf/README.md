# ravenrs-report-pdf

PDF export for [RavenRS Reporting](https://github.com/alexkads/RavenRS): draws a report `Document` as vector pages, with subsets of its fonts embedded, in pure Rust. It also runs in WebAssembly.

```rust
let pdf = ravenrs_report_pdf::to_pdf(&document)?;
```

Licensed under MIT or Apache-2.0.
