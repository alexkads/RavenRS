# TileLayout

A dashboard of tiles to drag and resize.

**Family:** Layout · **Rust:** `ravenrs::prelude::TileLayout`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `columns` | number 0–12 by 1 | `3` |  |
| `row_height` | number 0–440 by 1 | `110` |  |
| `locked` | on/off | `false` |  |
| `min_column_width` | number 0–800 by 1 | `200` |  |

## Stories

- **Example**: 728 × 232 pt, settings `{"columns":3.0,"locked":false,"min_column_width":200.0,"row_height":110.0}`
- **Two columns, locked**: 728 × 354 pt, settings `{"columns":2.0,"locked":true,"min_column_width":200.0,"row_height":110.0}`

## Sizing

Fills the width of its parent: 288, 728, 1168 pt wide in parents of 320, 760, 1200 pt; its height follows the width, as its content wraps.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover** on "Move Sales": fill transparent → ink-muted@70; fill transparent → ink-muted@70.
- **Focus** on "Move Sales": adds a rect (border 2.0 focus-ring).
- **Pressed** on "Move Sales": fill transparent → ink-muted@70; fill transparent → ink-muted@70.

## Behavior

- **Keys** its first control handles once focused: `ArrowDown`, `ArrowRight`.
- **Accessibility**: role `Group` "Sales".
- **Focus order** (Tab): Button "Move Sales" → Button "Move Bills" → Button "Move Stock" → Button "Move People".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Button "Move Sales" | 20.0 × 48.0 | — | — | — | — | — | — |
| Example | Label "Sales" | 429.0 × 18.0 | — | — | — | Inter 600 13.0px ink | — | 0.0 |
| Example | Label "THIS MONTH" | 82.0 × 16.0 | — | — | — | Inter 600 12.0px ink-muted | — | 0.0 |
| Example | Label "$48,200" | 123.0 × 36.0 | — | — | — | Inter 600 30.0px ink | — | 0.0 |
| Example | Label "3 late" | 35.0 × 16.0 | — | — | — | Inter 600 12.0px mix(ink, danger, 30) | — | 0.0 |

## API

Builder methods: `columns`, `locked`, `min_column_width`, `new`, `row_height`, `tile`.

## Example

```rust
fn tile_layout() -> impl View {
    let spots = State::new(vec![TileSpot::new("sales", 2, 1)]);
    TileLayout::new(spots.binding())
        .columns(3)
        .row_height(110.0)
        .tile(Tile::new("sales", "Sales", Stat::new("This month", "R$ 48,200")))
        .tile(
            Tile::new("bills", "Bills", Stat::new("Open", "12"))
                .accessory(Badge::new("3 late").tone(Tone::Danger)),
        )
        .tile(Tile::new("stock", "Stock", Stat::new("Items", "1,204")))
        .tile(Tile::new("people", "People", Stat::new("Active", "37")))
}
```

Every number above is in `TileLayout.json` beside this card.
