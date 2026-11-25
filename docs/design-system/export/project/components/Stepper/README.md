# Stepper

Decrement, value and increment for small quantities.

**Family:** Actions and inputs · **Rust:** `ravenrs::prelude::Stepper`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `label` | text | `Seats` |  |
| `disabled` | on/off | `false` |  |
| `read_only` | on/off | `false` |  |
| `seats` | number 0–12 by 1 | `3` | How the example starts. |

## Stories

- **Example**: 110 × 38 pt, settings `{"disabled":false,"label":"Seats","read_only":false,"seats":3.0}`
- **Disabled**: 110 × 38 pt, settings `{"disabled":true,"label":"Seats","read_only":false,"seats":3.0}`
- **Read only**: 110 × 38 pt, settings `{"disabled":false,"label":"Seats","read_only":true,"seats":3.0}`

## Sizing

Hugs its content: 110 × 38 pt in a parent of 760 pt.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover**: "Seats" does not change.
- **Focus** on "Seats": adds a rect (border 2.0 focus-ring).
- **Pressed**: "Seats" does not change.

## Behavior

- **Accessibility**: role `SpinButton` "Seats".
- **Focus order** (Tab): SpinButton "Seats" → Button "Decrease" → Button "Increase".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | SpinButton "Seats" | 110.0 × 38.0 | radius-md | bg-300 | 1.0 control-border | Inter 600 15.0px ink | minus 14.0px ink | space-3 |
| Example | Button "Decrease" | 32.0 × 32.0 | radius-md | transparent | — | — | minus 14.0px ink | 9.0 |
| Example | Button "Increase" | 32.0 × 32.0 | radius-md | transparent | — | — | plus 14.0px ink | 9.0 |
| Disabled | SpinButton "Seats" | 110.0 × 38.0 | radius-md | bg-300@45 | 1.0 control-border@45 | Inter 600 15.0px ink@45 | minus 14.0px ink@45 | space-3 |
| Disabled | Button "Decrease" | 32.0 × 32.0 | radius-md | transparent | — | — | minus 14.0px ink@45 | 9.0 |
| Disabled | Button "Increase" | 32.0 × 32.0 | radius-md | transparent | — | — | plus 14.0px ink@45 | 9.0 |
| Read only | SpinButton "Seats" | 110.0 × 38.0 | radius-md | bg-300@50 | 1.0 control-border | Inter 600 15.0px ink | minus 14.0px ink@45 | space-3 |

## API

Builder methods: `disabled`, `label`, `new`, `range`, `read_only`.

## Example

```rust
fn stepper() -> impl View {
    let seats = State::new(3);
    Stepper::new(seats.binding()).range(1, 50).label("Seats")
}
```

Every number above is in `Stepper.json` beside this card.
