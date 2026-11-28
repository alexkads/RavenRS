# ThemePicker

A small menu of themes that says which one is on.

**Family:** Actions and inputs · **Rust:** `ravenrs::prelude::ThemePicker`

## Stories

- **Example**: 112 × 32 pt, settings `{}`

## Sizing

Hugs its content: 112 × 32 pt in a parent of 760 pt.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover** on "Theme": fill bg-300 → mix(ink, bg-300, 6).
- **Focus** on "Theme": adds a rect (border 2.0 focus-ring).
- **Pressed** on "Theme": border {color:border,width:1.0} → {color:border,width:0.97}, fill bg-300 → mix(ink, bg-300, 12), rect [0.0,0.0,112.0,32.0] → [1.68,0.48,108.64,31.04].

## Behavior

- **Keys** its first control handles once focused: `Enter`, `Space`, `Escape`.
- **Accessibility**: role `Button` "Theme".
- **Focus order** (Tab): Button "Theme".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Button "Theme" | 112.0 × 32.0 | radius-md | bg-300 | 1.0 border | Inter 600 13.0px ink | moon 14.0px ink | space-3 |

## API

Builder methods: `new`, `of`, `theme`.

## Example

```rust
fn theme_picker() -> impl View {
    ThemePicker::new()
}
```

Every number above is in `ThemePicker.json` beside this card.
