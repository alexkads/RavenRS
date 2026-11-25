# Protractor

A band of ticks along an arc that sets an angle.

**Family:** Media · **Rust:** `ravenrs::prelude::Protractor`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `label` | text | `Straighten` |  |
| `disabled` | on/off | `false` |  |
| `step` | number 0–1 by 0.01 | `0.1` |  |
| `width` | number 0–1040 by 1 | `260` |  |

## Stories

- **Example**: 260 × 52 pt, settings `{"disabled":false,"label":"Straighten","step":0.1,"width":260.0}`
- **Disabled**: 260 × 52 pt, settings `{"disabled":true,"label":"Straighten","step":0.1,"width":260.0}`

## Sizing

Hugs its content: 260 × 52 pt in a parent of 760 pt.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover**: "Straighten" does not change.
- **Focus** on "Straighten": fill ink-muted@70 → ink; fill ink-muted@70 → accent; adds a rect (border 2.0 focus-ring).
- **Pressed**: "Straighten" does not change.

## Behavior

- **Events**: `on_commit`.
- **Keys** its first control handles once focused: `Escape`.
- **Accessibility**: role `Slider` "Straighten".
- **Focus order** (Tab): Slider "Straighten".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Slider "Straighten" | 260.0 × 52.0 | radius-md | bg-300 | 1.0 border | Inter 600 12.0px ink | — | 114.0 |
| Example | Label "+7.5°" | 32.0 × 16.0 | — | — | — | Inter 600 12.0px ink | — | 0.0 |
| Disabled | Slider "Straighten" | 260.0 × 52.0 | radius-md | bg-300 | 1.0 border | Inter 600 12.0px ink-muted | — | 114.0 |
| Disabled | Label "+7.5°" | 32.0 × 16.0 | — | — | — | Inter 600 12.0px ink-muted | — | 0.0 |

## API

Builder methods: `disabled`, `label`, `new`, `on_commit`, `range`, `step`, `width`.

## Example

```rust
fn protractor() -> impl View {
    let angle = State::new(7.5_f32);
    Protractor::new(angle.binding()).label("Straighten")
}
```

Every number above is in `Protractor.json` beside this card.
