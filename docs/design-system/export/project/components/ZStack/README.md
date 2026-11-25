# ZStack

Views on top of each other, in a box as large as the largest.

**Family:** Layout · **Rust:** `ravenrs::prelude::ZStack`

## Stories

- **Example**: 474 × 80 pt, settings `{}`

## Sizing

Hugs its content: 474 × 80 pt in a parent of 760 pt; in a parent of 320 pt it narrows to 254 pt.

## Behavior

- **Accessibility**: role `Label` "On top".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Label "On top" | 41.0 × 16.0 | — | — | — | Inter 600 12.0px mix(success, ink, 70) | — | 0.0 |

## API

Builder methods: `align`, `new`.

## Example

```rust
fn z_stack() -> impl View {
    ZStack::new((
        Spacer::new().frame(Some(220.0), Some(80.0)).background(Semantic::Accent).corner_radius(8.0),
        Badge::new("On top").tone(Tone::Success),
    ))
    .align(Align::Center)
}
```

Every number above is in `ZStack.json` beside this card.
