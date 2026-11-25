# Table

Simple table with a header, hover rows and numeric columns.

**Family:** Data · **Rust:** `ravenrs::prelude::Table`

## Stories

- **Example**: 728 × 130 pt, settings `{}`

## Sizing

Fills the width of its parent: 288, 728, 1168 pt wide in parents of 320, 760, 1200 pt; its height follows the width, as its content wraps.

## Behavior

- **Accessibility**: role `Table`.

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Row | 728.0 × 36.0 | 0.0 | transparent | — | Inter 600 12.0px ink-muted | — | space-4 |
| Example | Row | 728.0 × 46.0 | 0.0 | transparent | — | Inter 600 15.0px ink | — | space-4 |

## API

Builder methods: `new`, `row`.

## Example

```rust
fn table() -> impl View {
    Table::new([Column::new("Invoice"), Column::new("Customer").weight(2.0), Column::new("Amount").numeric()])
        .row(["INV-1042", "Northwind Labs", "R$ 4.800"])
        .row(["INV-1043", "Aurora Studio", "R$ 1.720"])
}
```

Every number above is in `Table.json` beside this card.
