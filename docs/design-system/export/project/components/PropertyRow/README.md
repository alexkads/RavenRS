# PropertyRow

A label on the left and its value on the right.

**Family:** Layout · **Rust:** `ravenrs::prelude::PropertyRow`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `label` | text | `Camera` |  |
| `value` | text | `NIKON D3100` |  |

## Stories

- **Example**: 728 × 18 pt, settings `{"label":"Camera","value":"NIKON D3100"}`
- **Empty value**: 728 × 18 pt, settings `{"label":"Lens","value":""}`
- **With a badge**: 728 × 22 pt, settings `{"label":"Camera","value":"NIKON D3100"}`

## Sizing

Fills the width of its parent: 288, 728, 1168 pt wide in parents of 320, 760, 1200 pt.

## Behavior

- **Accessibility**: role `Label` "Camera".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Label "Camera" | 728.0 × 18.0 | — | — | — | Inter 400 13.0px ink-muted | — | 0.0 |
| With a badge | Label "Edited" | 39.0 × 16.0 | — | — | — | Inter 600 12.0px mix(success, ink, 70) | — | 0.0 |

## API

Builder methods: `new`, `with`.

## Example

```rust
fn property_row() -> impl View {
    VStack::new((
        PropertyRow::new("Camera", "NIKON D3100"),
        PropertyRow::new("Lens", ""),
        PropertyRow::with("Status", Badge::new("Edited").tone(Tone::Success)),
    ))
    .spacing(8.0)
    .align(Align::Stretch)
}
```

Every number above is in `PropertyRow.json` beside this card.
