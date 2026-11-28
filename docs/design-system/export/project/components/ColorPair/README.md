# ColorPair

Foreground and background colors, one over the other, with a swap button.

**Family:** Actions and inputs · **Rust:** `ravenrs::prelude::ColorPair`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `size` | number 0–96 by 1 | `24` |  |

## Stories

- **Example**: 62 × 48 pt, settings `{"size":24.0}`

## Sizing

Hugs its content: 62 × 48 pt in a parent of 760 pt.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover** on "Swap foreground and background": fill transparent → ink@7.
- **Focus** on "Swap foreground and background": adds a rect (border 2.0 focus-ring).
- **Pressed** on "Swap foreground and background": fill transparent → ink@12, rect [40.0,0.0,22.0,22.0] → [40.33,0.33,21.34,21.34].

## Behavior

- **Events**: `on_background`, `on_foreground`.
- **Keys** its first control handles once focused: `Enter`, `Space`.
- **Accessibility**: role `Image` "Background".
- **Focus order** (Tab): Button "Swap foreground and background" → Button "Default colors".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Image "Background" | 24.0 × 24.0 | radius-sm | on-accent | 1.0 border | — | — | — |
| Example | Image "Foreground" | 24.0 × 24.0 | radius-sm | #e11d48 | 1.0 border | — | — | — |
| Example | Button "Swap foreground and background" | 22.0 × 22.0 | radius-md | transparent | — | — | arrow-up-down 12.0px ink | 5.0 |
| Example | Button "Default colors" | 22.0 × 22.0 | radius-md | transparent | — | — | rotate-ccw 12.0px ink | 5.0 |

## API

Builder methods: `defaults`, `new`, `on_background`, `on_foreground`, `size`.

## Example

```rust
fn color_pair() -> impl View {
    let (front, back) = (State::new(Color::hex(0xe11d48)), State::new(Color::WHITE));
    ColorPair::new(front.binding(), back.binding()).defaults(Color::BLACK, Color::WHITE)
}
```

Every number above is in `ColorPair.json` beside this card.
