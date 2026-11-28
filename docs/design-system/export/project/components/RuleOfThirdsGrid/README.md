# RuleOfThirdsGrid

Composition lines over a picture: thirds, divisions, squares or the golden ratio.

**Family:** Media · **Rust:** `ravenrs::prelude::RuleOfThirdsGrid`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `guide` | Thirds · GoldenRatio | `GoldenRatio` |  |

## Stories

- **Example**: 300 × 170 pt, settings `{"guide":"GoldenRatio"}`
- **Guide: Thirds**: 300 × 170 pt, settings `{"guide":"Thirds"}`

## Sizing

Hugs its content: 300 × 170 pt in a parent of 760 pt.

## API

Builder methods: `area`, `color`, `grow`, `guide`, `new`, `size`, `space`, `stage`.

## Example

```rust
fn rule_of_thirds_grid() -> impl View {
    let picture = Canvas::new(|frame, scene| {
        // ravenrs-lint: allow(raw_color) the sky of the stand-in photograph the grid is laid over
        let sky = LinearGradient::vertical(Color::rgb(0.3, 0.45, 0.7), Color::rgb(0.85, 0.6, 0.45));
        scene.push_quad(Quad::new(frame, sky));
    })
    .size(300.0, 170.0);
    RuleOfThirdsGrid::new().guide(CompositionGuide::GoldenRatio).stage(picture)
}
```

Every number above is in `RuleOfThirdsGrid.json` beside this card.
