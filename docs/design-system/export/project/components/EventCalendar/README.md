# EventCalendar

A month with its events inside each day.

**Family:** Forms · **Rust:** `ravenrs::prelude::EventCalendar`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `events_per_day` | number 0–12 by 1 | `3` |  |
| `view` | Month · Week · Day · Agenda · Year · Season | `Month` | A month of days, a week or a day with an hour on each line, the events in a list, the year or the season. |

## Stories

- **Month**: 486 × 602 pt, settings `{"events_per_day":3.0,"view":"Month"}`
- **Week**: 565 × 666 pt, settings `{"events_per_day":3.0,"view":"Week"}`
- **Day**: 498 × 666 pt, settings `{"events_per_day":3.0,"view":"Day"}`
- **Agenda**: 794 × 210 pt, settings `{"events_per_day":3.0,"view":"Agenda"}`
- **Year**: 622 × 890 pt, settings `{"events_per_day":3.0,"view":"Year"}`
- **Season**: 670 × 228 pt, settings `{"events_per_day":3.0,"view":"Season"}`

## Sizing

Hugs its content: 486 × 602 pt in a parent of 760 pt; in a parent that stretches its children, as a form column, it takes the whole width.

- **Year**: Hugs its content: 622 × 890 pt in a parent of 760 pt; in a parent of 320 pt it narrows to 609 pt; in a parent that stretches its children, as a form column, it takes the whole width; its height follows the width, as its content wraps.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Month:

- **Hover** on "Previous month": fill transparent → ink@7.
- **Focus** on "Previous month": adds a rect (border 2.0 focus-ring).
- **Pressed** on "Previous month": fill transparent → ink@12, rect [0.0,2.0,32.0,32.0] → [0.48,2.48,31.04,31.04].

## Behavior

- **Events**: `on_create`, `on_event`, `on_move`.
- **Keys** its first control handles once focused: `Enter`, `Space`.
- **Accessibility**: role `Button` "Previous month".
- **Focus order** (Tab): Button "Previous month" → Button "Next month" → Button "Today" → RadioButton "Month" → RadioButton "Week" → RadioButton "Day" → Button "16 November 2026".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Month | Button "Previous month" | 32.0 × 32.0 | radius-md | transparent | — | — | chevron-left 14.0px ink | 9.0 |
| Month | Button "Next month" | 32.0 × 32.0 | radius-md | transparent | — | — | chevron-right 14.0px ink | 9.0 |
| Month | Label "November 2026" | 133.0 × 24.0 | — | — | — | Inter 600 17.0px ink | — | 0.0 |
| Month | Button "Today" | 63.0 × 32.0 | radius-md | transparent | — | Inter 600 13.0px ink | — | space-3 |
| Month | RadioButton "Month" | 69.0 × 30.0 | 7.0 | bg-200 | 1.0 border | Inter 600 13.0px ink | — | 14.0 |
| Month | RadioButton "Week" | 64.0 × 30.0 | — | — | — | Inter 600 13.0px ink-muted | — | 14.0 |
| Month | Button "1 November 2026" | 66.0 × 104.0 | radius-sm | transparent | 1.0 border | Inter 600 12.0px ink | — | space-3 |
| Month | Button "2 November 2026" | 66.0 × 104.0 | radius-sm | transparent | 1.0 border | Inter 600 12.0px ink | — | 11.0 |
| Month | Button "10 November 2026" | 66.0 × 104.0 | radius-sm | transparent | 1.0 border | Inter 600 12.0px ink | — | space-2 |
| Month | Button "11 November 2026" | 66.0 × 104.0 | radius-sm | transparent | 1.0 border | Inter 600 12.0px ink | — | 10.0 |
| Month | Button "12 November 2026" | 66.0 × 104.0 | radius-sm | transparent | 1.0 border | Inter 600 12.0px ink | check 12.0px ink-muted | space-2 |
| Month | Button "Rent" | 58.0 × 20.0 | radius-sm | ink-muted@8 | — | Inter 600 12.0px ink-muted | check 12.0px ink-muted | 6.0 |
| Month | Button "16 November 2026" | 66.0 × 104.0 | radius-sm | transparent | 1.5 accent | Inter 600 12.0px on-accent | — | space-2 |
| Month | Button "Review" | 58.0 × 20.0 | radius-sm | warning@16 | — | Inter 600 12.0px ink | — | 6.0 |
| Month | Button "17 November 2026" | 66.0 × 104.0 | radius-sm | transparent | 1.0 border | Inter 600 12.0px ink | — | 9.0 |
| Month | Button "Salary" | 58.0 × 20.0 | radius-sm | success@16 | — | Inter 600 12.0px ink | — | 6.0 |
| Month | Button "20 November 2026" | 66.0 × 104.0 | radius-sm | transparent | 1.0 border | Inter 600 12.0px ink | — | 7.0 |
| Month | Button "1 December 2026" | 66.0 × 104.0 | radius-sm | ink@2 | 1.0 border | Inter 600 12.0px ink-muted | — | space-3 |
| Month | Button "2 December 2026" | 66.0 × 104.0 | radius-sm | ink@2 | 1.0 border | Inter 600 12.0px ink-muted | — | 11.0 |
| Agenda | Label "12 November 2026" | 794.0 × 18.0 | — | — | — | Inter 600 13.0px ink | — | 0.0 |
| Agenda | Label "All day" | 43.0 × 16.0 | — | — | — | Inter 600 12.0px ink-muted | — | 0.0 |
| Agenda | Label "16 November 2026" | 794.0 × 18.0 | — | — | — | Inter 600 13.0px accent | — | 0.0 |
| Year | Button "1 January 2026" | 26.0 × 26.0 | radius-sm | transparent | — | Inter 600 12.0px ink | — | 10.0 |
| Year | Button "2 January 2026" | 26.0 × 26.0 | radius-sm | transparent | — | Inter 600 12.0px ink | — | 9.0 |
| Year | Button "10 January 2026" | 26.0 × 26.0 | radius-sm | transparent | — | Inter 600 12.0px ink | — | 6.0 |
| Year | Button "11 January 2026" | 26.0 × 26.0 | radius-sm | transparent | — | Inter 600 12.0px ink | — | space-2 |
| Year | Button "17 January 2026" | 26.0 × 26.0 | radius-sm | transparent | — | Inter 600 12.0px ink | — | 7.0 |
| Year | Button "20 January 2026" | 26.0 × 26.0 | radius-sm | transparent | — | Inter 600 12.0px ink | — | 5.0 |
| Year | Button "16 November 2026" | 26.0 × 26.0 | radius-sm | accent | — | Inter 600 12.0px on-accent | — | 6.0 |

## API

Builder methods: `events`, `events_per_day`, `hemisphere`, `hours`, `new`, `on_create`, `on_event`, `on_move`, `today`, `view`, `views`.

## Example

```rust
fn event_calendar() -> impl View {
    let today = Date::new(2026, 11, 16).unwrap_or_else(|| unreachable!("a real date"));
    let (month, day) = (State::new(today.first_of_month()), State::new(Some(today)));
    EventCalendar::new(month.binding(), day.binding()).today(today).events([
        CalendarEvent::new(1, today.add_days(-4), "Rent").done(true),
        CalendarEvent::new(2, today, "Review").tone(Tone::Warning),
        CalendarEvent::new(3, today.add_days(3), "Salary").tone(Tone::Success),
    ])
}
```

Every number above is in `EventCalendar.json` beside this card.
