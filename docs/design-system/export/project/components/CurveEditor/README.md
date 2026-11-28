# CurveEditor

Points you drag over a smooth curve, with an optional histogram behind.

**Family:** Media · **Rust:** `ravenrs::prelude::CurveEditor`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `height` | number 0–760 by 1 | `190` |  |
| `label` | text | `Tone curve` |  |
| `baseline` | on/off | `true` |  |
| `disabled` | on/off | `false` |  |
| `grid` | number 0–16 by 1 | `4` |  |

## Stories

- **Example**: 728 × 190 pt, settings `{"baseline":true,"disabled":false,"grid":4.0,"height":190.0,"label":"Tone curve"}`
- **Without baseline**: 728 × 190 pt, settings `{"baseline":false,"disabled":false,"grid":4.0,"height":190.0,"label":"Tone curve"}`
- **Disabled**: 728 × 190 pt, settings `{"baseline":true,"disabled":true,"grid":4.0,"height":190.0,"label":"Tone curve"}`

## Sizing

Fills the width of its parent: 288, 728, 1168 pt wide in parents of 320, 760, 1200 pt.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover**: "Tone curve" does not change.
- **Focus** on "Tone curve": fill ink-muted@60 → ink; adds a rect (border 2.0 focus-ring).
- **Pressed** on "Tone curve": fill ink-muted@60 → ink; adds a rect (border 1.5 ink).

## Behavior

- **Keys** its first control handles once focused: `ArrowDown`, `ArrowUp`, `ArrowLeft`, `ArrowRight`, `Enter`, `Escape`.
- **Accessibility**: role `Slider` "Tone curve".
- **Focus order** (Tab): Slider "Tone curve".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Slider "Tone curve" | 728.0 × 190.0 | radius-sm | bg-000 | 1.0 border | — | — | — |

## API

Builder methods: `baseline`, `color`, `disabled`, `grid`, `height`, `histogram`, `label`, `new`, `size`, `value_at`.

## Example

```rust
fn curve_editor() -> impl View {
    let points = State::new(vec![
        Point::new(0.0, 0.0),
        Point::new(0.3, 0.2),
        Point::new(0.7, 0.85),
        Point::new(1.0, 1.0),
    ]);
    let hill: Vec<f32> = (0..128).map(|i| 50.0 * (-((i as f32 - 60.0) / 30.0).powi(2)).exp()).collect();
    CurveEditor::new(points.binding())
        .height(190.0)
        .histogram([HistogramChannel::new("Light", Color::rgb(0.6, 0.6, 0.6), hill)], HistogramScale::Linear)
        .label("Tone curve")
}
```

Every number above is in `CurveEditor.json` beside this card.
