# Minimap

Small copy of the content with a draggable rectangle around what is in view.

**Family:** Media · **Rust:** `ravenrs::prelude::Minimap`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `label` | text | `` | Empty leaves it as the component has it. |

## Stories

- **Example**: 412 × 150 pt, settings `{"label":""}`

## Sizing

Hugs its content: 412 × 150 pt in a parent of 760 pt.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover**: "Navigator" does not change.
- **Focus** on "Navigator": fill bg-000 → #3373b3; border null → {color:border,width:1.0}, fill bg-000 → transparent; adds a rect (border 2.0 focus-ring).
- **Pressed**: "Navigator" does not change.

## Behavior

- **Keys** its first control handles once focused: `ArrowDown`, `ArrowUp`, `ArrowLeft`, `ArrowRight`, `Escape`.
- **Accessibility**: role `Slider` "Navigator".
- **Focus order** (Tab): Slider "Navigator" → Image "Zoomable view".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Slider "Navigator" | 160.0 × 100.0 | radius-sm | bg-000 | — | — | — | — |
| Example | Image "Zoomable view" | 240.0 × 150.0 | — | — | — | — | — | — |

## API

Builder methods: `content_size`, `custom`, `image`, `label`, `max_size`, `press_zoom`.

## Example

```rust
fn minimap() -> impl View {
    let view = State::new(ZoomView::at(1.0));
    let picture = |frame: ZoomPanPaint, scene: &mut Scene| {
        // ravenrs-lint: allow(raw_color) the stand-in photograph the minimap shows: its colors are the content
        scene.push_quad(Quad::new(frame.placed, Color::rgb(0.2, 0.45, 0.7)));
        let sun = Rect::new(
            frame.placed.x() + frame.placed.width() * 0.6,
            frame.placed.y() + frame.placed.height() * 0.2,
            80.0 * frame.scale,
            80.0 * frame.scale,
        );
        // ravenrs-lint: allow(raw_color) the stand-in photograph the minimap shows: its colors are the content
        scene.push_quad(Quad::new(sun, Color::rgb(0.95, 0.8, 0.3)).radius(40.0 * frame.scale));
    };
    let content = Size::new(800.0, 500.0);
    HStack::new((
        Minimap::custom(view.binding(), content, picture),
        ZoomPanView::custom(view.binding(), content, picture).size(240.0, 150.0),
    ))
    .spacing(12.0)
}
```

Every number above is in `Minimap.json` beside this card.
