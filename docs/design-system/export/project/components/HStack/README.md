# HStack

Lays its children out in a row, a set space apart.

**Family:** Layout · **Rust:** `ravenrs::prelude::HStack`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `spacing` | number 0–48 by 4 | `16` | space-4 by default; the 4px grid. |
| `align` | Start · Center · End · Stretch | `Start` | Across the row. |
| `justify` | Start · Center · End · SpaceBetween | `Start` | Along the row. |
| `width` | number 200–760 by 10 | `520` |  |
| `wrap` | on/off | `false` | Children that do not fit go to the next line. |

## Stories

- **Row**: 240 × 40 pt, settings `{"align":"Start","justify":"Start","spacing":16.0,"width":520.0,"wrap":false}`
- **Tight**: 224 × 40 pt, settings `{"align":"Start","justify":"Start","spacing":8.0,"width":520.0,"wrap":false}`
- **Space between**: 520 × 40 pt, settings `{"align":"Start","justify":"SpaceBetween","spacing":16.0,"width":520.0,"wrap":false}`
- **Centered**: 380 × 40 pt, settings `{"align":"Center","justify":"Center","spacing":16.0,"width":520.0,"wrap":false}`

## Sizing

Hugs its content: 240 × 40 pt in a parent of 760 pt.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Row:

- **Hover** on "Save": fill accent → accent-hover.
- **Focus** on "Save": adds a rect (border 2.0 focus-ring).
- **Pressed** on "Save": fill accent → accent-pressed, rect [0.0,0.0,72.0,40.0] → [1.08,0.6,69.84,38.8].

## Behavior

- **Accessibility**: role `Button` "Save".
- **Focus order** (Tab): Button "Save" → Button "Cancel".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Row | Button "Save" | 72.0 × 40.0 | radius-md | accent | — | Inter 600 15.0px on-accent | — | 18.0 |
| Row | Button "Cancel" | 87.0 × 40.0 | radius-md | bg-300 | 1.0 border | Inter 600 15.0px ink | — | 18.0 |
| Row | Label "Draft" | 31.0 × 16.0 | — | — | — | Inter 600 12.0px mix(ink, ink-muted, 30) | — | 0.0 |

## API

Builder methods: `align`, `grow`, `height`, `justify`, `max_width`, `new`, `padding`, `spacing`, `width`, `wrap`.

## Example

```rust
fn h_stack() -> impl View {
    HStack::new((Text::new("First"), Text::new("Second"), Button::new("Continue")))
        .spacing(12.0)
        .align(Align::Center)
}
```

Every number above is in `HStack.json` beside this card.
