# GlassCard

Translucent card for Raven Glass compositions.

**Family:** Layout · **Rust:** `ravenrs::prelude::GlassCard`

## Stories

- **Example**: 354 × 70 pt, settings `{}`

## Sizing

Hugs its content: 354 × 70 pt in a parent of 760 pt; in a parent of 320 pt it narrows to 251 pt; in a parent that stretches its children, as a form column, it takes the whole width; its height follows the width, as its content wraps.

## Behavior

- **Accessibility**: role `Label` "Translucent, for Raven Glass compositions.".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Label "Translucent, for Raven Glass compositions." | 306.0 × 22.0 | — | — | — | Inter 400 15.0px ink | — | 0.0 |

## API

Builder methods: `corner_radius`, `new`, `padding`, `shadow`.

## Example

```rust
fn glass_card() -> impl View {
    GlassCard::new(Text::new("Translucent, for Raven Glass compositions."))
}
```

Every number above is in `GlassCard.json` beside this card.
