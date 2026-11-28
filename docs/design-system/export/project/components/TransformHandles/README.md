# TransformHandles

Eight handles, a turn and a pivot over a box, returning its geometry.

**Family:** Media · **Rust:** `ravenrs::prelude::TransformHandles`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `disabled` | on/off | `false` |  |
| `label` | text | `` | Empty leaves it as the component has it. |

## Stories

- **Example**: 420 × 230 pt, settings `{"disabled":false,"label":""}`
- **Disabled**: 420 × 230 pt, settings `{"disabled":true,"label":""}`

## Sizing

Hugs its content: 420 × 230 pt in a parent of 760 pt; in a parent of 320 pt it narrows to 302 pt.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover**: "Transform box" does not change.
- **Focus** on "Transform box": fill bg-000@55 → ink; fill bg-000@55 → ink; fill bg-000@55 → bg-000@60; fill bg-000@55 → ink.
- **Pressed**: "Transform box" does not change.

## Behavior

- **Events**: `on_commit`.
- **Keys** its first control handles once focused: `ArrowDown`, `ArrowUp`, `ArrowLeft`, `ArrowRight`, `Escape`.
- **Accessibility**: role `Group` "Transform box".
- **Focus order** (Tab): Group "Transform box".

## API

Builder methods: `aspect`, `confined`, `disabled`, `grow`, `keep_proportions`, `label`, `min_size`, `new`, `on_commit`, `pivot_handle`, `rotatable`, `rotate_outside`, `size`, `space`, `stage`.

## Example

```rust
fn transform_handles() -> impl View {
    let layer = State::new(BoxTransform::new(Rect::new(110.0, 70.0, 180.0, 100.0)).rotated(-8.0));
    let shown = layer.clone();
    let picture = Canvas::new(move |frame, scene| {
        let p = shown.get();
        let at = p.rect.translate(frame.origin);
        // ravenrs-lint: allow(raw_color) the stand-in canvas behind the layer being transformed
        scene.push_quad(Quad::new(frame, Color::rgb(0.2, 0.27, 0.35)));
        // ravenrs-lint: allow(raw_color) the layer being transformed, as the user painted it
        scene.push_quad(Quad::new(at, Color::rgb(0.3, 0.55, 0.75).with_alpha(0.6)));
    })
    .size(420.0, 230.0);
    TransformHandles::new(layer.binding()).stage(picture)
}
```

Every number above is in `TransformHandles.json` beside this card.
