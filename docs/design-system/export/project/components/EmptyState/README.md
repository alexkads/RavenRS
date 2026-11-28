# EmptyState

What an area shows when it has nothing: icon, title, sentence and actions.

**Family:** Display and feedback · **Rust:** `ravenrs::prelude::EmptyState`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `description` | text | `Import a folder to start. The originals stay where they are.` |  |

## Stories

- **Example**: 573 × 198 pt, settings `{"description":"Import a folder to start. The originals stay where they are."}`
- **Title only**: 470 × 168 pt, settings `{"description":""}`

## Sizing

Hugs its content: 573 × 198 pt in a parent of 760 pt; in a parent of 320 pt it narrows to 253 pt; its height follows the width, as its content wraps.

- **Title only**: Hugs its content: 470 × 168 pt in a parent of 760 pt; in a parent of 320 pt it narrows to 250 pt.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover** on "Import": fill accent → accent-hover.
- **Focus** on "Import": adds a rect (border 2.0 focus-ring).
- **Pressed** on "Import": fill accent → accent-pressed, rect [259.0,158.0,84.0,40.0] → [260.26,158.6,81.48,38.8].

## Behavior

- **Accessibility**: role `Heading` "No photos yet".
- **Focus order** (Tab): Button "Import" → Button "Learn more".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Heading "No photos yet" | 115.0 × 24.0 | — | — | — | Inter 600 17.0px ink | — | 0.0 |
| Example | Label "Import a folder to start. The originals stay where they are." | 407.0 × 22.0 | — | — | — | Inter 400 15.0px ink-muted | — | 0.0 |
| Example | Button "Import" | 84.0 × 40.0 | radius-md | accent | — | Inter 600 15.0px on-accent | — | 18.0 |
| Example | Button "Learn more" | 119.0 × 40.0 | radius-md | transparent | — | Inter 600 15.0px ink | — | 18.0 |

## API

Builder methods: `action`, `description`, `icon`, `new`.

## Example

```rust
fn empty_state() -> impl View {
    EmptyState::new("No photos yet")
        .icon(IconName::Image)
        .description("Import a folder to start. The originals stay where they are.")
        .action(Button::new("Import"))
        .action(Button::new("Learn more").style(ButtonStyle::Ghost))
}
```

Every number above is in `EmptyState.json` beside this card.
