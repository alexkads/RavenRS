# StepIndicator

Where a task in steps stands, for an app's own layout.

**Family:** Actions and inputs · **Rust:** `ravenrs::prelude::StepIndicator`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `vertical` | on/off | `true` |  |

## Stories

- **Example**: 145 × 152 pt, settings `{"vertical":true}`

## Sizing

Hugs its content: 145 × 152 pt in a parent of 760 pt; in a parent that stretches its children, as a form column, it takes the whole width.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover** on "Client": fill transparent → ink@5.
- **Focus** on "Client": adds a rect (border 2.0 focus-ring); color on-accent → danger.
- **Pressed** on "Client": fill transparent → ink@5.

## Behavior

- **Events**: `on_select`.
- **Keys** its first control handles once focused: `Escape`.
- **Accessibility**: role `List` "Step 3 of 4".
- **Focus order** (Tab): Button "Client" → Button "Photos" → Button "Review" → Button "Send".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Button "Client" | 145.0 × 32.0 | radius-sm | transparent | — | Inter 400 13.0px ink | check 14.0px on-accent | 11.0 |
| Example | Button "Photos" | 145.0 × 44.0 | radius-sm | transparent | — | Inter 400 13.0px ink | circle-alert 14.0px danger | 11.0 |
| Example | Button "Review" | 145.0 × 32.0 | radius-sm | transparent | — | Inter 600 13.0px on-accent | — | 14.0 |
| Example | Button "Send" | 145.0 × 32.0 | radius-sm | transparent | — | Inter 600 13.0px ink-muted | — | 14.0 |

## API

Builder methods: `new`, `on_select`, `vertical`.

## Example

```rust
fn step_indicator() -> impl View {
    StepIndicator::new(vec![
        IndicatorStep::new("Client", StepState::Done),
        IndicatorStep::new("Photos", StepState::Error).note("Two are missing"),
        IndicatorStep::new("Review", StepState::Current),
        IndicatorStep::new("Send", StepState::Upcoming),
    ])
    .vertical()
    .on_select(|_step| {})
}
```

Every number above is in `StepIndicator.json` beside this card.
