# Scheduler

Timeline of resources: drag, resize and draw events, zoom the time axis.

**Family:** Forms · **Rust:** `ravenrs::prelude::Scheduler`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `avatars` | on/off | `false` |  |
| `card` | on/off | `false` |  |
| `fill_rows` | on/off | `false` |  |
| `read_only` | on/off | `false` |  |
| `resource_width` | number 0–800 by 1 | `200` |  |
| `toolbar` | on/off | `true` |  |
| `axis` | Days · Quarter hours | `Days` |  |
| `unavailable` | on/off | `false` | A resource that takes nothing now. |
| `compact` | on/off | `false` |  |
| `legend` | text | `` | Empty shows none. |
| `resources_title` | text | `` | Empty leaves the labels' word. |

## Stories

- **Example**: 1392 × 244 pt, settings `{"avatars":false,"axis":"Days","card":false,"compact":false,"fill_rows":false,"legend":"","read_only":false,"resource_width":200.0,"resources_title":"","toolbar":true,"unavailable":false}`
- **Read only**: 1392 × 244 pt, settings `{"avatars":false,"axis":"Days","card":false,"compact":false,"fill_rows":false,"legend":"","read_only":true,"resource_width":200.0,"resources_title":"","toolbar":true,"unavailable":false}`
- **Agents at a glance**: 680 × 236 pt, settings `{"avatars":false,"axis":"Quarter hours","card":false,"compact":true,"fill_rows":false,"legend":"The vertical line is now","read_only":true,"resource_width":200.0,"resources_title":"AGENTS","toolbar":true,"unavailable":true}`
- **Quarter hours**: 680 × 284 pt, settings `{"avatars":false,"axis":"Quarter hours","card":false,"compact":false,"fill_rows":false,"legend":"","read_only":true,"resource_width":200.0,"resources_title":"","toolbar":true,"unavailable":true}`

## Sizing

Hugs its content: 1392 × 244 pt in a parent of 760 pt; its height follows the width, as its content wraps.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover** on "Previous month": fill transparent → ink@7.
- **Focus** on "Previous month": adds a rect (border 2.0 focus-ring).
- **Pressed** on "Previous month": fill transparent → ink@12, rect [8.0,8.0,32.0,32.0] → [8.48,8.48,31.04,31.04].

## Behavior

- **Events**: `on_change`, `on_create`, `on_delete`, `on_open`.
- **Keys** its first control handles once focused: `Enter`, `Space`.
- **Accessibility**: role `Button` "Previous month".
- **Focus order** (Tab): Button "Previous month" → Button "Next month" → Button "Zoom out" → Button "Zoom in" → Button "Vacation".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Button "Previous month" | 32.0 × 32.0 | radius-md | transparent | — | — | chevron-left 14.0px ink | 9.0 |
| Example | Button "Next month" | 32.0 × 32.0 | radius-md | transparent | — | — | chevron-right 14.0px ink | 9.0 |
| Example | Label "Nov 1, 2026 – Nov 30, 2026" | 226.0 × 24.0 | — | — | — | Inter 600 17.0px ink | — | 0.0 |
| Example | Button "Zoom out" | 32.0 × 32.0 | — | — | — | — | — | — |
| Example | Label "Resources" | 65.0 × 16.0 | — | — | — | Inter 600 12.0px ink-muted | — | 0.0 |
| Example | Label "November 2026" | 100.0 × 16.0 | — | — | — | Inter 600 12.0px ink | — | 0.0 |
| Example | Label "16" | 14.0 × 16.0 | — | — | — | — | — | — |
| Example | Row "Ana" | 200.0 × 68.0 | — | — | — | Inter 400 13.0px ink | — | 10.0 |
| Example | Label "Ana" | 180.0 × 18.0 | — | — | — | Inter 400 13.0px ink | — | 0.0 |
| Example | Row "Bruno" | 200.0 × 38.0 | — | — | — | Inter 400 13.0px ink | — | 10.0 |
| Example | Button "Vacation" | 160.0 × 26.0 | radius-sm | success@80 | — | Inter 600 12.0px on-accent | — | space-2 |
| Example | Button "Workshop" | 200.0 × 26.0 | radius-sm | #8196f9cc | — | Inter 600 12.0px on-accent | — | space-2 |
| Example | Button "On call" | 160.0 × 26.0 | radius-sm | warning@80 | — | Inter 600 12.0px on-accent | — | space-2 |
| Agents at a glance | Label "The vertical line is now" | 141.0 × 18.0 | — | — | — | Inter 400 13.0px ink-muted | — | 0.0 |
| Agents at a glance | Button "site #56" | 68.0 × 26.0 | radius-sm | ink-muted@16 | — | Inter 600 12.0px ink | check 12.0px ink-muted | space-2 |
| Quarter hours | Button "Today" | 63.0 × 32.0 | radius-md | transparent | — | Inter 600 13.0px ink | — | space-3 |
| Quarter hours | Button "Zoom out" | 32.0 × 32.0 | radius-md | transparent | — | — | zoom-out 14.0px ink | 9.0 |
| Quarter hours | Button "Zoom in" | 32.0 × 32.0 | radius-md | transparent | — | — | zoom-in 14.0px ink | 9.0 |
| Quarter hours | Row "oficina-1" | 200.0 × 46.0 | — | — | — | Inter 400 13.0px ink | — | 10.0 |

## API

Builder methods: `avatars`, `can_drop`, `card`, `compact`, `event_detail`, `event_menu`, `events`, `fill_rows`, `lane_menu`, `legend`, `new`, `now`, `on_change`, `on_create`, `on_delete`, `on_open`, `ranges`, `read_only`, `resource_width`, `resources`, `resources_title`, `selection`, `toolbar`.

## Example

```rust
fn scheduler() -> impl View {
    let first = Date::new(2026, 11, 1).unwrap_or_else(|| unreachable!("a real date"));
    let at = |day: i64| TimePoint::from_date(first).plus_days(day);
    let axis = State::new(TimeAxis::days(first, 30));
    Scheduler::new(axis.binding())
        .resources([Resource::new(1, "Ana"), Resource::new(2, "Bruno"), Resource::new(3, "Carla")])
        .events([
            SchedulerEvent::new(1, 1, at(1), at(5), "Vacation").tone(Tone::Success),
            SchedulerEvent::new(2, 1, at(3), at(8), "Workshop"),
            SchedulerEvent::new(3, 2, at(2), at(6), "On call").tone(Tone::Warning),
        ])
        .on_change(|change| println!("{change:?}"))
}
```

Every number above is in `Scheduler.json` beside this card.
