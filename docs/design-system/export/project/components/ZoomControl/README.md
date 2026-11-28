# ZoomControl

Fit, 1:1, the percentage and buttons that zoom in and out.

**Family:** Media · **Rust:** `ravenrs::prelude::ZoomControl`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `label` | text | `` | Empty leaves it as the component has it. |
| `size` | Small · Medium · Large · Mini | `Small` |  |

## Stories

- **Example**: 208 × 32 pt, settings `{"label":"","size":"Small"}`
- **Size: Medium**: 253 × 40 pt, settings `{"label":"","size":"Medium"}`
- **Size: Large**: 293 × 48 pt, settings `{"label":"","size":"Large"}`
- **Size: Mini**: 172 × 22 pt, settings `{"label":"","size":"Mini"}`

## Sizing

Hugs its content: 208 × 32 pt in a parent of 760 pt.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover** on "Fit": fill transparent → ink@7.
- **Focus** on "Fit": adds a rect (border 2.0 focus-ring).
- **Pressed** on "Fit": fill transparent → ink@12, rect [0.0,0.0,40.0,32.0] → [0.6,0.48,38.8,31.04].

## Behavior

- **Keys** its first control handles once focused: `Enter`, `Space`.
- **Accessibility**: role `Group` "Zoom".
- **Focus order** (Tab): Button "Fit" → Button "Actual size" → Button "Zoom out" → Button "Zoom in".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Button "Fit" | 40.0 × 32.0 | radius-md | transparent | — | Inter 600 13.0px ink | — | space-3 |
| Example | Button "Zoom out" | 32.0 × 32.0 | radius-md | transparent | — | — | minus 14.0px ink | 9.0 |
| Example | Label "150%" | 34.0 × 18.0 | — | — | — | Inter 400 13.0px ink | — | 0.0 |
| Example | Button "Zoom in" | 32.0 × 32.0 | radius-md | transparent | — | — | plus 14.0px ink | 9.0 |
| Size: Medium | Button "Fit" | 55.0 × 40.0 | radius-md | transparent | — | Inter 600 15.0px ink | — | 18.0 |
| Size: Medium | Button "Zoom out" | 40.0 × 40.0 | radius-md | transparent | — | — | minus 16.0px ink | space-3 |
| Size: Medium | Button "Zoom in" | 40.0 × 40.0 | radius-md | transparent | — | — | plus 16.0px ink | space-3 |
| Size: Large | Button "Fit" | 67.0 × 48.0 | radius-md | transparent | — | Inter 600 15.0px ink | — | space-6 |
| Size: Large | Button "Zoom out" | 48.0 × 48.0 | radius-md | transparent | — | — | minus 16.0px ink | space-4 |
| Size: Large | Button "Zoom in" | 48.0 × 48.0 | radius-md | transparent | — | — | plus 16.0px ink | space-4 |
| Size: Mini | Button "Fit" | 32.0 × 22.0 | radius-md | transparent | — | Inter 600 12.0px ink | — | space-2 |
| Size: Mini | Button "Zoom out" | 22.0 × 22.0 | radius-md | transparent | — | — | minus 12.0px ink | 5.0 |
| Size: Mini | Button "Zoom in" | 22.0 × 22.0 | radius-md | transparent | — | — | plus 12.0px ink | 5.0 |

## API

Builder methods: `label`, `new`, `range`, `size`.

## Example

```rust
fn zoom_control() -> impl View {
    let view = State::new(ZoomView::at(1.5));
    ZoomControl::new(view.binding())
}
```

Every number above is in `ZoomControl.json` beside this card.
