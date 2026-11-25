# Grid

Equal-width columns that wrap into rows.

**Family:** Layout · **Rust:** `ravenrs::prelude::Grid`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `spacing` | number 0–32 by 0.1 | `8` |  |

## Stories

- **Example**: 728 × 116 pt, settings `{"spacing":8.0}`
- **Wide spacing**: 728 × 132 pt, settings `{"spacing":24.0}`

## Sizing

Fills the width of its parent: 288, 728, 1168 pt wide in parents of 320, 760, 1200 pt.

## Behavior

- **Accessibility**: role `Label` "1".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Label "1" | 205.0 × 22.0 | — | — | — | Inter 400 15.0px ink | — | 0.0 |

## API

Builder methods: `new`, `spacing`.

`GridExportFormat`: `Csv`, `Pdf`, `Xlsx`.

`GridTableKind`: `Boolean`, `Date`, `Number`, `Text`.

## Example

```rust
fn grid() -> impl View {
    let cell = |n: u32| Card::new(Text::new(n.to_string())).padding(16.0);
    Grid::new(3, (cell(1), cell(2), cell(3), cell(4), cell(5), cell(6))).spacing(8.0)
}
```

Every number above is in `Grid.json` beside this card.
