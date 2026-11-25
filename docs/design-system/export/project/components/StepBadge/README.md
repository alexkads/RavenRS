# StepBadge

The round mark of a step.

**Family:** Actions and inputs · **Rust:** `ravenrs::prelude::StepBadge`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `state` | Upcoming · Current · Done · Error | `Done` |  |
| `size` | number 0–96 by 1 | `24` |  |

## Stories

- **Example**: 120 × 24 pt, settings `{"size":24.0,"state":"Done"}`
- **State: Upcoming**: 120 × 24 pt, settings `{"size":24.0,"state":"Upcoming"}`
- **State: Current**: 120 × 24 pt, settings `{"size":24.0,"state":"Current"}`
- **State: Error**: 120 × 24 pt, settings `{"size":24.0,"state":"Error"}`

## Sizing

Hugs its content: 120 × 24 pt in a parent of 760 pt.

## Behavior

- **Accessibility**: role `Image` "1".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Image "1" | 24.0 × 24.0 | radius-full | accent | — | — | check 14.0px on-accent | 5.0 |
| Example | Image "2" | 24.0 × 24.0 | radius-full | transparent | 1.0 danger | — | circle-alert 14.0px danger | 5.0 |
| Example | Image "3" | 24.0 × 24.0 | radius-full | accent | — | Inter 600 13.0px on-accent | — | space-2 |
| Example | Label "3" | 9.0 × 18.0 | — | — | — | Inter 600 13.0px on-accent | — | 0.0 |
| Example | Image "4" | 24.0 × 24.0 | radius-full | ink@8 | — | Inter 600 13.0px ink-muted | — | space-2 |
| Example | Label "4" | 9.0 × 18.0 | — | — | — | Inter 600 13.0px ink-muted | — | 0.0 |
| State: Upcoming | Image "1" | 24.0 × 24.0 | radius-full | ink@8 | — | Inter 600 13.0px ink-muted | — | 9.0 |
| State: Current | Image "1" | 24.0 × 24.0 | radius-full | accent | — | Inter 600 13.0px on-accent | — | 9.0 |

## API

Builder methods: `new`, `size`, `state`.

## Example

```rust
fn step_badge() -> impl View {
    HStack::new((
        StepBadge::new(1).state(StepState::Done),
        StepBadge::new(2).state(StepState::Error),
        StepBadge::new(3).state(StepState::Current),
        StepBadge::new(4),
    ))
    .spacing(8.0)
}
```

Every number above is in `StepBadge.json` beside this card.
