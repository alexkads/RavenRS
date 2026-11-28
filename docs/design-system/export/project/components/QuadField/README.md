# QuadField

A plane of squares that waves like a floor and stands up into a picture as the page scrolls.

**Family:** Layout · **Rust:** `ravenrs::prelude::QuadField`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `height` | number 0–1120 by 1 | `280` |  |
| `range` | Scene · Through · Enter · Exit | `Scene` |  |

## Stories

- **Example**: 10 × 280 pt, settings `{"height":280.0,"range":"Scene"}`
- **Range: Through**: 10 × 280 pt, settings `{"height":280.0,"range":"Through"}`
- **Range: Enter**: 10 × 280 pt, settings `{"height":280.0,"range":"Enter"}`
- **Range: Exit**: 10 × 280 pt, settings `{"height":280.0,"range":"Exit"}`

## Sizing

Hugs its content: 10 × 280 pt in a parent of 760 pt; in a parent that stretches its children, as a form column, it takes the whole width.

## API

Builder methods: `height`, `new`, `range`.

## Example

```rust
fn quad_field() -> impl View {
    // Scroll inside: the waving floor stands up into a small dashboard.
    // ravenrs-lint: allow(raw_color) the pixels of the scene, like those of an `ImageData::from_fn`
    let (floor, bars, ground) = (Color::hex(0x222734), Color::hex(0x4c66ee), Color::hex(0x12151c));
    let picture = move |u: f32, v: f32, time: f64| {
        let bar = ((u - 0.1) / 0.1).floor();
        let height = 0.35 + 0.25 * (bar * 1.7 + time as f32).sin().abs();
        if v < 0.15 {
            floor
        } else if (0.1..0.9).contains(&u) && v > 0.9 - height {
            bars
        } else {
            ground
        }
    };
    ScrollView::new((
        ScrollScene::new(QuadField::new(64, 36, picture).height(280.0)).length(600.0),
        Canvas::new(|_, _| {}).size(10.0, 400.0),
    ))
    .spacing(0.0)
    .height(280.0)
}
```

Every number above is in `QuadField.json` beside this card.
