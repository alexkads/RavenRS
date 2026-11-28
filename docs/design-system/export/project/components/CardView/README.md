# CardView

Records as cards, only those in sight are built.

**Family:** Data · **Rust:** `ravenrs::prelude::CardView`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `card_width` | number 0–720 by 1 | `180` |  |
| `card_height` | number 0–336 by 1 | `84` |  |
| `height` | number 0–1200 by 1 | `300` |  |
| `label` | text | `` | Empty leaves it as the component has it. |
| `selection_mode` | None · Single · Multiple | `Single` |  |

## Stories

- **Example**: 728 × 300 pt, settings `{"card_height":84.0,"card_width":180.0,"height":300.0,"label":"","selection_mode":"Single"}`
- **Selection mode: None**: 728 × 300 pt, settings `{"card_height":84.0,"card_width":180.0,"height":300.0,"label":"","selection_mode":"None"}`
- **Selection mode: Multiple**: 728 × 300 pt, settings `{"card_height":84.0,"card_width":180.0,"height":300.0,"label":"","selection_mode":"Multiple"}`

## Sizing

Fills the width of its parent: 288, 728, 1168 pt wide in parents of 320, 760, 1200 pt.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover** on "Cards": border {color:border,width:1.0} → {color:mix(ink, border, 25),width:1.0}.
- **Focus** on "Cards": border {color:border,width:1.0} → {color:accent,width:1.5}; adds a rect (border 2.0 focus-ring).
- **Pressed** on "Cards": border {color:border,width:1.0} → {color:accent,width:1.5}; border {color:border,width:1.0} → {color:mix(ink, border, 25),width:1.0}.

## Behavior

- **Events**: `on_open`.
- **Keys** its first control handles once focused: `ArrowDown`, `ArrowRight`, `Space`, `Escape`, `End`.
- **Accessibility**: role `List` "Cards".
- **Focus order** (Tab): List "Cards".

## API

Builder methods: `card_height`, `card_name`, `card_width`, `height`, `label`, `new`, `on_open`, `selection`, `selection_mode`.

## Example

```rust
fn card_view() -> impl View {
    let selected = State::new(std::collections::BTreeSet::from([1]));
    CardView::new(10_000, |i| -> AnyView {
        Box::new(
            VStack::new((
                Text::new(format!("Customer {i}")).font_weight(Weight::SEMIBOLD),
                Text::new(format!("R$ {},00", 100 + i * 7)).foreground(Semantic::InkMuted),
            ))
            .spacing(4.0),
        )
    })
    .card_name(|i| format!("Customer {i}"))
    .card_width(180.0)
    .card_height(84.0)
    .selection(selected.binding())
    .height(300.0)
}
```

Every number above is in `CardView.json` beside this card.
