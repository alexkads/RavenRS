# ToggleButton

Button that stays pressed, like bold or mute.

**Family:** Actions and inputs · **Rust:** `ravenrs::prelude::ToggleButton`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `pill` | on/off | `false` |  |
| `bold` | on/off | `true` | How the example starts. |
| `italic` | on/off | `false` | How the example starts. |
| `open` | on/off | `true` | How the example starts. |

## Stories

- **Example**: 213 × 34 pt, settings `{"bold":true,"italic":false,"open":true,"pill":false}`
- **Italic**: 213 × 34 pt, settings `{"bold":true,"italic":true,"open":true,"pill":false}`

## Sizing

Hugs its content: 213 × 34 pt in a parent of 760 pt.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover** on "Italic": fill transparent → ink@6; color ink-muted → ink.
- **Focus** on "Bold": adds a rect (border 2.0 focus-ring).
- **Pressed** on "Italic": fill transparent → ink@6; color ink-muted → ink.

## Behavior

- **Keys** its first control handles once focused: `Enter`, `Space`.
- **Accessibility**: role `Button` "Bold".
- **Focus order** (Tab): Button "Bold" → Button "Italic" → Button "Open".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Button "Bold" | 52.0 × 34.0 | 8.0 | bg-300 | 1.0 border | Inter 600 13.0px ink | — | space-3 |
| Example | Button "Italic" | 55.0 × 34.0 | 8.0 | transparent | — | Inter 600 13.0px ink-muted | — | space-3 |
| Example | Button "Open" | 90.0 × 34.0 | radius-full | bg-300 | 1.0 border | Inter 600 13.0px ink | — | space-3 |

## API

Builder methods: `count`, `icon`, `new`, `pill`.

## Example

```rust
fn toggle_button() -> impl View {
    let bold = State::new(true);
    let italic = State::new(false);
    let open = State::new(true);
    HStack::new((
        ToggleButton::new(bold.binding(), "Bold"),
        ToggleButton::new(italic.binding(), "Italic"),
        ToggleButton::new(open.binding(), "Open").count(12).pill(),
    ))
    .spacing(8.0)
}
```

Every number above is in `ToggleButton.json` beside this card.
