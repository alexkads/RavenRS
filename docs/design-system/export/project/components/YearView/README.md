# YearView

The twelve months of a year, with marks on the days.

**Family:** Navigation · **Rust:** `ravenrs::prelude::YearView`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `year` | number 2000–2050 by 1 | `2026` | How the example starts. |
| `months` | number 0–12 by 1 | `0` | How many months from December 2025; 0 shows the year. |
| `header` | on/off | `true` | The year and its arrows above the months. |

## Stories

- **Example**: 622 × 888 pt, settings `{"header":true,"months":0.0,"year":2026.0}`
- **A season, no header**: 622 × 182 pt, settings `{"header":false,"months":3.0,"year":2026.0}`

## Sizing

Hugs its content: 622 × 888 pt in a parent of 760 pt; in a parent of 320 pt it narrows to 194 pt; its height follows the width, as its content wraps.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover** on "Previous year": fill transparent → ink@7.
- **Focus** on "Previous year": adds a rect (border 2.0 focus-ring).
- **Pressed** on "Previous year": fill transparent → ink@12, rect [0.0,0.0,32.0,32.0] → [0.48,0.48,31.04,31.04].

## Behavior

- **Events**: `on_month`, `on_pick`.
- **Keys** its first control handles once focused: `Enter`, `Space`.
- **Accessibility**: role `Button` "Previous year".
- **Focus order** (Tab): Button "Previous year" → Button "Next year" → Button "5 November 2026".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Button "Previous year" | 32.0 × 32.0 | radius-md | transparent | — | — | chevron-left 14.0px ink | 9.0 |
| Example | Label "2026" | 39.0 × 22.0 | — | — | — | Inter 600 15.0px ink | — | 0.0 |
| Example | Button "Next year" | 32.0 × 32.0 | radius-md | transparent | — | — | chevron-right 14.0px ink | 9.0 |
| Example | Label "January" | 52.0 × 18.0 | — | — | — | Inter 600 13.0px ink | — | 0.0 |
| Example | Button "1 January 2026" | 26.0 × 26.0 | radius-sm | transparent | — | Inter 600 12.0px ink | — | 10.0 |
| Example | Button "2 January 2026" | 26.0 × 26.0 | radius-sm | transparent | — | Inter 600 12.0px ink | — | 9.0 |
| Example | Button "10 January 2026" | 26.0 × 26.0 | radius-sm | transparent | — | Inter 600 12.0px ink | — | 6.0 |
| Example | Button "11 January 2026" | 26.0 × 26.0 | radius-sm | transparent | — | Inter 600 12.0px ink | — | space-2 |
| Example | Button "17 January 2026" | 26.0 × 26.0 | radius-sm | transparent | — | Inter 600 12.0px ink | — | 7.0 |
| Example | Button "20 January 2026" | 26.0 × 26.0 | radius-sm | transparent | — | Inter 600 12.0px ink | — | 5.0 |
| Example | Button "5 November 2026" | 26.0 × 26.0 | radius-sm | transparent | 1.0 accent | Inter 600 12.0px ink | — | 9.0 |

## API

Builder methods: `columns`, `header`, `marks`, `months`, `new`, `on_month`, `on_pick`, `selected`, `today`.

## Example

```rust
fn year_view() -> impl View {
    let year = State::new(2026);
    let chosen = State::new(None::<Date>);
    YearView::new(year.binding())
        .selected(chosen.binding())
        .today(Date::new(2026, 11, 5).unwrap_or_else(|| Date::from_days(0)))
        .marks(|day| match day.day() % 11 {
            0 => Some(DayMark::new().tone(Tone::Accent).label("Shoot")),
            5 => Some(DayMark::new().tone(Tone::Success).label("Delivery")),
            _ => None,
        })
}
```

Every number above is in `YearView.json` beside this card.
