# FormField

Labeled field with a hint and validation messages.

**Family:** Forms · **Rust:** `ravenrs::prelude::FormField`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `value` | text | `` | What is typed; an address with no domain shows the error. |
| `server_error` | text | `` | An error found elsewhere, as by rules that read a database. |
| `required` | on/off | `true` |  |
| `hint` | text | `We send the invoice here.` |  |
| `show_errors` | on/off | `false` |  |

## Stories

- **Example**: 165 × 88 pt, settings `{"hint":"We send the invoice here.","required":true,"server_error":"","show_errors":false,"value":""}`
- **Filled**: 165 × 88 pt, settings `{"hint":"We send the invoice here.","required":true,"server_error":"","show_errors":false,"value":"ana@company.com"}`
- **Error**: 272 × 89 pt, settings `{"hint":"We send the invoice here.","required":true,"server_error":"","show_errors":true,"value":"ana@"}`
- **Server error**: 237 × 89 pt, settings `{"hint":"We send the invoice here.","required":true,"server_error":"This email already has an account.","show_errors":false,"value":"ana@company.com"}`

## Sizing

Hugs its content: 165 × 88 pt in a parent of 760 pt; in a parent that stretches its children, as a form column, it takes the whole width.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover** on "Work email": border {color:control-border,width:1.0} → {color:mix(ink, control-border, 25),width:1.0}.
- **Focus** on "Work email": border {color:control-border,width:1.0} → {color:accent,width:1.5}; adds a rect (accent); adds a rect (border 2.0 focus-ring).
- **Pressed** on "Work email": border {color:control-border,width:1.0} → {color:accent,width:1.5}; adds a rect (accent).

## Behavior

- **Keys** its first control handles once focused: `Space`, `Escape`.
- **Accessibility**: role `Label` "Work email".
- **Focus order** (Tab): TextInput "Work email".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Label "Work email" | 71.0 × 18.0 | — | — | — | Inter 600 13.0px ink | — | 0.0 |
| Example | Label "*" | 8.0 × 18.0 | — | — | — | Inter 600 13.0px danger | — | 0.0 |
| Example | TextInput "Work email" | 165.0 × 40.0 | radius-md | bg-300 | 1.0 control-border | Inter 400 15.0px ink-muted | — | space-3 |
| Example | Label "We send the invoice here." | 165.0 × 18.0 | — | — | — | Inter 400 13.0px ink-muted | — | 0.0 |
| Filled | TextInput "Work email" | 165.0 × 40.0 | radius-md | bg-300 | 1.0 control-border | Inter 400 15.0px ink | — | space-3 |
| Error | Label "Enter an email like name@example.com." | 248.0 × 18.0 | — | — | — | Inter 400 13.0px danger | — | 0.0 |

## API

Builder methods: `accessible_description`, `accessible_name`, `error`, `error_text`, `hint`, `label_ink`, `new`, `note_text`, `required`, `show_errors`, `tint`, `validate`.

## Example

```rust
fn form_field() -> impl View {
    let email = State::new(String::new());
    FormField::new("Work email", TextField::new(email.binding()).placeholder("you@company.com"))
        .required()
        .hint("We send the invoice here.")
        .validate(email.binding(), validate::email)
}
```

Every number above is in `FormField.json` beside this card.
