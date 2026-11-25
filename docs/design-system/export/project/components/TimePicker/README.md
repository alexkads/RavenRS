# TimePicker

A time of day, typed or picked from a list.

**Family:** Actions and inputs · **Rust:** `ravenrs::prelude::TimePicker`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `step_minutes` | number 0–60 by 1 | `15` |  |
| `label` | text | `Starts at` |  |
| `disabled` | on/off | `false` |  |
| `read_only` | on/off | `false` |  |
| `width` | number 0–480 by 1 | `120` |  |

## Stories

- **Example**: 204 × 40 pt, settings `{"disabled":false,"label":"Starts at","read_only":false,"step_minutes":15.0,"width":120.0}`
- **Disabled**: 204 × 40 pt, settings `{"disabled":true,"label":"Starts at","read_only":false,"step_minutes":15.0,"width":120.0}`
- **Read only**: 204 × 40 pt, settings `{"disabled":false,"label":"Starts at","read_only":true,"step_minutes":15.0,"width":120.0}`

## Sizing

Hugs its content: 204 × 40 pt in a parent of 760 pt.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover** on "Starts at": border {color:control-border,width:1.0} → {color:mix(ink, control-border, 25),width:1.0}.
- **Focus** on "Starts at": border {color:control-border,width:1.0} → {color:accent,width:1.5}; adds a rect (accent); adds a rect (border 2.0 focus-ring).
- **Pressed** on "Starts at": border {color:control-border,width:1.0} → {color:accent,width:1.5}; adds a rect (accent).

## Behavior

- **Keys** its first control handles once focused: `ArrowUp`, `ArrowLeft`, `Escape`, `Home`.
- **Accessibility**: role `TextInput` "Starts at".
- **Focus order** (Tab): TextInput "Starts at" → Button "Pick a time".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | TextInput "Starts at" | 160.0 × 40.0 | radius-md | bg-300 | 1.0 control-border | Inter 400 15.0px ink | — | space-3 |
| Example | Button "Pick a time" | 40.0 × 40.0 | radius-md | transparent | — | — | clock 16.0px ink | space-3 |
| Disabled | TextInput "Starts at" | 160.0 × 40.0 | radius-md | bg-300@45 | 1.0 control-border@45 | Inter 400 15.0px ink@45 | — | space-3 |
| Disabled | Button "Pick a time" | 40.0 × 40.0 | radius-md | transparent | — | — | clock 16.0px ink@45 | space-3 |
| Read only | TextInput "Starts at" | 160.0 × 40.0 | radius-md | bg-300@50 | 1.0 control-border | Inter 400 15.0px ink | — | space-3 |

## API

Builder methods: `disabled`, `label`, `new`, `read_only`, `step_minutes`, `width`.

## Example

```rust
fn time_picker() -> impl View {
    let at = State::new(Time::new(9, 30));
    TimePicker::new(at.binding()).step_minutes(15).label("Starts at")
}
```

Every number above is in `TimePicker.json` beside this card.
