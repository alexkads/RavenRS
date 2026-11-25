# Disclosure

Header that shows or hides its content.

**Family:** Navigation · **Rust:** `ravenrs::prelude::Disclosure`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `open` | on/off | `false` |  |

## Stories

- **Example**: 196 × 42 pt, settings `{"open":false}`
- **Open**: 196 × 66 pt, settings `{"open":true}`

## Sizing

Hugs its content: 196 × 42 pt in a parent of 760 pt; in a parent that stretches its children, as a form column, it takes the whole width.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover** on "Advanced options": fill transparent → ink@4.
- **Focus** on "Advanced options": adds a rect (border 2.0 focus-ring).
- **Pressed** on "Advanced options": fill transparent → ink@4.

## Behavior

- **Keys** its first control handles once focused: `Enter`, `Space`.
- **Accessibility**: role `Button` "Advanced options".
- **Focus order** (Tab): Button "Advanced options".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Button "Advanced options" | 196.0 × 42.0 | 8.0 | transparent | — | Inter 600 15.0px ink | chevron-right 16.0px ink-muted | space-2 |
| Example | Image "Changed" | 8.0 × 8.0 | radius-full | warning | — | — | — | — |
| Open | Button "Advanced options" | 196.0 × 42.0 | 8.0 | transparent | — | Inter 600 15.0px ink | chevron-down 16.0px ink-muted | space-2 |
| Open | Label "Shown when open." | 136.0 × 22.0 | — | — | — | Inter 400 15.0px ink-muted | — | 0.0 |

## API

Builder methods: `expanded`, `key`, `mark`, `new`, `open`, `switch`, `title_view`.

## Example

```rust
fn disclosure() -> impl View {
    Disclosure::new("Advanced options", Text::new("Shown when open.").foreground(Semantic::InkMuted))
        // Tells what is inside while it is closed. `.switch` and `.expanded` take bindings.
        .mark(Dot::new().tone(Tone::Warning).label("Changed"))
}
```

Every number above is in `Disclosure.json` beside this card.
