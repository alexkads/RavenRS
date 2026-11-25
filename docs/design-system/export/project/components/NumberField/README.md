# NumberField

A typed number or amount of money, formatted when you leave.

**Family:** Actions and inputs · **Rust:** `ravenrs::prelude::NumberField`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `spin_buttons` | on/off | `true` |  |
| `label` | text | `Seats` |  |
| `decimals` | number 0–10 by 1 | `0` |  |
| `disabled` | on/off | `false` |  |
| `grouping` | on/off | `true` |  |
| `placeholder` | text | `` | Empty leaves it as the component has it. |
| `prefix` | text | `` | Empty leaves it as the component has it. |
| `read_only` | on/off | `false` |  |
| `step` | number 0–4 by 0.1 | `1` |  |
| `suffix` | text | `` | Empty leaves it as the component has it. |

## Stories

- **Example**: 232 × 40 pt, settings `{"decimals":0.0,"disabled":false,"grouping":true,"label":"Seats","placeholder":"","prefix":"","read_only":false,"spin_buttons":true,"step":1.0,"suffix":""}`
- **Disabled**: 232 × 40 pt, settings `{"decimals":0.0,"disabled":true,"grouping":true,"label":"Seats","placeholder":"","prefix":"","read_only":false,"spin_buttons":true,"step":1.0,"suffix":""}`
- **Read only**: 232 × 40 pt, settings `{"decimals":0.0,"disabled":false,"grouping":true,"label":"Seats","placeholder":"","prefix":"","read_only":true,"spin_buttons":true,"step":1.0,"suffix":""}`
- **Field only**: 232 × 40 pt, settings `{"decimals":0.0,"disabled":false,"grouping":true,"label":"","placeholder":"","prefix":"","read_only":false,"spin_buttons":true,"step":1.0,"suffix":""}`
- **Money**: 728 × 40 pt, settings `{"decimals":0.0,"disabled":false,"grouping":true,"label":"Seats","placeholder":"","prefix":"","read_only":false,"spin_buttons":true,"step":1.0,"suffix":""}`

## Sizing

Hugs its content: 232 × 40 pt in a parent of 760 pt.

- **Money**: Fills the width of its parent: 288, 728, 1168 pt wide in parents of 320, 760, 1200 pt.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover** on "Seats": border {color:control-border,width:1.0} → {color:mix(ink, control-border, 25),width:1.0}.
- **Focus** on "Seats": border {color:control-border,width:1.0} → {color:accent,width:1.5}; adds a rect (accent); adds a rect (border 2.0 focus-ring).
- **Pressed** on "Seats": border {color:control-border,width:1.0} → {color:accent,width:1.5}; adds a rect (accent).

## Behavior

- **Events**: `on_leave`.
- **Keys** its first control handles once focused: `ArrowDown`, `ArrowUp`, `ArrowLeft`, `Space`, `Escape`, `Home`.
- **Accessibility**: role `TextInput` "Seats".
- **Focus order** (Tab): TextInput "Seats" → Button "Decrease" → Button "Increase".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | TextInput "Seats" | 160.0 × 40.0 | radius-md | bg-300 | 1.0 control-border | Inter 400 15.0px ink | — | space-3 |
| Example | Button "Decrease" | 32.0 × 32.0 | radius-md | transparent | — | — | minus 14.0px ink | 9.0 |
| Example | Button "Increase" | 32.0 × 32.0 | radius-md | transparent | — | — | plus 14.0px ink | 9.0 |
| Disabled | TextInput "Seats" | 160.0 × 40.0 | radius-md | bg-300@45 | 1.0 control-border@45 | Inter 400 15.0px ink@45 | — | space-3 |
| Disabled | Button "Decrease" | 32.0 × 32.0 | radius-md | transparent | — | — | minus 14.0px ink@45 | 9.0 |
| Disabled | Button "Increase" | 32.0 × 32.0 | radius-md | transparent | — | — | plus 14.0px ink@45 | 9.0 |
| Read only | TextInput "Seats" | 160.0 × 40.0 | radius-md | bg-300@50 | 1.0 control-border | Inter 400 15.0px ink | — | space-3 |

## API

Builder methods: `decimals`, `disabled`, `grouping`, `label`, `money`, `new`, `on_leave`, `placeholder`, `prefix`, `range`, `read_only`, `spin_buttons`, `step`, `suffix`, `width`.

## Example

```rust
fn number_field() -> impl View {
    let (seats, price) = (State::new(Some(12.0)), State::new(Some(129_990_i64)));
    VStack::new((
        NumberField::new(seats.binding()).range(0.0, 100.0).spin_buttons().label("Seats"),
        NumberField::money(price.binding(), "R$ ").label("Price"),
    ))
    .spacing(12.0)
    .align(Align::Stretch)
}
```

Every number above is in `NumberField.json` beside this card.
