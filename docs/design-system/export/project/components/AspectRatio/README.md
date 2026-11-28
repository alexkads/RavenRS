# AspectRatio

Keeps content at a width-to-height ratio.

**Family:** Layout · **Rust:** `ravenrs::prelude::AspectRatio`

## Stories

- **Example**: 320 × 180 pt, settings `{}`

## Sizing

Hugs its content: 320 × 180 pt in a parent of 760 pt; in a parent of 320 pt it narrows to 1 pt.

## API

Builder methods: `new`.

## Example

```rust
fn aspect_ratio() -> impl View {
    VStack::new((AspectRatio::new(16.0 / 9.0, Image::new(picture(1.0)).stretch()),))
        .width(320.0)
        .align(Align::Stretch)
}
```

Every number above is in `AspectRatio.json` beside this card.
