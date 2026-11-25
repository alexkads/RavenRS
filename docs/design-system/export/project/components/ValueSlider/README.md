# ValueSlider

A value in small that opens a slider or can be typed.

**Family:** Actions and inputs · **Rust:** `ravenrs::prelude::ValueSlider`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `step` | number 0–4 by 0.1 | `1` |  |
| `unit` | text | `%` |  |
| `label` | text | `Opacity` |  |
| `show_label` | on/off | `true` |  |
| `disabled` | on/off | `false` |  |
| `field` | on/off | `false` |  |

## Stories

- **Example**: 240 × 32 pt, settings `{"disabled":false,"field":false,"label":"Opacity","show_label":true,"step":1.0,"unit":"%"}`
- **Disabled**: 240 × 32 pt, settings `{"disabled":true,"field":false,"label":"Opacity","show_label":true,"step":1.0,"unit":"%"}`

## Sizing

Hugs its content: 240 × 32 pt in a parent of 760 pt.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover** on "Opacity: 80%": fill bg-300 → mix(ink, bg-300, 6).
- **Focus** on "Opacity: 80%": adds a rect (border 2.0 focus-ring).
- **Pressed** on "Opacity: 80%": border {color:border,width:1.0} → {color:border,width:0.97}, fill bg-300 → mix(ink, bg-300, 12), rect [56.0,0.0,76.0,32.0] → [57.14,0.48,73.72,31.04].

## Behavior

- **Keys** its first control handles once focused: `Enter`, `Space`.
- **Accessibility**: role `Button` "Opacity: 80%".
- **Focus order** (Tab): Button "Opacity: 80%" → TextInput "Size" → Button "Size".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Button "Opacity: 80%" | 76.0 × 32.0 | radius-md | bg-300 | 1.0 border | Inter 600 13.0px ink | chevron-down 14.0px ink | space-3 |
| Example | TextInput "Size" | 84.0 × 32.0 | radius-md | bg-300 | 1.0 control-border | Inter 400 13.0px ink | chevron-down 12.0px ink | 10.0 |
| Example | Label "px" | 15.0 × 18.0 | — | — | — | Inter 400 13.0px ink-muted | — | 0.0 |
| Example | Button "Size" | 22.0 × 22.0 | radius-md | transparent | — | — | chevron-down 12.0px ink | 5.0 |
| Disabled | Button "Opacity: 80%" | 76.0 × 32.0 | radius-md | bg-300@45 | 1.0 border | Inter 600 13.0px ink@45 | chevron-down 14.0px ink@45 | space-3 |

## API

Builder methods: `disabled`, `field`, `label`, `neutral`, `new`, `range`, `show_label`, `slider_width`, `step`, `unit`, `value_format`.

## Example

```rust
fn value_slider() -> impl View {
    let (opacity, size) = (State::new(80.0_f32), State::new(24.0_f32));
    HStack::new((
        ValueSlider::new(opacity.binding())
            .range(0.0, 100.0)
            .step(1.0)
            .unit("%")
            .label("Opacity")
            .show_label(),
        ValueSlider::new(size.binding()).range(1.0, 200.0).step(1.0).unit(" px").label("Size").field(),
    ))
    .spacing(24.0)
    .align(Align::Center)
}
```

Every number above is in `ValueSlider.json` beside this card.
