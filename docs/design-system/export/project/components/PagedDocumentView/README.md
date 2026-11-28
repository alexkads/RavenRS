# PagedDocumentView

A bar with page and zoom over pages built only when they show.

**Family:** Media · **Rust:** `ravenrs::prelude::PagedDocumentView`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `height` | number 0–1600 by 1 | `400` |  |
| `label` | text | `` | Empty leaves it as the component has it. |

## Stories

- **Example**: 728 × 400 pt, settings `{"height":400.0,"label":""}`

## Sizing

Hugs its content: 728 × 400 pt in a parent of 760 pt; in a parent of 320 pt it narrows to 336 pt.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover**: "Document" does not change.
- **Focus** on "Document": adds a rect (border 2.0 focus-ring).
- **Pressed**: "Document" does not change.

## Behavior

- **Keys** its first control handles once focused: `End`.
- **Accessibility**: role `Group` "Document".
- **Focus order** (Tab): Group "Document" → Button "Next page" → Button "Fit" → Button "Actual size" → Button "Zoom out" → Button "Zoom in".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Button "Previous page" | 32.0 × 32.0 | radius-md | transparent | — | — | chevron-up 14.0px ink@45 | 9.0 |
| Example | Button "Next page" | 32.0 × 32.0 | radius-md | transparent | — | — | chevron-down 14.0px ink | 9.0 |
| Example | Label "Page 1 of 48" | 77.0 × 18.0 | — | — | — | Inter 400 13.0px ink | — | 0.0 |
| Example | Button "Fit" | 40.0 × 32.0 | radius-md | accent@18 | 1.0 accent@55 | Inter 600 13.0px mix(ink, accent, 30) | — | space-3 |
| Example | Button "Actual size" | 40.0 × 32.0 | radius-md | transparent | — | Inter 600 13.0px ink | — | space-3 |
| Example | Button "Zoom out" | 32.0 × 32.0 | radius-md | transparent | — | — | minus 14.0px ink | 9.0 |
| Example | Button "Zoom in" | 32.0 × 32.0 | radius-md | transparent | — | — | plus 14.0px ink | 9.0 |
| Example | Image "Page 1 of 48" | 696.0 × 986.0 | 0.0 | bg-100 | 1.0 border | — | — | — |
| Example | Image "Page 2 of 48" | 696.0 × 986.0 | — | — | — | — | — | — |

## API

Builder methods: `height`, `label`, `new`, `page`, `zoom`.

## Example

```rust
fn paged_document_view() -> impl View {
    let sheet = ImageData::from_fn(120, 170, |x, y| {
        // Lines of a page, where a real app would hand over a rendered PDF.
        let line = y % 12 < 3 && (10..110).contains(&x) && (14..156).contains(&y);
        if line { Color::rgb(0.55, 0.55, 0.6) } else { Color::rgb(0.98, 0.98, 0.97) }
    });
    PagedDocumentView::new(
        48,
        |_| Size::new(420.0, 595.0),
        // Page 3 has not been drawn yet: it shows a placeholder of the right size.
        move |page| sheet.clone().filter(|_| page != 2),
    )
    .height(400.0)
}
```

Every number above is in `PagedDocumentView.json` beside this card.
