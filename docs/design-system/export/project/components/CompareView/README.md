# CompareView

Two pictures side by side or across a divider you drag.

**Family:** Media · **Rust:** `ravenrs::prelude::CompareView`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `mode` | Divider · SideBySide | `Divider` |  |
| `width` | number 160–760 by 10 | `420` |  |
| `height` | number 80–480 by 10 | `200` |  |
| `captions` | on/off | `true` | Before and After over each picture. |

## Stories

- **Divider**: 420 × 200 pt, settings `{"captions":true,"height":200.0,"mode":"Divider","width":420.0}`
- **Side by side**: 420 × 200 pt, settings `{"captions":true,"height":200.0,"mode":"SideBySide","width":420.0}`
- **No captions**: 420 × 200 pt, settings `{"captions":false,"height":200.0,"mode":"Divider","width":420.0}`

## Sizing

Hugs its content: 420 × 200 pt in a parent of 760 pt; in a parent of 320 pt it narrows to 412 pt.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Divider:

- **Hover**: "Comparison" does not change.
- **Focus** on "Comparison": adds a rect (border 2.0 focus-ring).
- **Pressed**: "Comparison" does not change.

## Behavior

- **Keys** its first control handles once focused: `ArrowLeft`, `ArrowRight`, `Home`, `End`.
- **Accessibility**: role `Slider` "Comparison".
- **Focus order** (Tab): Slider "Comparison".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Divider | Slider "Comparison" | 420.0 × 200.0 | 0.0 | bg-000 | — | Inter 400 12.0px ink | — | space-4 |
| Divider | Label "Before" | 40.0 × 16.0 | — | — | — | Inter 400 12.0px ink | — | 0.0 |
| No captions | Slider "Comparison" | 420.0 × 200.0 | 0.0 | bg-000 | — | — | — | — |

## API

Builder methods: `captions`, `chosen`, `divider`, `grow`, `label`, `mode`, `new`, `no_captions`, `size`.

## Example

```rust
fn compare_view() -> AnyView {
    let tone = |warm: f32| {
        ImageData::from_fn(240, 160, move |x, y| {
            let light = 0.25 + 0.5 * x as f32 / 240.0;
            Color::rgb(light + warm, light, light - warm + 0.2 * y as f32 / 160.0)
        })
    };
    match (tone(0.0), tone(0.2)) {
        (Some(before), Some(after)) => {
            Box::new(CompareView::new(before, after).mode(CompareMode::Divider).size(420.0, 200.0))
        }
        _ => Box::new(Text::new("No pictures")),
    }
}
```

Every number above is in `CompareView.json` beside this card.
