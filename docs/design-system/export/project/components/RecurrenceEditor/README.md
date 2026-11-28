# RecurrenceEditor

Edits how something repeats: how often, on which days, until when.

**Family:** Forms · **Rust:** `ravenrs::prelude::RecurrenceEditor`

## Stories

- **Example**: 361 × 226 pt, settings `{}`

## Sizing

Hugs its content: 361 × 226 pt in a parent of 760 pt; in a parent of 320 pt it narrows to 356 pt; in a parent that stretches its children, as a form column, it takes the whole width; its height follows the width, as its content wraps.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover** on "Once": color ink-muted → ink.
- **Focus** on "Once": adds a rect (border 2.0 focus-ring).
- **Pressed** on "Once": color ink-muted → ink.

## Behavior

- **Keys** its first control handles once focused: `ArrowDown`, `ArrowUp`, `ArrowLeft`, `ArrowRight`, `Enter`, `Space`, `Home`, `End`.
- **Accessibility**: role `RadioGroup` "Repeats".
- **Focus order** (Tab): RadioButton "Once" → RadioButton "Daily" → RadioButton "Weekly" → RadioButton "Monthly" → RadioButton "Yearly" → SpinButton "Every" → Button "Decrease" → Button "Increase" → Button "Su" → Button "Mo" → Button "Tu" → Button "We" → Button "Th" → Button "Fr" → Button "Sa" → RadioButton "Never" → RadioButton "On a date" → RadioButton "After".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | RadioButton "Once" | 62.0 × 30.0 | — | — | — | Inter 600 13.0px ink-muted | — | 14.0 |
| Example | RadioButton "Weekly" | 75.0 × 30.0 | 7.0 | bg-200 | 1.0 border | Inter 600 13.0px ink | — | 14.0 |
| Example | Label "Every" | 35.0 × 18.0 | — | — | — | Inter 400 13.0px ink | — | 0.0 |
| Example | SpinButton "Every" | 110.0 × 38.0 | radius-md | bg-300 | 1.0 control-border | Inter 600 15.0px ink | minus 14.0px ink | space-3 |
| Example | Button "Decrease" | 32.0 × 32.0 | radius-md | transparent | — | — | minus 14.0px ink | 9.0 |
| Example | Button "Increase" | 32.0 × 32.0 | radius-md | transparent | — | — | plus 14.0px ink | 9.0 |
| Example | Button "Su" | 41.0 × 34.0 | 8.0 | transparent | — | Inter 600 13.0px ink-muted | — | space-3 |
| Example | Button "Mo" | 44.0 × 34.0 | 8.0 | bg-300 | 1.0 border | Inter 600 13.0px ink | — | space-3 |
| Example | Label "Ends" | 32.0 × 18.0 | — | — | — | Inter 600 13.0px ink | — | 0.0 |
| Example | Label "Every week. Next: Nov 5, 2026, Nov 9, 2026, Nov 12, 2026" | 361.0 × 18.0 | — | — | — | Inter 400 13.0px ink-muted | — | 0.0 |

## API

Builder methods: `new`.

## Example

```rust
fn recurrence_editor() -> impl View {
    let start = Date::new(2026, 11, 5).unwrap_or_else(|| unreachable!("a real date"));
    let rule = State::new(RecurrenceRule::new(Frequency::Weekly).on_weekdays([1, 4]));
    RecurrenceEditor::new(rule.binding(), start)
}
```

Every number above is in `RecurrenceEditor.json` beside this card.
