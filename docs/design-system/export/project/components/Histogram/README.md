# Histogram

Overlapping channels from ready-made bins, linear or logarithmic, with clip marks.

**Family:** Media · **Rust:** `ravenrs::prelude::Histogram`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `zones` | number 0–20 by 1 | `5` |  |
| `height` | number 0–440 by 1 | `110` |  |
| `clip_share` | number 0–1 by 0.01 | `0.001` |  |
| `label` | text | `` | Empty leaves it as the component has it. |
| `scale` | Linear · Logarithmic | `Linear` |  |

## Stories

- **Example**: 728 × 110 pt, settings `{"clip_share":0.001,"height":110.0,"label":"","scale":"Linear","zones":5.0}`
- **Three zones**: 728 × 110 pt, settings `{"clip_share":0.001,"height":110.0,"label":"","scale":"Linear","zones":3.0}`
- **Scale: Logarithmic**: 728 × 110 pt, settings `{"clip_share":0.001,"height":110.0,"label":"","scale":"Logarithmic","zones":5.0}`

## Sizing

Fills the width of its parent: 288, 728, 1168 pt wide in parents of 320, 760, 1200 pt.

## Behavior

- **Accessibility**: role `Image` "Histogram".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Image "Histogram" | 728.0 × 110.0 | radius-sm | bg-000 | 1.0 border | — | — | — |

## API

Builder methods: `clip_share`, `height`, `label`, `new`, `scale`, `size`, `zones`.

`HistogramScale`: `Linear`, `Logarithmic`.

## Example

```rust
fn histogram() -> impl View {
    let hill = |center: f32| -> Vec<f32> {
        (0..256).map(|i| 400.0 * (-((i as f32 - center) / 40.0).powi(2)).exp() + 10.0).collect()
    };
    Histogram::new([
        HistogramChannel::new("Red", Color::rgb(0.9, 0.15, 0.15), hill(90.0)),
        HistogramChannel::new("Green", Color::rgb(0.15, 0.8, 0.2), hill(128.0)),
        HistogramChannel::new("Blue", Color::rgb(0.2, 0.3, 0.95), hill(170.0)),
    ])
    .zones(5)
    .height(110.0)
}
```

Every number above is in `Histogram.json` beside this card.
