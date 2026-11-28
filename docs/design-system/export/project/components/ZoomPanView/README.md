# ZoomPanView

Zoom and pan with pinch, wheel, drag, keys and inertia, within limits.

**Family:** Media · **Rust:** `ravenrs::prelude::ZoomPanView`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `label` | text | `Sample picture` |  |
| `grow` | number 0–4 by 0.1 | `1` |  |
| `inertia` | on/off | `true` |  |

## Stories

- **Example**: 420 × 240 pt, settings `{"grow":1.0,"inertia":true,"label":"Sample picture"}`

## Sizing

Hugs its content: 420 × 240 pt in a parent of 760 pt; in a parent of 320 pt it narrows to 360 pt.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover** on "Actual size": fill transparent → ink@7.
- **Focus** on "Fit": adds a rect (border 2.0 focus-ring).
- **Pressed** on "Actual size": fill transparent → ink@12, rect [44.0,0.0,40.0,32.0] → [44.6,0.48,38.8,31.04].

## Behavior

- **Accessibility**: role `Group` "Zoom".
- **Focus order** (Tab): Button "Fit" → Button "Actual size" → Button "Zoom out" → Button "Zoom in" → Image "Sample picture".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Button "Fit" | 40.0 × 32.0 | radius-md | accent@18 | 1.0 accent@55 | Inter 600 13.0px mix(ink, accent, 30) | — | space-3 |
| Example | Button "Actual size" | 40.0 × 32.0 | radius-md | transparent | — | Inter 600 13.0px ink | — | space-3 |
| Example | Button "Zoom out" | 32.0 × 32.0 | radius-md | transparent | — | — | minus 14.0px ink | 9.0 |
| Example | Label "25%" | 29.0 × 18.0 | — | — | — | Inter 400 13.0px ink | — | 0.0 |
| Example | Button "Zoom in" | 32.0 × 32.0 | radius-md | transparent | — | — | plus 14.0px ink | 9.0 |
| Example | Image "Sample picture" | 420.0 × 200.0 | — | — | — | — | — | — |

## API

Builder methods: `custom`, `grow`, `image`, `inertia`, `label`, `range`, `size`.

## Example

```rust
fn zoom_pan_view() -> impl View {
    let view = State::new(ZoomView::fit());
    let content = Size::new(1200.0, 800.0);
    let picture = |frame: ZoomPanPaint, scene: &mut Scene| {
        // ravenrs-lint: allow(raw_color) the stand-in photograph being zoomed: its colors are the content
        scene.push_quad(Quad::new(frame.placed, Color::rgb(0.16, 0.3, 0.5)));
        let side = 120.0 * frame.scale;
        // ravenrs-lint: allow(raw_color) the stand-in photograph being zoomed: its colors are the content
        for (i, color) in [Color::rgb(0.9, 0.45, 0.3), Color::rgb(0.95, 0.8, 0.3)].into_iter().enumerate() {
            let x = frame.placed.x() + frame.placed.width() * (0.2 + 0.4 * i as f32);
            let y = frame.placed.y() + frame.placed.height() * 0.35;
            scene.push_quad(Quad::new(Rect::new(x, y, side, side), color).radius(side / 2.0));
        }
    };
    VStack::new((
        ZoomControl::new(view.binding()),
        ZoomPanView::custom(view.binding(), content, picture).size(420.0, 200.0).label("Sample picture"),
    ))
    .spacing(8.0)
}
```

Every number above is in `ZoomPanView.json` beside this card.
