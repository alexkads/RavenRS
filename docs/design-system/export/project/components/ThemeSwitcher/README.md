# ThemeSwitcher

Switches between Raven Dark, Raven Light and Raven Glass.

**Family:** Actions and inputs · **Rust:** `ravenrs::prelude::ThemeSwitcher`

## Stories

- **Example**: 251 × 36 pt, settings `{}`

## Sizing

Hugs its content: 251 × 36 pt in a parent of 760 pt.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover** on "Light": color ink-muted → ink; color ink-muted → ink.
- **Focus** on "Dark": adds a rect (border 2.0 focus-ring).
- **Pressed** on "Light": color ink-muted → ink; color ink-muted → ink.

## Behavior

- **Events**: `on_change`.
- **Keys** its first control handles once focused: `ArrowDown`, `ArrowUp`, `ArrowLeft`, `ArrowRight`, `End`.
- **Accessibility**: role `RadioGroup` "Theme".
- **Focus order** (Tab): RadioButton "Dark" → RadioButton "Light" → RadioButton "Glass".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | RadioButton "Dark" | 78.0 × 30.0 | 7.0 | bg-200 | 1.0 border | Inter 600 13.0px ink | moon 14.0px ink | 14.0 |
| Example | RadioButton "Light" | 80.0 × 30.0 | — | — | — | Inter 600 13.0px ink-muted | sun 14.0px ink-muted | 14.0 |
| Example | RadioButton "Glass" | 83.0 × 30.0 | — | — | — | Inter 600 13.0px ink-muted | layers 14.0px ink-muted | 14.0 |

## API

Builder methods: `new`, `on_change`, `themes`.

## Example

```rust
fn theme_switcher() -> impl View {
    ThemeSwitcher::new()
}
```

Every number above is in `ThemeSwitcher.json` beside this card.
