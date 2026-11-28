# Kbd

Keyboard key, for showing shortcuts.

**Family:** Display and feedback · **Rust:** `ravenrs::prelude::Kbd`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `inherit_color` | on/off | `false` |  |

## Stories

- **Example**: 204 × 32 pt, settings `{"inherit_color":false}`

## Sizing

Hugs its content: 204 × 32 pt in a parent of 760 pt.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover** on "Open": fill accent → accent-hover.
- **Focus** on "Open": adds a rect (border 2.0 focus-ring).
- **Pressed** on "Open": fill accent → accent-pressed, rect [110.0,0.0,94.0,32.0] → [111.41,0.48,91.18,31.04]; border {color:on-accent@40,width:1.0} → {color:on-accent@40,width:0.97}, rect [164.0,7.0,28.0,19.0] → [163.79,7.27,27.16,18.43].

## Behavior

- **Accessibility**: role `Label` "⌘".
- **Focus order** (Tab): Button "Open".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Label "⌘" | 13.0 × 16.0 | — | — | — | Inter 400 12.0px ink-muted | — | 0.0 |
| Example | Button "Open" | 94.0 × 32.0 | radius-md | accent | — | Inter 600 13.0px on-accent | — | space-3 |

## API

Builder methods: `inherit_color`, `new`.

## Example

```rust
fn kbd() -> impl View {
    HStack::new((
        Kbd::new("⌘"),
        Kbd::new("Shift"),
        Kbd::new("K"),
        // The button gives the key its own color.
        Button::new("Open").size(ButtonSize::Small).shortcut("F8"),
    ))
    .spacing(8.0)
    .align(Align::Center)
}
```

Every number above is in `Kbd.json` beside this card.
