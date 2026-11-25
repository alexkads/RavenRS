# FavoriteToggle

Heart or star that marks a favorite: filled when on.

**Family:** Actions and inputs · **Rust:** `ravenrs::prelude::FavoriteToggle`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `label` | text | `Warm portrait` |  |
| `size` | number 0–64 by 1 | `16` |  |
| `star` | on/off | `false` |  |

## Stories

- **Example**: 64 × 28 pt, settings `{"label":"Warm portrait","size":16.0,"star":false}`

## Sizing

Hugs its content: 64 × 28 pt in a parent of 760 pt.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover** on "Pinned": fill transparent → ink@6; color ink-muted → ink.
- **Focus** on "Warm portrait": adds a rect (border 2.0 focus-ring).
- **Pressed** on "Pinned": fill transparent → ink@6; color ink-muted → ink.

## Behavior

- **Keys** its first control handles once focused: `Enter`, `Space`.
- **Accessibility**: role `Button` "Warm portrait".
- **Focus order** (Tab): Button "Warm portrait" → Button "Pinned".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Button "Warm portrait" | 28.0 × 28.0 | radius-sm | transparent | — | — | — | — |
| Example | Button "Pinned" | 28.0 × 28.0 | radius-sm | transparent | — | — | star 16.0px ink-muted | 6.0 |

## API

Builder methods: `color`, `label`, `new`, `size`, `star`.

## Example

```rust
fn favorite_toggle() -> impl View {
    let (heart, star) = (State::new(true), State::new(false));
    HStack::new((
        FavoriteToggle::new(heart.binding()).label("Warm portrait"),
        FavoriteToggle::new(star.binding()).label("Pinned").star(),
    ))
    .spacing(8.0)
}
```

Every number above is in `FavoriteToggle.json` beside this card.
