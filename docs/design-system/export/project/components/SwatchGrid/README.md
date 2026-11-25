# SwatchGrid

Grid of color swatches; the handler gets the keys held with the click.

**Family:** Actions and inputs · **Rust:** `ravenrs::prelude::SwatchGrid`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `columns` | number 0–12 by 1 | `3` |  |
| `label` | text | `` | Empty leaves it as the component has it. |

## Stories

- **Example**: 333 × 88 pt, settings `{"columns":3.0,"label":""}`

## Sizing

Hugs its content: 333 × 88 pt in a parent of 760 pt; in a parent of 320 pt it narrows to 256 pt; its height follows the width, as its content wraps.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover** on "Amber": fill transparent → ink@6.
- **Focus** on "Rose": adds a rect (border 2.0 focus-ring).
- **Pressed** on "Amber": fill transparent → ink@6.

## Behavior

- **Events**: `on_pick`.
- **Keys** its first control handles once focused: `ArrowDown`, `ArrowRight`, `End`.
- **Accessibility**: role `Group` "Swatches".
- **Focus order** (Tab): Button "Rose" → Button "Amber" → Button "Green" → Button "Blue" → Button "Violet" → Button "Ink".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Button "Rose" | 32.0 × 32.0 | radius-full | transparent | — | — | — | — |
| Example | Label "Click: foreground. Cmd or Ctrl and click: background." | 330.0 × 16.0 | — | — | — | Inter 600 12.0px ink | — | 0.0 |

## API

Builder methods: `columns`, `label`, `new`, `on_pick`, `selected`.

## Example

```rust
fn swatch_grid() -> impl View {
    let (front, back) = (State::new(Color::hex(0xe11d48)), State::new(Color::hex(0x2563eb)));
    let (for_front, for_back, shown) = (front.clone(), back.clone(), front.clone());
    VStack::new((
        view_fn(move || {
            let (for_front, for_back) = (for_front.clone(), for_back.clone());
            SwatchGrid::new([
                (Color::hex(0xe11d48), "Rose"),
                (Color::hex(0xf59e0b), "Amber"),
                (Color::hex(0x16a34a), "Green"),
                (Color::hex(0x2563eb), "Blue"),
                (Color::hex(0x7c3aed), "Violet"),
                (Color::hex(0x111827), "Ink"),
            ])
            .columns(3)
            .selected(Some(shown.get()))
            .on_pick(move |color, held| {
                // With Cmd or Ctrl, the color goes to the background.
                if held.meta || held.ctrl { for_back.set(color) } else { for_front.set(color) }
            })
        }),
        Text::new("Click: foreground. Cmd or Ctrl and click: background.").style(TextStyle::Caption),
    ))
    .spacing(8.0)
}
```

Every number above is in `SwatchGrid.json` beside this card.
