# TagBox

Several values from a list, shown as tags.

**Family:** Actions and inputs · **Rust:** `ravenrs::prelude::TagBox`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `allow_new` | on/off | `true` |  |
| `label` | text | `Tags` |  |
| `disabled` | on/off | `false` |  |
| `fill` | on/off | `false` |  |
| `placeholder` | text | `` | Empty leaves it as the component has it. |
| `read_only` | on/off | `false` |  |
| `width` | number 0–1120 by 1 | `280` |  |

## Stories

- **Example**: 280 × 40 pt, settings `{"allow_new":true,"disabled":false,"fill":false,"label":"Tags","placeholder":"","read_only":false,"width":280.0}`
- **Disabled**: 280 × 40 pt, settings `{"allow_new":true,"disabled":true,"fill":false,"label":"Tags","placeholder":"","read_only":false,"width":280.0}`
- **Read only**: 280 × 40 pt, settings `{"allow_new":true,"disabled":false,"fill":false,"label":"Tags","placeholder":"","read_only":true,"width":280.0}`

## Sizing

Hugs its content: 280 × 40 pt in a parent of 760 pt.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover** on "Tags": border {color:control-border,width:1.0} → {color:mix(ink, control-border, 25),width:1.0}.
- **Focus** on "Tags": adds a rect (border 2.0 focus-ring).
- **Pressed** on "Tags": border {color:control-border,width:1.0} → {color:mix(ink, control-border, 25),width:1.0}.

## Behavior

- **Keys** its first control handles once focused: `ArrowDown`, `ArrowUp`, `Enter`, `Space`, `Escape`.
- **Accessibility**: role `ComboBox` "Tags".
- **Focus order** (Tab): ComboBox "Tags" → Button "Remove Urgent".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | ComboBox "Tags" | 280.0 × 40.0 | radius-md | bg-300 | 1.0 control-border | Inter 400 13.0px ink | x 12.0px ink | 14.0 |
| Example | Button "Remove Urgent" | 22.0 × 22.0 | radius-md | transparent | — | — | x 12.0px ink | 5.0 |
| Disabled | ComboBox "Tags" | 280.0 × 40.0 | radius-md | bg-300@45 | 1.0 control-border@45 | Inter 400 13.0px ink@45 | chevron-down 16.0px ink-muted@45 | 14.0 |
| Read only | ComboBox "Tags" | 280.0 × 40.0 | radius-md | bg-300@50 | 1.0 control-border | Inter 400 13.0px ink | chevron-down 16.0px ink-muted | 14.0 |

## API

Builder methods: `allow_new`, `disabled`, `fill`, `label`, `most`, `new`, `placeholder`, `read_only`, `width`.

## Example

```rust
fn tag_box() -> impl View {
    let tags = State::new(vec![String::from("urgent")]);
    TagBox::new(
        tags.binding(),
        vec![
            LookupItem::new("urgent", "Urgent"),
            LookupItem::new("review", "To review"),
            LookupItem::new("paid", "Paid"),
        ],
    )
    .allow_new(true)
    .label("Tags")
}
```

Every number above is in `TagBox.json` beside this card.
