# Accordion

Disclosures separated by hairlines.

**Family:** Navigation · **Rust:** `ravenrs::prelude::Accordion`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `first_open` | on/off | `true` | The first section starts open. |

## Stories

- **Example**: 306 × 123 pt, settings `{"first_open":true}`
- **All closed**: 280 × 89 pt, settings `{"first_open":false}`

## Sizing

Hugs its content: 306 × 123 pt in a parent of 760 pt; in a parent of 320 pt it narrows to 280 pt; in a parent that stretches its children, as a form column, it takes the whole width; its height follows the width, as its content wraps.

- **All closed**: Hugs its content: 280 × 89 pt in a parent of 760 pt; in a parent that stretches its children, as a form column, it takes the whole width.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover** on "What platforms does it support?": fill transparent → ink@4.
- **Focus** on "What platforms does it support?": adds a rect (border 2.0 focus-ring).
- **Pressed** on "What platforms does it support?": fill transparent → ink@4.

## Behavior

- **Keys** its first control handles once focused: `Enter`, `Space`.
- **Accessibility**: role `Button` "What platforms does it support?".
- **Focus order** (Tab): Button "What platforms does it support?" → Button "Can I build my own components?".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Button "What platforms does it support?" | 306.0 × 42.0 | 8.0 | transparent | — | Inter 600 15.0px ink | chevron-down 16.0px ink-muted | space-2 |
| Example | Label "Desktop and browsers with WebGPU." | 266.0 × 22.0 | — | — | — | Inter 400 15.0px ink | — | 0.0 |
| Example | Button "Can I build my own components?" | 306.0 × 42.0 | 8.0 | transparent | — | Inter 600 15.0px ink | chevron-right 16.0px ink-muted | space-2 |

## API

Builder methods: `new`.

## Example

```rust
fn accordion() -> impl View {
    Accordion::new(vec![
        Disclosure::new("What platforms does it support?", Text::new("Desktop and browsers with WebGPU."))
            .open(true),
        Disclosure::new("Can I build my own components?", Text::new("Yes, with the same public API.")),
    ])
}
```

Every number above is in `Accordion.json` beside this card.
