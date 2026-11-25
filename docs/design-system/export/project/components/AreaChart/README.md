# AreaChart

Smooth area chart of evenly spaced values.

**Family:** Data · **Rust:** `ravenrs::prelude::AreaChart`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `height` | number 0–720 by 1 | `180` |  |

## Stories

- **Example**: 728 × 180 pt, settings `{"height":180.0}`

## Sizing

Fills the width of its parent: 288, 728, 1168 pt wide in parents of 320, 760, 1200 pt.

## Behavior

- **Accessibility**: role `Image` "12, 14, 13, 17, 16, 21, 19, 24, 23, 27".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Image "12, 14, 13, 17, 16, 21, 19, 24, 23, 27" | 728.0 × 180.0 | — | — | — | — | — | — |

## API

Builder methods: `height`, `new`, `tone`.

## Example

```rust
fn area_chart() -> impl View {
    AreaChart::new([12.0, 14.0, 13.0, 17.0, 16.0, 21.0, 19.0, 24.0, 23.0, 27.0]).height(180.0)
}
```

Every number above is in `AreaChart.json` beside this card.
