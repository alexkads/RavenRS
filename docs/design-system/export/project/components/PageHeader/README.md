# PageHeader

Title, description, badges and actions at the top of a page.

**Family:** Layout · **Rust:** `ravenrs::prelude::PageHeader`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `title` | text | `Sessions` |  |
| `description` | text | `Every shoot, newest first.` |  |
| `action` | on/off | `true` | A button at the end of the line. |
| `compact` | on/off | `false` |  |
| `badge` | text | `12` | Beside the title; empty shows none. |
| `badge_icon` | none · plus · check · trash-2 · pencil · search · settings · download | `none` |  |

## Stories

- **Example**: 728 × 50 pt, settings `{"action":true,"badge":"12","badge_icon":"none","compact":false,"description":"Every shoot, newest first.","title":"Sessions"}`
- **Compact**: 728 × 40 pt, settings `{"action":true,"badge":"12","badge_icon":"none","compact":true,"description":"Every shoot, newest first.","title":"Sessions"}`
- **No action**: 160 × 50 pt, settings `{"action":false,"badge":"12","badge_icon":"none","compact":false,"description":"Every shoot, newest first.","title":"Sessions"}`

## Sizing

Hugs its content: 728 × 50 pt in a parent of 760 pt; in a parent of 320 pt it narrows to 160 pt; its height follows the width, as its content wraps.

- **No action**: Hugs its content: 160 × 50 pt in a parent of 760 pt.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover** on "New session": fill accent → accent-hover.
- **Focus** on "New session": adds a rect (border 2.0 focus-ring).
- **Pressed** on "New session": fill accent → accent-pressed, rect [575.0,5.0,153.0,40.0] → [577.29,5.6,148.41,38.8].

## Behavior

- **Accessibility**: role `Heading` "Sessions".
- **Focus order** (Tab): Button "New session".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Heading "Sessions" | 97.0 × 28.0 | — | — | — | Inter 600 22.0px ink | — | 0.0 |
| Example | Label "12" | 14.0 × 16.0 | — | — | — | Inter 600 12.0px mix(ink, ink-muted, 30) | — | 0.0 |
| Example | Label "Every shoot, newest first." | 156.0 × 18.0 | — | — | — | Inter 400 13.0px ink-muted | — | 0.0 |
| Example | Button "New session" | 153.0 × 40.0 | radius-md | accent | — | Inter 600 15.0px on-accent | plus 16.0px on-accent | 18.0 |
| Compact | Heading "Sessions" | 75.0 × 24.0 | — | — | — | Inter 600 17.0px ink | — | 0.0 |

## API

Builder methods: `action`, `badge`, `compact`, `description`, `new`.

## Example

```rust
fn page_header() -> impl View {
    PageHeader::new("Sessions")
        .description("Every shoot, newest first.")
        .badge(Badge::new("12"))
        .action(Button::new("New session").icon(IconName::Plus))
}
```

Every number above is in `PageHeader.json` beside this card.
