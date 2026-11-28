# DateTimePicker

A date and a time over one value.

**Family:** Actions and inputs · **Rust:** `ravenrs::prelude::DateTimePicker`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `label` | text | `Starts` |  |
| `disabled` | on/off | `false` |  |
| `read_only` | on/off | `false` |  |
| `step_minutes` | number 0–120 by 1 | `30` |  |

## Stories

- **Example**: 728 × 40 pt, settings `{"disabled":false,"label":"Starts","read_only":false,"step_minutes":30.0}`
- **Disabled**: 728 × 40 pt, settings `{"disabled":true,"label":"Starts","read_only":false,"step_minutes":30.0}`
- **Read only**: 728 × 40 pt, settings `{"disabled":false,"label":"Starts","read_only":true,"step_minutes":30.0}`

## Sizing

Fills the width of its parent: 288, 728, 1168 pt wide in parents of 320, 760, 1200 pt; its height follows the width, as its content wraps.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover** on "Starts": border {color:control-border,width:1.0} → {color:mix(ink, control-border, 25),width:1.0}.
- **Focus** on "Starts": adds a rect (border 2.0 focus-ring).
- **Pressed** on "Starts": border {color:control-border,width:1.0} → {color:mix(ink, control-border, 25),width:1.0}.

## Behavior

- **Keys** its first control handles once focused: `Enter`, `Space`.
- **Accessibility**: role `ComboBox` "Starts".
- **Focus order** (Tab): ComboBox "Starts" → TextInput "Starts" → Button "Pick a time".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | ComboBox "Starts" | 516.0 × 40.0 | radius-md | bg-300 | 1.0 control-border | Inter 400 15.0px ink | calendar 16.0px ink-muted | space-3 |
| Example | TextInput "Starts" | 160.0 × 40.0 | radius-md | bg-300 | 1.0 control-border | Inter 400 15.0px ink | — | space-3 |
| Example | Button "Pick a time" | 40.0 × 40.0 | radius-md | transparent | — | — | clock 16.0px ink | space-3 |
| Disabled | ComboBox "Starts" | 516.0 × 40.0 | radius-md | bg-300@45 | 1.0 control-border@45 | Inter 400 15.0px ink@45 | calendar 16.0px ink-muted@45 | space-3 |
| Disabled | TextInput "Starts" | 160.0 × 40.0 | radius-md | bg-300@45 | 1.0 control-border@45 | Inter 400 15.0px ink@45 | — | space-3 |
| Disabled | Button "Pick a time" | 40.0 × 40.0 | radius-md | transparent | — | — | clock 16.0px ink@45 | space-3 |
| Read only | ComboBox "Starts" | 516.0 × 40.0 | radius-md | bg-300@50 | 1.0 control-border | Inter 400 15.0px ink | calendar 16.0px ink-muted | space-3 |
| Read only | TextInput "Starts" | 160.0 × 40.0 | radius-md | bg-300@50 | 1.0 control-border | Inter 400 15.0px ink | — | space-3 |

## API

Builder methods: `disabled`, `label`, `new`, `range`, `read_only`, `step_minutes`.

## Example

```rust
fn date_time_picker() -> impl View {
    let month = Date::new(2026, 11, 1).unwrap_or_else(|| Date::from_days(0));
    let at = State::new(Time::new(14, 0).map(|time| DateTime { date: month, time }));
    DateTimePicker::new(at.binding(), month).label("Starts")
}
```

Every number above is in `DateTimePicker.json` beside this card.
