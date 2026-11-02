# ravenrs-grid-export

Turns what a [RavenRS](https://github.com/alexkads/RavenRS) `DataGrid` shows into a file: Excel, PDF or CSV. The rows go through the same report engine and exporters as any report, so numbers and dates stay real in the workbook and the PDF has pages, headers and totals.

```rust,ignore
DataGrid::new(data, columns)
    .export_title("Accounts")
    .on_export_file(|format, table| {
        if let Ok(file) = ravenrs_grid_export::export(&table, format, None) {
            // Save `file.bytes` under `file.name`.
        }
    })
```

`ravenrs-widgets` knows nothing of reports, and reports know nothing of widgets. This crate is what joins them.
