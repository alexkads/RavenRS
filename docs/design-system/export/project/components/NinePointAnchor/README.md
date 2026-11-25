# NinePointAnchor

Three by three grid for a point of reference, moved by the arrow keys.

**Family:** Actions and inputs · **Rust:** `ravenrs::prelude::NinePointAnchor`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `cell_size` | number 0–48 by 0.1 | `12` |  |
| `label` | text | `` | Empty leaves it as the component has it. |

## Stories

- **Example**: 38 × 38 pt, settings `{"cell_size":12.0,"label":""}`

## Sizing

Hugs its content: 38 × 38 pt in a parent of 760 pt.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover**: "Reference point" does not change.
- **Focus** on "Reference point": adds a rect (border 2.0 focus-ring).
- **Pressed**: "Reference point" does not change.

## Behavior

- **Keys** its first control handles once focused: `ArrowDown`, `ArrowUp`, `ArrowLeft`, `ArrowRight`.
- **Accessibility**: role `RadioGroup` "Reference point".
- **Focus order** (Tab): RadioGroup "Reference point".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | RadioButton "Top left" | 12.0 × 12.0 | 0.0 | transparent | 1.0 ink-muted | — | — | — |
| Example | RadioButton "Center" | 12.0 × 12.0 | 0.0 | ink | 1.0 ink | — | — | — |

## API

Builder methods: `cell_size`, `label`, `new`.

## Example

```rust
fn nine_point_anchor() -> impl View {
    let point = State::new(AnchorPoint::Center);
    NinePointAnchor::new(point.binding()).cell_size(12.0)
}
```

Every number above is in `NinePointAnchor.json` beside this card.
