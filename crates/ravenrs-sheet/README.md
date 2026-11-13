# ravenrs-sheet

Headless spreadsheet engine for [RavenRS](https://github.com/alexkads/RavenRS): cells and styles,
a formula language with more than 500 functions and dynamic arrays, incremental recalculation,
number formats, XLSX, XLS (read), CSV and SYLK reading and writing, workbooks protected by a
password, charts, and sessions in which every action is a
command with undo. Nothing here draws, so it runs on servers and in WebAssembly too.
`ravenrs-spreadsheet` puts a `Spreadsheet` component on top of it.

```rust
use ravenrs_sheet::engine::Session;

let mut session = Session::new();
session.new_workbook();
session.execute("cell.set", serde_json::json!({ "cell": "A1", "input": "=SUM(1,2)" }))?;
```

The engine comes from [GridCraft](https://github.com/storytold/gridcraft), and its file formats
from [craft-libs](https://github.com/storytold/craft-libs); see `NOTICE`. Every input has a work
limit, so a hostile file or formula stops with an error instead of hanging.

Licensed under MIT or Apache-2.0.
