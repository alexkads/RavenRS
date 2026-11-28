# ParameterForm

A form made from a schema of parameters: groups, lists of rows, options that depend on another field, and a value or an expression in each field.

**Family:** Actions and inputs · **Rust:** `ravenrs::prelude::ParameterForm`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `live` | on/off | `false` | Each change goes back at once, with no Apply or Discard: for a form inside a dialog. |
| `several` | on/off | `false` | A field of several choices, kept as a list: the suites of a test run. |

## Stories

- **Example**: 211 × 408 pt, settings `{"live":false,"several":false}`
- **Live, inside a dialog**: 184 × 372 pt, settings `{"live":true,"several":false}`
- **Several choices**: 211 × 484 pt, settings `{"live":false,"several":true}`

## Sizing

Hugs its content: 211 × 408 pt in a parent of 760 pt; in a parent that stretches its children, as a form column, it takes the whole width.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover** on "Address": border {color:control-border,width:1.0} → {color:mix(ink, control-border, 25),width:1.0}.
- **Focus** on "Address": border {color:control-border,width:1.0} → {color:accent,width:1.5}; adds a rect (accent); adds a rect (border 2.0 focus-ring).
- **Pressed** on "Address": border {color:control-border,width:1.0} → {color:accent,width:1.5}; adds a rect (accent).

## Behavior

- **Events**: `on_apply`, `on_discard`, `on_load`.
- **Keys** its first control handles once focused: `Space`, `Escape`.
- **Accessibility**: role `Label` "Address".
- **Focus order** (Tab): TextInput "Address" → TextInput "Retries" → RadioButton "Fixed" → RadioButton "Expression" → TextInput "Only when" → Button "Add".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Label "Address" | 53.0 × 18.0 | — | — | — | Inter 600 13.0px ink | — | 0.0 |
| Example | Label "*" | 8.0 × 18.0 | — | — | — | Inter 600 13.0px danger | — | 0.0 |
| Example | TextInput "Address" | 211.0 × 40.0 | radius-md | bg-300 | 1.0 control-border | — | — | — |
| Example | TextInput "Retries" | 211.0 × 40.0 | radius-md | bg-300 | 1.0 control-border | Inter 400 15.0px ink | — | space-3 |
| Example | RadioButton "Fixed" | 62.0 × 30.0 | 7.0 | bg-200 | 1.0 border | Inter 600 13.0px ink | — | 14.0 |
| Example | RadioButton "Expression" | 98.0 × 30.0 | — | — | — | Inter 600 13.0px ink-muted | — | 14.0 |
| Example | Heading "Headers" | 70.0 × 24.0 | — | — | — | Inter 600 17.0px ink | — | 0.0 |
| Example | Button "Add" | 72.0 × 32.0 | radius-md | bg-300 | 1.0 border | Inter 600 13.0px ink | plus 14.0px ink | space-3 |
| Example | Button "Discard" | 92.0 × 40.0 | radius-md | transparent | — | Inter 600 15.0px ink@45 | — | 18.0 |
| Example | Button "Apply" | 103.0 × 40.0 | radius-md | accent@45 | — | Inter 600 15.0px on-accent@45 | check 16.0px on-accent@45 | 18.0 |
| Live, inside a dialog | Label "Address is required" | 120.0 × 18.0 | — | — | — | Inter 400 13.0px danger | — | 0.0 |
| Several choices | ComboBox "Suites" | 211.0 × 40.0 | radius-md | bg-300 | 1.0 control-border | Inter 400 13.0px ink | x 12.0px ink | 14.0 |
| Several choices | Button "Remove Orders" | 22.0 × 22.0 | radius-md | transparent | — | — | x 12.0px ink | 5.0 |

## API

Builder methods: `live`, `new`, `on_apply`, `on_discard`, `on_load`, `options`.

## Example

```rust
fn parameter_form() -> impl View {
    let values = State::new(ParamValues::new());
    ParameterForm::new(
        vec![
            ParamField::new("url", "Address", ParamKind::Text).required(),
            ParamField::new("retries", "Retries", ParamKind::Number).default(3),
            ParamField::new("when", "Only when", ParamKind::Text).expressions(),
            ParamField::new(
                "headers",
                "Headers",
                ParamKind::List(vec![
                    ParamField::new("name", "Name", ParamKind::Text),
                    ParamField::new("value", "Value", ParamKind::Text),
                ]),
            ),
        ],
        values.binding(),
    )
}
```

Every number above is in `ParameterForm.json` beside this card.
