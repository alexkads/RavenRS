# ImageLightbox

A picture over the whole app, with zoom, arrows, Esc, caption and counter.

**Family:** Media · **Rust:** `ravenrs::prelude::ImageLightbox`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `label` | text | `` | Empty leaves it as the component has it. |
| `wrap` | on/off | `false` |  |

## Stories

- **Example**: 138 × 40 pt, settings `{"label":"","wrap":false}`

## Sizing

Hugs its content: 138 × 40 pt in a parent of 760 pt.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover** on "Open pictures": fill accent → accent-hover.
- **Focus** on "Open pictures": adds a rect (border 2.0 focus-ring).
- **Pressed** on "Open pictures": fill accent → accent-pressed, rect [0.0,0.0,138.0,40.0] → [2.07,0.6,133.86,38.8].

## Behavior

- **Keys** its first control handles once focused: `Enter`, `Space`, `Escape`.
- **Accessibility**: role `Button` "Open pictures".
- **Focus order** (Tab): Button "Open pictures".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Button "Open pictures" | 138.0 × 40.0 | radius-md | accent | — | Inter 600 15.0px on-accent | — | 18.0 |

## API

Builder methods: `label`, `new`, `wrap`.

## Example

```rust
fn image_lightbox() -> impl View {
    let open = State::new(None::<usize>);
    let opener = open.clone();
    let shades = [0.25_f32, 0.45, 0.65];
    let pictures = shades.into_iter().filter_map(|shade| {
        let image = ImageData::from_fn(320, 200, move |x, _| Color::rgb(shade, 0.3, x as f32 / 320.0))?;
        Some(LightboxItem::new(image).caption(format!("Shade {shade}")))
    });
    VStack::new((
        Button::new("Open pictures").on_click(move || opener.set(Some(0))),
        ImageLightbox::new(open.binding(), pictures),
    ))
}
```

Every number above is in `ImageLightbox.json` beside this card.
