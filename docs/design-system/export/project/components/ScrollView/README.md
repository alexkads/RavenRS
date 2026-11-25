# ScrollView

Scrolling stack, vertical or horizontal.

**Family:** Layout · **Rust:** `ravenrs::prelude::ScrollView`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `spacing` | number 0–32 by 0.1 | `8` |  |
| `height` | number 0–720 by 1 | `180` |  |
| `horizontal` | on/off | `false` |  |
| `scrollbar_outside` | number 0–48 by 1 | `0` | How far past the right edge the scrollbar sits, in points. |

## Stories

- **Example**: 54 × 180 pt, settings `{"height":180.0,"horizontal":false,"scrollbar_outside":0.0,"spacing":8.0}`
- **Horizontal**: 54 × 180 pt, settings `{"height":180.0,"horizontal":true,"scrollbar_outside":0.0,"spacing":8.0}`

## Sizing

Hugs its content: 54 × 180 pt in a parent of 760 pt; in a parent that stretches its children, as a form column, it takes the whole width.

## Behavior

- **Accessibility**: role `Label` "Row 1".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Label "Row 1" | 54.0 × 22.0 | — | — | — | Inter 400 15.0px ink | — | 0.0 |
| Example | Label "Row 10" | 54.0 × 22.0 | — | — | — | — | — | — |

## API

Builder methods: `align`, `height`, `horizontal`, `new`, `padding`, `scrollbar_outside`, `spacing`.

## Example

```rust
fn scroll_view() -> impl View {
    let rows: Vec<AnyView> = (1..=30).map(|n| Box::new(Text::new(format!("Row {n}"))) as AnyView).collect();
    ScrollView::new(rows).spacing(8.0).height(180.0)
}
```

Every number above is in `ScrollView.json` beside this card.
