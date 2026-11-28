# FilteredGrid

A list of records: the page header, a bar with search and filters, and the grid under it, filling the width.

**Family:** Patterns · **Rust:** `ravenrs::prelude::FilteredGrid`

## Stories

- **Pattern**: 728 × 462 pt, settings `{}`

## Sizing

Fills the width of its parent: 632, 728, 1168 pt wide in parents of 320, 760, 1200 pt; in a parent of 320 pt it narrows to 632 pt.

## Behavior

- **Accessibility**: role `Heading` "Invoices".
- **Focus order** (Tab): TextInput "Search invoices" → ComboBox "Status" → Button "Export" → TextInput "Search all columns" → Button "Group" → Button "Columns" → TextInput "Filter Invoice" → TextInput "Filter Customer" → TextInput "Filter Amount" → Grid "Data grid".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Pattern | Heading "Invoices" | 89.0 × 28.0 | — | — | — | Inter 600 22.0px ink | — | 0.0 |
| Pattern | Label "Sent this month." | 100.0 × 18.0 | — | — | — | Inter 400 13.0px ink-muted | — | 0.0 |
| Pattern | TextInput "Search invoices" | 280.0 × 40.0 | radius-md | bg-300 | 1.0 control-border | Inter 400 15.0px ink-muted | search 16.0px ink-muted | space-3 |
| Pattern | ComboBox "Status" | 220.0 × 40.0 | radius-md | bg-300 | 1.0 control-border | Inter 400 15.0px ink | chevron-down 16.0px ink-muted | space-3 |
| Pattern | Button "Export" | 108.0 × 40.0 | radius-md | bg-300 | 1.0 border | Inter 600 15.0px ink | download 16.0px ink | 18.0 |
| Pattern | TextInput "Search all columns" | 240.0 × 32.0 | radius-md | bg-300 | 1.0 control-border | Inter 400 13.0px ink-muted | search 16.0px ink-muted | space-2 |
| Pattern | Button "Group" | 107.0 × 32.0 | radius-md | bg-300 | 1.0 border | Inter 600 13.0px ink | layers 14.0px ink | space-3 |
| Pattern | Button "Columns" | 124.0 × 32.0 | radius-md | bg-300 | 1.0 border | Inter 600 13.0px ink | table 14.0px ink | space-3 |
| Pattern | ColumnHeader "Invoice" | 134.0 × 38.0 | — | — | — | Inter 600 12.0px ink-muted | — | space-3 |
| Pattern | ColumnHeader "Amount" | 154.0 × 38.0 | — | — | — | Inter 600 12.0px ink-muted | — | 101.0 |
| Pattern | TextInput "Filter Invoice" | 140.0 × 38.0 | radius-sm | transparent | — | Inter 400 13.0px ink-muted | — | space-3 |
| Pattern | TextInput "Filter Amount" | 160.0 × 38.0 | radius-sm | transparent | — | Inter 400 13.0px ink-muted | — | 91.0 |
| Pattern | Row | 540.0 × 36.0 | 0.0 | transparent | — | Inter 400 13.0px ink | — | space-3 |
| Pattern | Row | 540.0 × 36.0 | 0.0 | bg-300@40 | — | Inter 400 13.0px ink | — | space-3 |
| Pattern | Row | 540.0 × 36.0 | — | — | — | — | — | — |
| Pattern | Button "Edit summary Invoice" | 140.0 × 40.0 | 0.0 | transparent | — | Inter 400 13.0px ink | — | space-3 |
| Pattern | Button "Edit summary Amount" | 160.0 × 40.0 | 0.0 | transparent | — | Inter 400 13.0px ink | — | 84.0 |

Every number above is in `FilteredGrid.json` beside this card.
