# Eyedropper

An armed button that takes the color of the point clicked, supplied by the caller.

**Family:** Media · **Rust:** `ravenrs::prelude::Eyedropper`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `disabled` | on/off | `false` |  |
| `hint` | text | `` | Empty leaves it as the component has it. |
| `label` | text | `` | Empty leaves it as the component has it. |
| `size` | Small · Medium · Large · Mini | `Small` |  |

## Stories

- **Example**: 300 × 170 pt, settings `{"disabled":false,"hint":"","label":"","size":"Small"}`
- **Disabled**: 300 × 170 pt, settings `{"disabled":true,"hint":"","label":"","size":"Small"}`
- **Size: Medium**: 300 × 178 pt, settings `{"disabled":false,"hint":"","label":"","size":"Medium"}`
- **Size: Large**: 300 × 186 pt, settings `{"disabled":false,"hint":"","label":"","size":"Large"}`
- **Size: Mini**: 300 × 166 pt, settings `{"disabled":false,"hint":"","label":"","size":"Mini"}`

## Sizing

Hugs its content: 300 × 170 pt in a parent of 760 pt.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover** on "Pick a color": fill transparent → ink@7; adds a rect (ink); adds a text (null).
- **Focus** on "Pick a color": adds a rect (border 2.0 focus-ring).
- **Pressed** on "Pick a color": adds a rect (ink@12); adds a icon (null); adds a rect (ink); adds a text (null).

## Behavior

- **Keys** its first control handles once focused: `Enter`, `Space`.
- **Accessibility**: role `Button` "Pick a color".
- **Focus order** (Tab): Button "Pick a color".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Button "Pick a color" | 32.0 × 32.0 | radius-md | transparent | — | — | crosshair 14.0px ink | 9.0 |
| Disabled | Button "Pick a color" | 32.0 × 32.0 | radius-md | transparent | — | — | crosshair 14.0px ink@45 | 9.0 |
| Size: Medium | Button "Pick a color" | 40.0 × 40.0 | radius-md | transparent | — | — | crosshair 16.0px ink | space-3 |
| Size: Large | Button "Pick a color" | 48.0 × 48.0 | radius-md | transparent | — | — | crosshair 16.0px ink | space-4 |
| Size: Mini | Button "Pick a color" | 22.0 × 22.0 | radius-md | transparent | — | — | crosshair 12.0px ink | 5.0 |

## API

Builder methods: `disabled`, `hint`, `icon`, `label`, `new`, `size`.

## Example

```rust
fn eyedropper() -> impl View {
    let shown = State::new(Color::rgb(0.5, 0.5, 0.5));
    let told = shown.clone();
    let dropper = EyedropperState::new().on_pick(move |color| told.set(color));
    let ramp = |x: f32, y: f32| Color::rgb(x / 300.0, y / 130.0, 0.5);
    let picture = Canvas::new(move |frame, scene| {
        for i in 0..30 {
            let x = i as f32 * 10.0;
            let band = Rect::new(frame.x() + x, frame.y(), 10.0, frame.height());
            scene.push_quad(Quad::new(band, ramp(x, 65.0)));
        }
    })
    .size(300.0, 130.0);
    let swatch = Canvas::new(move |frame, scene| {
        scene.push_quad(Quad::new(frame, shown.get()).radius(4.0));
    })
    .size(28.0, 28.0);
    VStack::new((
        HStack::new((Eyedropper::new(&dropper), swatch)).spacing(8.0),
        EyedropperArea::new(&dropper, move |at| Some(ramp(at.x, at.y))).stage(picture),
    ))
    .spacing(8.0)
}
```

Every number above is in `Eyedropper.json` beside this card.
