# HoverCard

Rich content shown while the pointer is over a trigger.

**Family:** Overlays · **Rust:** `ravenrs::prelude::HoverCard`

## Stories

- **Example**: 74 × 22 pt, settings `{}`

## Sizing

Hugs its content: 74 × 22 pt in a parent of 760 pt.

## Behavior

- **Accessibility**: role `Label` "@ana.lima".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Label "@ana.lima" | 74.0 × 22.0 | — | — | — | Inter 400 15.0px accent | — | 0.0 |

## API

Builder methods: `new`.

## Example

```rust
fn hover_card() -> impl View {
    HoverCard::new(
        Text::new("@ana.lima").foreground(Semantic::Accent),
        HStack::new((Avatar::new("Ana Lima"), Text::new("Design lead at Northwind"))).spacing(12.0),
    )
}
```

Every number above is in `HoverCard.json` beside this card.
