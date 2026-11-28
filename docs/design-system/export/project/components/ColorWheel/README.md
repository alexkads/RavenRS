# ColorWheel

Hue and saturation on a disc, by a drag or the arrow keys.

**Family:** Media · **Rust:** `ravenrs::prelude::ColorWheel`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `diameter` | number 0–640 by 1 | `160` |  |
| `label` | text | `Shadows` |  |
| `disabled` | on/off | `false` |  |

## Stories

- **Example**: 234 × 160 pt, settings `{"diameter":160.0,"disabled":false,"label":"Shadows"}`
- **Disabled**: 234 × 160 pt, settings `{"diameter":160.0,"disabled":true,"label":"Shadows"}`

## Sizing

Hugs its content: 234 × 160 pt in a parent of 760 pt.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover**: "Shadows" does not change.
- **Focus** on "Shadows": adds a rect (border 2.0 focus-ring).
- **Pressed** on "Shadows": fill #ffcc99 → on-accent, rect [98.79,58.9,13.0,13.0] → [73.5,73.5,13.0,13.0]; rect [97.79,57.9,15.0,15.0] → [72.5,72.5,15.0,15.0].

## Behavior

- **Keys** its first control handles once focused: `ArrowDown`, `ArrowUp`, `ArrowLeft`, `ArrowRight`, `Home`.
- **Accessibility**: role `Slider` "Shadows".
- **Focus order** (Tab): Slider "Shadows".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Slider "Shadows" | 160.0 × 160.0 | — | — | — | — | — | — |
| Example | Label "30°, 40%" | 58.0 × 18.0 | — | — | — | Inter 400 13.0px ink-muted | — | 0.0 |

## API

Builder methods: `brightness`, `diameter`, `disabled`, `label`, `new`.

## Example

```rust
fn color_wheel() -> impl View {
    let tint = State::new(HueSaturation::new(30.0, 0.4));
    let shown = tint.clone();
    HStack::new((
        ColorWheel::new(tint.binding()).diameter(160.0).label("Shadows"),
        Text::bound(&shown, |v| format!("{:.0}°, {:.0}%", v.hue, v.saturation * 100.0))
            .style(TextStyle::Callout)
            .foreground(Semantic::InkMuted),
    ))
    .spacing(16.0)
}
```

Every number above is in `ColorWheel.json` beside this card.
