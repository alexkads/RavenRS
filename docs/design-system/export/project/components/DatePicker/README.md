# DatePicker

Field that opens a calendar in a popover.

**Family:** Forms · **Rust:** `ravenrs::prelude::DatePicker`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `label` | text | `Date` |  |
| `disabled` | on/off | `false` |  |
| `fill` | on/off | `false` |  |
| `placeholder` | text | `` | Empty leaves it as the component has it. |
| `read_only` | on/off | `false` |  |
| `width` | number 0–880 by 1 | `220` |  |

## Stories

- **Example**: 220 × 40 pt, settings `{"disabled":false,"fill":false,"label":"Date","placeholder":"","read_only":false,"width":220.0}`
- **Disabled**: 220 × 40 pt, settings `{"disabled":true,"fill":false,"label":"Date","placeholder":"","read_only":false,"width":220.0}`
- **Read only**: 220 × 40 pt, settings `{"disabled":false,"fill":false,"label":"Date","placeholder":"","read_only":true,"width":220.0}`

## Sizing

Hugs its content: 220 × 40 pt in a parent of 760 pt.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover** on "Date": border {color:control-border,width:1.0} → {color:mix(ink, control-border, 25),width:1.0}.
- **Focus** on "Date": adds a rect (border 2.0 focus-ring).
- **Pressed** on "Date": border {color:control-border,width:1.0} → {color:mix(ink, control-border, 25),width:1.0}.

## Behavior

- **Keys** its first control handles once focused: `Enter`, `Space`.
- **Accessibility**: role `ComboBox` "Date".
- **Focus order** (Tab): ComboBox "Date".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | ComboBox "Date" | 220.0 × 40.0 | radius-md | bg-300 | 1.0 control-border | Inter 400 15.0px ink-muted | calendar 16.0px ink-muted | space-3 |
| Disabled | ComboBox "Date" | 220.0 × 40.0 | radius-md | bg-300@45 | 1.0 control-border@45 | Inter 400 15.0px ink-muted@45 | calendar 16.0px ink-muted@45 | space-3 |
| Read only | ComboBox "Date" | 220.0 × 40.0 | radius-md | bg-300@50 | 1.0 control-border | Inter 400 15.0px ink-muted | calendar 16.0px ink-muted | space-3 |

## API

Builder methods: `disabled`, `fill`, `label`, `new`, `placeholder`, `read_only`, `today`, `width`.

## Example

```rust
fn date_picker() -> impl View {
    let today = Date::new(2026, 11, 1).unwrap_or_else(|| unreachable!("a real date"));
    let date = State::new(None);
    DatePicker::new(date.binding(), today).today(today).label("Date")
}
```

Every number above is in `DatePicker.json` beside this card.
