# CropOverlay

A crop with a veil, proportion and angle, returning the rectangle and the angle.

**Family:** Media · **Rust:** `ravenrs::prelude::CropOverlay`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `disabled` | on/off | `false` |  |
| `label` | text | `` | Empty leaves it as the component has it. |

## Stories

- **Example**: 400 × 280 pt, settings `{"disabled":false,"label":""}`
- **Disabled**: 400 × 280 pt, settings `{"disabled":true,"label":""}`

## Sizing

Hugs its content: 400 × 280 pt in a parent of 760 pt; in a parent of 320 pt it narrows to 314 pt.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover**: "Crop area" does not change.
- **Focus** on "Crop area": adds a rect (border 2.0 focus-ring); fill scrim → ink-muted@70; fill scrim → ink; fill scrim → accent.
- **Pressed**: "Crop area" does not change.

## Behavior

- **Events**: `on_commit`.
- **Keys** its first control handles once focused: `ArrowDown`, `ArrowUp`, `ArrowLeft`, `ArrowRight`, `Escape`.
- **Accessibility**: role `Group` "Crop area".
- **Focus order** (Tab): Group "Crop area" → Slider "Angle".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Slider "Angle" | 400.0 × 52.0 | radius-md | bg-300 | 1.0 border | Inter 600 12.0px ink | — | 187.0 |
| Example | Label "0.0°" | 27.0 × 16.0 | — | — | — | Inter 600 12.0px ink | — | 0.0 |

## API

Builder methods: `angle_limit`, `aspect`, `disabled`, `grow`, `guide`, `label`, `min_size`, `new`, `on_commit`, `size`, `space`, `stage`, `turning_guide`.

## Example

```rust
fn crop_overlay() -> impl View {
    let crop = State::new(CropArea::new(Rect::new(70.0, 30.0, 240.0, 150.0)));
    let picture = Canvas::new(|frame, scene| {
        // ravenrs-lint: allow(raw_color) the sky of the stand-in photograph being cropped
        let sky = LinearGradient::vertical(Color::rgb(0.3, 0.45, 0.7), Color::rgb(0.85, 0.6, 0.45));
        scene.push_quad(Quad::new(frame, sky));
    })
    .size(400.0, 220.0);
    VStack::new((
        CropOverlay::new(crop.binding()).stage(picture),
        Protractor::new(crop.binding().map(|c| c.angle, |c, angle| c.angle = angle)).width(400.0),
    ))
    .spacing(8.0)
}
```

Every number above is in `CropOverlay.json` beside this card.
