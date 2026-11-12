# ravenrs-spreadsheet

A spreadsheet component for [RavenRS](https://github.com/alexkads/RavenRS): it opens, edits and
saves `.xlsx` workbooks, with formulas that recalculate, number formats, styles, several sheets,
undo and the clipboard, on the desktop and on the web.

```rust
use ravenrs_spreadsheet::{Spreadsheet, SpreadsheetState};

let sheet = SpreadsheetState::new();
sheet.set_cell("A1", "Total").unwrap();
sheet.set_cell("B1", "=SUM(B2:B10)").unwrap();
let view = Spreadsheet::new(&sheet);
```

The engine is [`ravenrs-sheet`](../ravenrs-sheet), which comes from GridCraft.

Licensed under MIT or Apache-2.0.
