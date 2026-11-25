# ColorSwatch

Round color to pick from a palette.

**Family:** Actions and inputs · **Rust:** `ravenrs::prelude::ColorSwatch`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `selected` | on/off | `false` |  |
| `current` | number 0–5 by 1 | `0` | How the example starts. |

## Stories

- **Example**: 128 × 32 pt, settings `{"current":0.0,"selected":false}`
- **Selected**: 128 × 32 pt, settings `{"current":0.0,"selected":true}`

## Sizing

Hugs its content: 128 × 32 pt in a parent of 760 pt.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover** on "Accent": fill transparent → ink@6.
- **Focus** on "Accent": adds a rect (border 2.0 focus-ring).
- **Pressed** on "Accent": fill transparent → ink@6.

## Behavior

- **Events**: `on_click`.
- **Accessibility**: role `Button` "Accent".
- **Focus order** (Tab): Button "Accent" → Button "Success" → Button "Danger".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Button "Accent" | 32.0 × 32.0 | radius-full | transparent | — | — | — | — |

## API

Builder methods: `new`, `none`, `on_click`, `selected`.

## Example

```rust
fn color_swatch() -> impl View {
    let current = State::new(0);
    view_fn(move || {
        let theme = Theme::raven_light();
        let options =
            [(Semantic::Accent, "Accent"), (Semantic::Success, "Success"), (Semantic::Danger, "Danger")];
        let swatches: Vec<AnyView> = options
            .iter()
            .enumerate()
            .map(|(i, (role, name))| {
                let pick = current.clone();
                Box::new(
                    ColorSwatch::new(theme.color(*role), *name)
                        .selected(current.get() == i)
                        .on_click(move || pick.set(i)),
                ) as AnyView
            })
            .collect();
        HStack::new(swatches)
    })
}
```

Every number above is in `ColorSwatch.json` beside this card.
