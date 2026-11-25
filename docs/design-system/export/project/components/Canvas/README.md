# Canvas

Box that draws whatever its closure appends to the scene.

**Family:** Media · **Rust:** `ravenrs::prelude::Canvas`

## Stories

- **Example**: 218 × 130 pt, settings `{}`

## Sizing

Hugs its content: 218 × 130 pt in a parent of 760 pt.

## API

Builder methods: `grow`, `new`, `size`.

## Example

```rust
fn canvas() -> impl View {
    Canvas::new(|frame, scene| {
        for i in 0..8 {
            let x = frame.x() + i as f32 * 28.0;
            let height = 20.0 + i as f32 * 12.0;
            let bar = Rect::new(x, frame.max_y() - height, 22.0, height);
            // ravenrs-lint: allow(raw_color) a Canvas paints plain colors: it is drawing, with no theme to read
            scene.push_quad(Quad::new(bar, Color::rgb(0.3, 0.4, 0.93)).radius(4.0));
        }
    })
    .size(240.0, 130.0)
}
```

Every number above is in `Canvas.json` beside this card.
