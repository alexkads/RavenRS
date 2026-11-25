# VStack

Lays its children out in a column, a set space apart.

**Family:** Layout · **Rust:** `ravenrs::prelude::VStack`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `spacing` | number 0–48 by 4 | `16` | space-4 by default; the 4px grid. |
| `align` | Start · Center · End · Stretch | `Start` | Across the column. |
| `width` | number 160–760 by 10 | `360` |  |

## Stories

- **Column**: 231 × 116 pt, settings `{"align":"Start","spacing":16.0,"width":360.0}`
- **Tight**: 231 × 92 pt, settings `{"align":"Start","spacing":4.0,"width":360.0}`
- **Stretched**: 360 × 116 pt, settings `{"align":"Stretch","spacing":16.0,"width":360.0}`

## Sizing

Hugs its content: 231 × 116 pt in a parent of 760 pt.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Column:

- **Hover** on "Open": fill bg-300 → mix(ink, bg-300, 6).
- **Focus** on "Open": adds a rect (border 2.0 focus-ring).
- **Pressed** on "Open": border {color:border,width:1.0} → {color:border,width:0.97}, fill bg-300 → mix(ink, bg-300, 12), rect [0.0,76.0,75.0,40.0] → [1.13,76.6,72.75,38.8].

## Behavior

- **Accessibility**: role `Label` "Quarterly report".
- **Focus order** (Tab): Button "Open".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Column | Label "Quarterly report" | 114.0 × 22.0 | — | — | — | Inter 400 15.0px ink | — | 0.0 |
| Column | Label "A grouped list of what changed." | 227.0 × 22.0 | — | — | — | Inter 400 15.0px ink-muted | — | 0.0 |
| Column | Button "Open" | 75.0 × 40.0 | radius-md | bg-300 | 1.0 border | Inter 600 15.0px ink | — | 18.0 |

## API

Builder methods: `align`, `grow`, `height`, `justify`, `max_width`, `new`, `padding`, `spacing`, `width`, `wrap`.

## Example

```rust
fn v_stack() -> impl View {
    VStack::new((Text::new("First"), Text::new("Second"), Button::new("Continue"))).spacing(12.0)
}
```

Every number above is in `VStack.json` beside this card.
