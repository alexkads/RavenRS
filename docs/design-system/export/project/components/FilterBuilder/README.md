# FilterBuilder

Builds a filter out of lines, in the expression language.

**Family:** Actions and inputs · **Rust:** `ravenrs::prelude::FilterBuilder`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `filter` | text | `[Price] > 10 And Contains([Name], 'pro')` | How the example starts. |

## Stories

- **Example**: 576 × 136 pt, settings `{"filter":"[Price] > 10 And Contains([Name], 'pro')"}`

## Sizing

Hugs its content: 576 × 136 pt in a parent of 760 pt; in a parent of 320 pt it narrows to 220 pt; its height follows the width, as its content wraps.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover** on "How lines combine": border {color:control-border,width:1.0} → {color:mix(ink, control-border, 25),width:1.0}.
- **Focus** on "How lines combine": adds a rect (border 2.0 focus-ring).
- **Pressed** on "How lines combine": border {color:control-border,width:1.0} → {color:mix(ink, control-border, 25),width:1.0}.

## Behavior

- **Keys** its first control handles once focused: `ArrowDown`, `ArrowUp`, `Enter`, `Space`, `Escape`.
- **Accessibility**: role `ComboBox` "How lines combine".
- **Focus order** (Tab): ComboBox "How lines combine" → Button "Condition" → Button "Group" → ComboBox "Field" → ComboBox "Comparison" → TextInput "Value" → Button "Remove" → ComboBox "Field" → ComboBox "Comparison" → TextInput "Value" → Button "Remove".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | ComboBox "How lines combine" | 200.0 × 40.0 | radius-md | bg-300 | 1.0 control-border | Inter 400 15.0px ink | chevron-down 16.0px ink-muted | space-3 |
| Example | Button "Condition" | 107.0 × 32.0 | radius-md | transparent | — | Inter 600 13.0px ink | plus 14.0px ink | space-3 |
| Example | TextInput "Value" | 180.0 × 40.0 | radius-md | bg-300 | 1.0 control-border | Inter 400 15.0px ink | — | space-3 |
| Example | Button "Remove" | 32.0 × 32.0 | radius-md | transparent | — | — | x 14.0px ink | 9.0 |

## API

Builder methods: `new`.

## Example

```rust
fn filter_builder() -> impl View {
    let filter = State::new(String::from("[Price] > 10 And Contains([Name], 'pro')"));
    FilterBuilder::new(
        filter.binding(),
        [
            FilterField::new("Name", "Name", FilterFieldKind::Text),
            FilterField::new("Price", "Price", FilterFieldKind::Number),
            FilterField::new("Active", "Active", FilterFieldKind::Boolean),
        ],
    )
}
```

Every number above is in `FilterBuilder.json` beside this card.
