# DateRangePicker

A period in two clicks, or ready-made.

**Family:** Actions and inputs · **Rust:** `ravenrs::prelude::DateRangePicker`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `label` | text | `Period` |  |
| `disabled` | on/off | `false` |  |
| `placeholder` | text | `` | Empty leaves it as the component has it. |
| `read_only` | on/off | `false` |  |
| `width` | number 0–1360 by 1 | `340` |  |

## Stories

- **Example**: 340 × 40 pt, settings `{"disabled":false,"label":"Period","placeholder":"","read_only":false,"width":340.0}`
- **Disabled**: 340 × 40 pt, settings `{"disabled":true,"label":"Period","placeholder":"","read_only":false,"width":340.0}`
- **Read only**: 340 × 40 pt, settings `{"disabled":false,"label":"Period","placeholder":"","read_only":true,"width":340.0}`

## Sizing

Hugs its content: 340 × 40 pt in a parent of 760 pt; in a parent of 320 pt it narrows to 328 pt.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover** on "Period": border {color:control-border,width:1.0} → {color:mix(ink, control-border, 25),width:1.0}.
- **Focus** on "Period": adds a rect (border 2.0 focus-ring).
- **Pressed** on "Period": border {color:control-border,width:1.0} → {color:mix(ink, control-border, 25),width:1.0}.

## Behavior

- **Keys** its first control handles once focused: `ArrowDown`, `ArrowUp`, `Enter`, `Space`.
- **Accessibility**: role `ComboBox` "Period".
- **Focus order** (Tab): ComboBox "Period" → Button "Clear".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | ComboBox "Period" | 340.0 × 40.0 | radius-md | bg-300 | 1.0 control-border | Inter 400 15.0px ink | calendar 16.0px ink-muted | space-3 |
| Example | Button "Clear" | 32.0 × 32.0 | radius-md | transparent | — | — | x 14.0px ink | 9.0 |
| Disabled | ComboBox "Period" | 340.0 × 40.0 | radius-md | bg-300@45 | 1.0 control-border@45 | Inter 400 15.0px ink@45 | calendar 16.0px ink-muted@45 | space-3 |
| Read only | ComboBox "Period" | 340.0 × 40.0 | radius-md | bg-300@50 | 1.0 control-border | Inter 400 15.0px ink | calendar 16.0px ink-muted | space-3 |

## API

Builder methods: `blocked`, `disabled`, `label`, `new`, `placeholder`, `preset`, `range`, `read_only`, `today`, `width`.

## Example

```rust
fn date_range_picker() -> impl View {
    let today = Date::new(2026, 11, 2).unwrap_or_else(|| unreachable!("a valid date"));
    let period = State::new(Some(DateRange::new(today, today.add_days(6))));
    DateRangePicker::new(period.binding(), today)
        .today(today)
        .preset("Last 7 days", DateRange::new(today.add_days(-6), today))
        .preset("Last 30 days", DateRange::new(today.add_days(-29), today))
        .label("Period")
}
```

Every number above is in `DateRangePicker.json` beside this card.
