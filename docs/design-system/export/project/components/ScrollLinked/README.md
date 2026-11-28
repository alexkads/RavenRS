# ScrollLinked

Fades, moves or scales its content by how far the page has scrolled.

**Family:** Layout · **Rust:** `ravenrs::prelude::ScrollLinked`

## Stories

- **Example**: 193 × 220 pt, settings `{}`

## Sizing

Hugs its content: 193 × 220 pt in a parent of 760 pt; in a parent that stretches its children, as a form column, it takes the whole width.

## Behavior

- **Accessibility**: role `Label` "Each card fades in".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Label "Each card fades in" | 145.0 × 22.0 | — | — | — | Inter 400 15.0px ink | — | 0.0 |
| Example | Label "as it enters the view," | 145.0 × 22.0 | — | — | — | — | — | — |

## API

Builder methods: `new`.

## Example

```rust
fn scroll_linked() -> impl View {
    let card = |text: &'static str| {
        ScrollLinked::new(Card::new(Text::new(text)), ScrollLink::fade_in().range(ScrollRange::Enter))
    };
    ScrollView::new((
        card("Each card fades in"),
        card("and rises into place"),
        card("as it enters the view,"),
        card("following the scroll"),
        card("and not a clock."),
    ))
    .spacing(80.0)
    .height(220.0)
}
```

Every number above is in `ScrollLinked.json` beside this card.
