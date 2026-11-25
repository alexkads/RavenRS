# Calendar

Month grid to pick a date.

**Family:** Forms · **Rust:** `ravenrs::prelude::Calendar`

## Stories

- **Example**: 264 × 298 pt, settings `{}`

## Sizing

Hugs its content: 264 × 298 pt in a parent of 760 pt.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover** on "Previous month": fill transparent → ink@7.
- **Focus** on "Previous month": adds a rect (border 2.0 focus-ring).
- **Pressed** on "Previous month": fill transparent → ink@12, rect [0.0,0.0,32.0,32.0] → [0.48,0.48,31.04,31.04].

## Behavior

- **Keys** its first control handles once focused: `Enter`, `Space`.
- **Accessibility**: role `Button` "Previous month".
- **Focus order** (Tab): Button "Previous month" → Button "Next month" → Button "1 November 2026".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Button "Previous month" | 32.0 × 32.0 | radius-md | transparent | — | — | chevron-left 14.0px ink | 9.0 |
| Example | Label "November 2026" | 118.0 × 22.0 | — | — | — | Inter 600 15.0px ink | — | 0.0 |
| Example | Button "Next month" | 32.0 × 32.0 | radius-md | transparent | — | — | chevron-right 14.0px ink | 9.0 |
| Example | Button "1 November 2026" | 36.0 × 36.0 | 9.0 | accent | — | Inter 600 13.0px on-accent | — | 15.0 |
| Example | Button "2 November 2026" | 36.0 × 36.0 | 9.0 | transparent | — | Inter 400 13.0px ink | — | 14.0 |
| Example | Button "10 November 2026" | 36.0 × 36.0 | 9.0 | transparent | — | Inter 400 13.0px ink | — | 11.0 |
| Example | Button "11 November 2026" | 36.0 × 36.0 | 9.0 | transparent | — | Inter 400 13.0px ink | — | 13.0 |
| Example | Button "15 November 2026" | 36.0 × 36.0 | 9.0 | transparent | — | Inter 400 13.0px ink | — | space-3 |
| Example | Button "20 November 2026" | 36.0 × 36.0 | 9.0 | transparent | — | Inter 400 13.0px ink | — | 10.0 |
| Example | Button "1 December 2026" | 36.0 × 36.0 | 9.0 | transparent | — | Inter 400 13.0px ink-muted | — | 15.0 |
| Example | Button "2 December 2026" | 36.0 × 36.0 | 9.0 | transparent | — | Inter 400 13.0px ink-muted | — | 14.0 |
| Example | Button "10 December 2026" | 36.0 × 36.0 | 9.0 | transparent | — | Inter 400 13.0px ink-muted | — | 11.0 |
| Example | Button "11 December 2026" | 36.0 × 36.0 | 9.0 | transparent | — | Inter 400 13.0px ink-muted | — | 13.0 |

## API

Builder methods: `new`, `range`, `today`.

`CalendarView`: `Day`, `Month`, `Week`.

## Example

```rust
fn calendar() -> impl View {
    let today = Date::new(2026, 11, 1).unwrap_or_else(|| unreachable!("a real date"));
    let date = State::new(Some(today));
    Calendar::new(date.binding(), today).today(today)
}
```

Every number above is in `Calendar.json` beside this card.
