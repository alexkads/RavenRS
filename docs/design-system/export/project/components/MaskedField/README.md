# MaskedField

Text that follows a pattern, like a phone number.

**Family:** Actions and inputs · **Rust:** `ravenrs::prelude::MaskedField`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `label` | text | `Phone` |  |
| `disabled` | on/off | `false` |  |
| `keep_literals` | on/off | `false` |  |
| `placeholder` | text | `` | Empty leaves it as the component has it. |
| `read_only` | on/off | `false` |  |
| `phone` | text | `11987654321` | How the example starts. |

## Stories

- **Example**: 728 × 40 pt, settings `{"disabled":false,"keep_literals":false,"label":"Phone","phone":"11987654321","placeholder":"","read_only":false}`
- **Disabled**: 728 × 40 pt, settings `{"disabled":true,"keep_literals":false,"label":"Phone","phone":"11987654321","placeholder":"","read_only":false}`
- **Read only**: 728 × 40 pt, settings `{"disabled":false,"keep_literals":false,"label":"Phone","phone":"11987654321","placeholder":"","read_only":true}`

## Sizing

Fills the width of its parent: 288, 728, 1168 pt wide in parents of 320, 760, 1200 pt.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover** on "Phone": border {color:control-border,width:1.0} → {color:mix(ink, control-border, 25),width:1.0}.
- **Focus** on "Phone": border {color:control-border,width:1.0} → {color:accent,width:1.5}; adds a rect (accent); adds a rect (border 2.0 focus-ring).
- **Pressed** on "Phone": border {color:control-border,width:1.0} → {color:accent,width:1.5}; adds a rect (accent).

## Behavior

- **Keys** its first control handles once focused: `ArrowUp`, `ArrowLeft`, `Escape`, `Home`.
- **Accessibility**: role `TextInput` "Phone".
- **Focus order** (Tab): TextInput "Phone".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | TextInput "Phone" | 728.0 × 40.0 | radius-md | bg-300 | 1.0 control-border | Inter 400 15.0px ink | — | space-3 |
| Disabled | TextInput "Phone" | 728.0 × 40.0 | radius-md | bg-300@45 | 1.0 control-border@45 | Inter 400 15.0px ink@45 | — | space-3 |
| Read only | TextInput "Phone" | 728.0 × 40.0 | radius-md | bg-300@50 | 1.0 control-border | Inter 400 15.0px ink | — | space-3 |

## API

Builder methods: `complete`, `disabled`, `keep_literals`, `label`, `new`, `optional_rule`, `placeholder`, `read_only`, `rule`, `width`.

## Example

```rust
fn masked_field() -> impl View {
    let phone = State::new(String::from("11987654321"));
    MaskedField::new(phone.binding(), "(00) 00000-0000").label("Phone")
}
```

Every number above is in `MaskedField.json` beside this card.
