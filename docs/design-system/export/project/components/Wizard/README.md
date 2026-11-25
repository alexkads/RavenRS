# Wizard

A task in steps that can refuse to be left.

**Family:** Actions and inputs · **Rust:** `ravenrs::prelude::Wizard`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `finish_label` | text | `` | Empty leaves it as the component has it. |
| `step` | number 0–5 by 1 | `0` | How the example starts. |

## Stories

- **Example**: 342 × 168 pt, settings `{"finish_label":"","step":0.0}`

## Sizing

Hugs its content: 342 × 168 pt in a parent of 760 pt; in a parent of 320 pt it narrows to 288 pt; in a parent that stretches its children, as a form column, it takes the whole width; its height follows the width, as its content wraps.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover** on "Name": border {color:control-border,width:1.0} → {color:mix(ink, control-border, 25),width:1.0}.
- **Focus** on "Name": border {color:control-border,width:1.0} → {color:accent,width:1.5}; adds a rect (accent); adds a rect (border 2.0 focus-ring).
- **Pressed** on "Name": border {color:control-border,width:1.0} → {color:accent,width:1.5}; adds a rect (accent).

## Behavior

- **Events**: `on_cancel`, `on_finish`.
- **Keys** its first control handles once focused: `Space`, `Escape`.
- **Accessibility**: role `List` "Step 1 of 3".
- **Focus order** (Tab): TextInput "Name" → Button "Next".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | ListItem "Account" | 97.0 × 32.0 | radius-sm | transparent | — | Inter 600 13.0px on-accent | — | 15.0 |
| Example | ListItem "Opening balance" | 149.0 × 32.0 | radius-sm | transparent | — | Inter 600 13.0px ink-muted | — | 14.0 |
| Example | Label "Name" | 37.0 × 18.0 | — | — | — | Inter 600 13.0px ink | — | 0.0 |
| Example | TextInput "Name" | 342.0 × 40.0 | radius-md | bg-300 | 1.0 control-border | — | — | — |
| Example | Button "Next" | 71.0 × 40.0 | radius-md | accent | — | Inter 600 15.0px on-accent | — | 18.0 |

## API

Builder methods: `finish_label`, `new`, `on_cancel`, `on_finish`.

## Example

```rust
fn wizard() -> impl View {
    let step = State::new(0);
    let name = State::new(String::new());
    let filled = name.clone();
    Wizard::new(
        step.binding(),
        vec![
            WizardStep::new("Account", FormField::new("Name", TextField::new(name.binding()))).check(
                move || {
                    if filled.get().trim().is_empty() { Err("Fill in the name".to_owned()) } else { Ok(()) }
                },
            ),
            WizardStep::new("Opening balance", Text::new("How much it holds today")),
            WizardStep::new("Review", Text::new("Everything is ready")),
        ],
    )
    .on_finish(|| {})
}
```

Every number above is in `Wizard.json` beside this card.
