# Pagination

Previous, numbered pages with ellipses, and Next.

**Family:** Navigation · **Rust:** `ravenrs::prelude::Pagination`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `compact` | on/off | `false` |  |
| `page` | number 0–5 by 1 | `0` | How the example starts. |

## Stories

- **Example**: 439 × 32 pt, settings `{"compact":false,"page":0.0}`
- **Compact**: 257 × 32 pt, settings `{"compact":true,"page":0.0}`

## Sizing

Hugs its content: 439 × 32 pt in a parent of 760 pt; in a parent of 320 pt it narrows to 288 pt; in a parent that stretches its children, as a form column, it takes the whole width; its height follows the width, as its content wraps.

- **Compact**: Hugs its content: 257 × 32 pt in a parent of 760 pt; in a parent that stretches its children, as a form column, it takes the whole width.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover** on "Page 2": fill transparent → ink@7.
- **Focus** on "Page 1": adds a rect (border 2.0 focus-ring).
- **Pressed** on "Page 2": fill transparent → ink@12, rect [139.0,0.0,33.0,32.0] → [139.49,0.48,32.01,31.04].

## Behavior

- **Accessibility**: role `Group` "Pagination".
- **Focus order** (Tab): Button "Page 1" → Button "Page 2" → Button "Page 3" → Button "Page 4" → Button "Page 5" → Button "Page 24" → Button "Next".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Button "Previous" | 101.0 × 32.0 | radius-md | transparent | — | Inter 600 13.0px ink@45 | chevron-left 14.0px ink@45 | space-3 |
| Example | Button "Page 1" | 30.0 × 32.0 | radius-md | bg-300 | 1.0 border | Inter 600 13.0px ink | — | space-3 |
| Example | Button "Page 2" | 33.0 × 32.0 | radius-md | transparent | — | Inter 600 13.0px ink | — | space-3 |
| Example | Button "Next" | 76.0 × 32.0 | radius-md | transparent | — | Inter 600 13.0px ink | chevron-right 14.0px ink | space-3 |
| Compact | Label "Page 1 / 24" | 72.0 × 18.0 | — | — | — | Inter 400 13.0px ink | — | 19.0 |

## API

Builder methods: `compact`, `labels`, `new`.

## Example

```rust
fn pagination() -> impl View {
    let page = State::new(0);
    Pagination::new(page.binding(), 24)
}
```

Every number above is in `Pagination.json` beside this card.
