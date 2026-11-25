# Skeleton

Placeholder block shown while content loads.

**Family:** Display and feedback · **Rust:** `ravenrs::prelude::Skeleton`

## Stories

- **Example**: 180 × 44 pt, settings `{}`

## Sizing

Hugs its content: 180 × 44 pt in a parent of 760 pt; in a parent that stretches its children, as a form column, it takes the whole width.

## API

Builder methods: `block`, `circle`, `line`.

## Example

```rust
fn skeleton() -> impl View {
    HStack::new((
        Skeleton::circle(44.0),
        VStack::new((Skeleton::line(14.0), Skeleton::block(120.0, 12.0)))
            .spacing(8.0)
            .align(Align::Stretch)
            .grow(1.0),
    ))
    .spacing(16.0)
}
```

Every number above is in `Skeleton.json` beside this card.
