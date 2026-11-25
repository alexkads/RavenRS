# Divider

Hairline separator.

**Family:** Layout · **Rust:** `ravenrs::prelude::Divider`

## Stories

- **Example**: 46 × 69 pt, settings `{}`

## Sizing

Hugs its content: 46 × 69 pt in a parent of 760 pt; in a parent that stretches its children, as a form column, it takes the whole width.

## Behavior

- **Accessibility**: role `Label` "Above".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Label "Above" | 46.0 × 22.0 | — | — | — | Inter 400 15.0px ink | — | 0.0 |

## API

Builder methods: `horizontal`, `vertical`.

## Example

```rust
fn divider() -> impl View {
    VStack::new((Text::new("Above"), Divider::horizontal(), Text::new("Below")))
        .spacing(12.0)
        .align(Align::Stretch)
}
```

Every number above is in `Divider.json` beside this card.
