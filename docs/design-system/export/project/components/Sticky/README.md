# Sticky

Stays below the top while its section scrolls past.

**Family:** Layout · **Rust:** `ravenrs::prelude::Sticky`

## Stories

- **Example**: 77 × 220 pt, settings `{}`

## Sizing

Hugs its content: 77 × 220 pt in a parent of 760 pt; in a parent that stretches its children, as a form column, it takes the whole width.

## Behavior

- **Accessibility**: role `Label` "Inputs".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Label "Inputs" | 39.0 × 16.0 | — | — | — | Inter 600 12.0px #a1b1f8 | — | 0.0 |
| Example | Label "Inputs 1" | 54.0 × 22.0 | — | — | — | Inter 400 15.0px ink | — | 0.0 |
| Example | Label "Overlays 3" | 76.0 × 22.0 | — | — | — | — | — | — |
| Example | Label "Data" | 29.0 × 16.0 | — | — | — | — | — | — |

## API

Builder methods: `left`, `new`, `top`.

## Example

```rust
fn sticky() -> impl View {
    let section = |name: &'static str| {
        let rows: Vec<AnyView> =
            (1..=6).map(|n| Box::new(Text::new(format!("{name} {n}"))) as AnyView).collect();
        VStack::new((Sticky::new(Badge::new(name).tone(Tone::Accent)), VStack::new(rows).spacing(8.0)))
            .spacing(8.0)
    };
    ScrollView::new((section("Inputs"), section("Overlays"), section("Data"))).spacing(16.0).height(220.0)
}
```

Every number above is in `Sticky.json` beside this card.
