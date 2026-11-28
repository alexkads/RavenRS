# ExpressionEditor

Writes an expression with fields, functions and operators at hand.

**Family:** Actions and inputs · **Rust:** `ravenrs::prelude::ExpressionEditor`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `preview` | text | `` | Empty leaves it as the component has it. |
| `preview_error` | text | `` | Empty leaves it as the component has it. |
| `rows` | number 0–20 by 1 | `5` |  |
| `expression` | text | `[Quantity] * [UnitPrice]` | How the example starts. |

## Stories

- **Example**: 364 × 510 pt, settings `{"expression":"[Quantity] * [UnitPrice]","preview":"","preview_error":"","rows":5.0}`

## Sizing

Hugs its content: 364 × 510 pt in a parent of 760 pt; in a parent that stretches its children, as a form column, it takes the whole width.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover** on "Expression": border {color:control-border,width:1.0} → {color:mix(ink, control-border, 25),width:1.0}.
- **Focus** on "Expression": border {color:control-border,width:1.0} → {color:accent,width:1.5}; adds a rect (accent); adds a rect (border 2.0 focus-ring).
- **Pressed** on "Expression": border {color:control-border,width:1.0} → {color:accent,width:1.5}; adds a rect (accent).

## Behavior

- **Keys** its first control handles once focused: `ArrowUp`, `ArrowLeft`, `Enter`, `Space`, `Escape`, `Home`.
- **Accessibility**: role `TextInput` "Expression".
- **Focus order** (Tab): TextInput "Expression" → Button "Fields" → Button "Parameters" → Button "Logical" → Button "Text" → Button "Math" → Button "Date and time" → Button "Reporting" → Button "Aggregates" → Button "Operators" → Button "Constants" → TextInput "Search" → Button "Quantity" → Button "Unit price".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | TextInput "Expression" | 364.0 × 122.0 | radius-md | bg-300 | 1.0 control-border | Inter 400 15.0px ink | — | space-3 |
| Example | Status | 364.0 × 24.0 | radius-sm | success@15 | — | Inter 600 12.0px mix(success, ink, 70) | circle-check 14.0px success | 10.0 |
| Example | Label "The expression is valid." | 146.0 × 16.0 | — | — | — | Inter 600 12.0px mix(success, ink, 70) | — | 0.0 |
| Example | Button "Fields" | 140.0 × 32.0 | radius-md | bg-300 | — | Inter 600 13.0px ink | — | space-2 |
| Example | Button "Parameters" | 140.0 × 32.0 | radius-md | transparent | — | Inter 400 13.0px ink-muted | — | space-2 |
| Example | TextInput "Search" | 200.0 × 32.0 | radius-md | bg-300 | 1.0 control-border | Inter 400 13.0px ink-muted | search 16.0px ink-muted | space-2 |

## API

Builder methods: `field`, `fields`, `new`, `parameter`, `parameters`, `preview`, `preview_error`, `rows`.

## Example

```rust
fn expression_editor() -> impl View {
    let expression = State::new("[Quantity] * [UnitPrice]".to_owned());
    ExpressionEditor::new(expression.binding())
        .field(FilterField::new("Quantity", "Quantity", FilterFieldKind::Number))
        .field(FilterField::new("UnitPrice", "Unit price", FilterFieldKind::Number))
        .parameter(FilterField::new("MinTotal", "Least total", FilterFieldKind::Number))
}
```

Every number above is in `ExpressionEditor.json` beside this card.
