# Slider

Picks a number in a range by dragging, with keys and an optional value at the side.

**Family:** Actions and inputs · **Rust:** `ravenrs::prelude::Slider`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `label` | text | `Volume` |  |
| `value` | number -5–5 by 0.1 | `0.6` |  |
| `min` | number -10–0 by 1 | `0` |  |
| `max` | number 1–10 by 1 | `1` |  |
| `step` | number 0.01–1 by 0.01 | `0.01` |  |
| `show_value` | on/off | `false` | The value at the right of the label. |

## Stories

- **Volume**: 728 × 24 pt, settings `{"label":"Volume","max":1.0,"min":0.0,"show_value":false,"step":0.01,"value":0.6}`
- **Exposure**: 728 × 24 pt, settings `{"label":"Exposure","max":5.0,"min":-5.0,"show_value":true,"step":0.1,"value":1.5}`

## Sizing

Fills the width of its parent: 288, 728, 1168 pt wide in parents of 320, 760, 1200 pt.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Volume:

- **Hover** on "Volume": adds a rect (accent@25).
- **Focus** on "Volume": adds a rect (border 2.0 focus-ring).
- **Pressed** on "Volume": adds a rect (accent); adds a rect (accent@25); adds a rect (on-accent); takes a shape away.

## Behavior

- **Keys** its first control handles once focused: `ArrowDown`, `ArrowUp`, `ArrowLeft`, `ArrowRight`, `Home`, `End`.
- **Accessibility**: role `Slider` "Volume".
- **Focus order** (Tab): Slider "Volume".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Volume | Slider "Volume" | 728.0 × 24.0 | — | — | — | — | — | — |

## API

Builder methods: `disabled`, `label`, `marks`, `neutral`, `new`, `range`, `read_only`, `show_value`, `step`, `track_colors`, `value_format`, `width`.

## Example

```rust
fn slider() -> impl View {
    let volume = State::new(0.6_f32);
    let (exposure, warmth) = (State::new(1.5_f32), State::new(0.4_f32));
    let shown = volume.clone();
    VStack::new((
        Slider::new(volume.binding()).label("Volume"),
        Text::dynamic(move || format!("{:.0}%", shown.get() * 100.0)).foreground(Semantic::InkMuted),
        // Fills from zero, a double click goes back to it, and the value sits at the right.
        Slider::new(exposure.binding())
            .range(-5.0, 5.0)
            .step(0.1)
            .neutral(0.0)
            .label("Exposure")
            .show_value(),
        Slider::new(warmth.binding())
            // ravenrs-lint: allow(raw_color) the colors on the track say what the value means, cold to warm
            .track_colors([Color::hex(0x3b82f6), Color::hex(0xfacc15)])
            .label("Temperature"),
    ))
    .spacing(12.0)
    .align(Align::Stretch)
}
```

Every number above is in `Slider.json` beside this card.
