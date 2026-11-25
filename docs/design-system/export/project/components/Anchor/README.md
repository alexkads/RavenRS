# Anchor

A place a #name link scrolls to, gliding there.

**Family:** Layout · **Rust:** `ravenrs::prelude::Anchor`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `offset` | number 0–200 by 1 | `0` | Lands this far below the top, under a pinned bar, in points. |

## Stories

- **Example**: 170 × 200 pt, settings `{"offset":0.0}`

## Sizing

Hugs its content: 170 × 200 pt in a parent of 760 pt; in a parent that stretches its children, as a form column, it takes the whole width.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover** on "Jump to the summary": fill transparent → ink@6; color mix(ink, accent, 40) → ink.
- **Focus** on "Jump to the summary": fill transparent → ink@6; color mix(ink, accent, 40) → ink; adds a rect (border 2.0 focus-ring).
- **Pressed** on "Jump to the summary": fill transparent → ink@6; color mix(ink, accent, 40) → ink.

## Behavior

- **Keys** its first control handles once focused: `Enter`, `Space`, `Escape`.
- **Accessibility**: role `Link` "Jump to the summary".
- **Focus order** (Tab): Link "Jump to the summary".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Link "Jump to the summary" | 170.0 × 30.0 | radius-sm | transparent | — | Inter 600 15.0px mix(ink, accent, 40) | — | 6.0 |
| Example | Label "Row 1" | 41.0 × 22.0 | — | — | — | Inter 400 15.0px ink | — | 0.0 |
| Example | Label "Row 9" | 44.0 × 22.0 | — | — | — | — | — | — |
| Example | Heading "Summary" | 170.0 × 28.0 | — | — | — | — | — | — |

## API

Builder methods: `new`, `offset`.

`AnchorPoint`: `Bottom`, `BottomLeft`, `BottomRight`, `Center`, `Left`, `Right`, `Top`, `TopLeft`, `TopRight`.

## Example

```rust
fn anchor() -> impl View {
    let rows: Vec<AnyView> = (1..=12).map(|n| Box::new(Text::new(format!("Row {n}"))) as AnyView).collect();
    ScrollView::new((
        Link::new("Jump to the summary", "#summary"),
        VStack::new(rows).spacing(8.0),
        Anchor::new("summary", Text::new("Summary").style(TextStyle::Title)),
        Canvas::new(|_, _| {}).size(10.0, 160.0),
    ))
    .spacing(12.0)
    .height(200.0)
}
```

Every number above is in `Anchor.json` beside this card.
