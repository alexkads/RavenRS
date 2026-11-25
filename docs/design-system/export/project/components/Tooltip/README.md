# Tooltip

Short label shown on hover, with an optional key cap.

**Family:** Overlays · **Rust:** `ravenrs::prelude::Tooltip`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `shortcut` | text | `` | Empty leaves it as the component has it. |

## Stories

- **Example**: 262 × 96 pt, settings `{"shortcut":""}`

## Sizing

Hugs its content: 262 × 96 pt in a parent of 760 pt.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover** on "It stays out of the way": fill bg-300 → mix(ink, bg-300, 6); adds a rect (ink); adds a text (null).
- **Focus** on "It stays out of the way": adds a rect (border 2.0 focus-ring).
- **Pressed** on "It stays out of the way": adds a rect (border 0.97 border); adds a text (null); adds a rect (ink); adds a text (null).

## Behavior

- **Accessibility**: role `Button` "It stays out of the way".
- **Focus order** (Tab): Button "It stays out of the way" → Button "Save the file" → Button "Open a file".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Button "It stays out of the way" | 99.0 × 40.0 | radius-md | bg-300 | 1.0 border | Inter 600 15.0px ink | — | 18.0 |

## API

Builder methods: `new`, `shortcut`.

## Example

```rust
fn tooltip() -> impl View {
    VStack::new((
        Spacer::new().min(40.0),
        HStack::new((
            Tooltip::new(Button::new("Show tip").style(ButtonStyle::Secondary), "It stays out of the way"),
            Button::new("Save").style(ButtonStyle::Secondary).tooltip("Save the file"),
            Tooltip::new(Button::new("Open").style(ButtonStyle::Secondary), "Open a file").shortcut("⌘O"),
        ))
        .spacing(8.0),
    ))
}
```

Every number above is in `Tooltip.json` beside this card.
