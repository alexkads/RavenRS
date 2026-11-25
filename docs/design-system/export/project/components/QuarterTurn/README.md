# QuarterTurn

QuarterTurn.

**Family:** Media · **Rust:** `ravenrs::prelude::QuarterTurn`

## Stories

- **Example**: 514 × 53 pt, settings `{}`

## Sizing

Hugs its content: 514 × 53 pt in a parent of 760 pt; in a parent of 320 pt it narrows to 64 pt.

## Behavior

- **Accessibility**: role `Label` "Files".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Label "Files" | 22.0 × 33.0 | — | — | — | Inter 400 15.0px ink | — | 6.0 |
| Example | Label "Search" | 22.0 × 50.0 | — | — | — | Inter 400 15.0px ink | — | space-4 |
| Example | Label "Names down a narrow strip, read the same by screen readers" | 437.0 × 22.0 | — | — | — | Inter 400 15.0px ink | — | 0.0 |

## API

Builder methods: `new`.

## Example

```rust
fn quarter_turn() -> impl View {
    HStack::new((
        QuarterTurn::new(Text::new("Files"), Turn::Clockwise),
        QuarterTurn::new(Text::new("Search"), Turn::CounterClockwise),
        Text::new("Names down a narrow strip, read the same by screen readers"),
    ))
    .spacing(16.0)
}
```

Every number above is in `QuarterTurn.json` beside this card.
