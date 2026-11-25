# VirtualList

A list of any length with rows of any height, which jumps to a row and stays at the end.

**Family:** Data · **Rust:** `ravenrs::prelude::VirtualList`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `height` | number 0–1040 by 1 | `260` |  |
| `bare` | on/off | `false` |  |
| `follow_end` | on/off | `false` |  |
| `horizontal` | on/off | `false` |  |
| `label` | text | `` | Empty leaves it as the component has it. |
| `padding` | number 0–48 by 1 | `0` | Room around the rows, in points. |
| `spacing` | number 0–48 by 1 | `0` | Room between rows, in points. |

## Stories

- **Example**: 728 × 260 pt, settings `{"bare":false,"follow_end":false,"height":260.0,"horizontal":false,"label":"","padding":0.0,"spacing":0.0}`
- **Spaced**: 728 × 260 pt, settings `{"bare":false,"follow_end":false,"height":260.0,"horizontal":false,"label":"","padding":8.0,"spacing":4.0}`

## Sizing

Fills the width of its parent: 288, 728, 1168 pt wide in parents of 320, 760, 1200 pt.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover**: "List" does not change.
- **Focus** on "List": adds a rect (border 1.5 accent); adds a rect (border 2.0 focus-ring).
- **Pressed**: "List" does not change.

## Behavior

- **Events**: `on_activate`, `on_visible`.
- **Keys** its first control handles once focused: `ArrowDown`, `ArrowUp`, `Enter`, `Space`, `Escape`, `Home`, `End`.
- **Accessibility**: role `List` "List".
- **Focus order** (Tab): List "List".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | ListItem "Row 0 A row that says how tall it is" | 728.0 × 76.0 | — | — | — | Inter 400 15.0px ink | — | space-2 |
| Example | ListItem "Row 1 A row that says how tall it is A row that says how tall it is" | 728.0 × 98.0 | — | — | — | Inter 400 15.0px ink | — | space-2 |
| Example | ListItem "Row 2 A row that says how tall it is A row that says how tall it is A row that says how tall it is" | 728.0 × 120.0 | — | — | — | Inter 400 15.0px ink | — | space-2 |
| Example | ListItem "Row 4 A row that says how tall it is A row that says how tall it is" | 728.0 × 98.0 | — | — | — | — | — | — |
| Example | ListItem "Row 5 A row that says how tall it is A row that says how tall it is A row that says how tall it is" | 728.0 × 120.0 | — | — | — | — | — | — |
| Spaced | ListItem "Row 0 A row that says how tall it is" | 728.0 × 80.0 | — | — | — | Inter 400 15.0px ink | — | space-2 |
| Spaced | ListItem "Row 1 A row that says how tall it is A row that says how tall it is" | 728.0 × 102.0 | — | — | — | Inter 400 15.0px ink | — | space-2 |
| Spaced | ListItem "Row 2 A row that says how tall it is A row that says how tall it is A row that says how tall it is" | 728.0 × 124.0 | — | — | — | Inter 400 15.0px ink | — | space-2 |
| Spaced | ListItem "Row 4 A row that says how tall it is A row that says how tall it is" | 728.0 × 102.0 | — | — | — | — | — | — |
| Spaced | ListItem "Row 5 A row that says how tall it is A row that says how tall it is A row that says how tall it is" | 728.0 × 124.0 | — | — | — | — | — | — |

## API

Builder methods: `estimated_height`, `follow_end`, `height`, `horizontal`, `key`, `label`, `new`, `on_activate`, `on_visible`, `row_height`, `row_heights`, `row_name`, `scroll_to`, `selection`, `spacing`.

## Example

```rust
fn virtual_list() -> impl View {
    // Ten thousand rows of different heights: only what shows exists.
    let selected = State::new(None::<usize>);
    VirtualList::new(10_000, |index| {
        let tall = 1 + index % 3;
        let words = vec!["A row that says how tall it is"; tall].join("\n");
        VStack::new((Text::new(format!("Row {index}")), Text::new(words))).padding(8.0)
    })
    .key(|index| index as u64)
    .selection(selected.binding())
    .height(260.0)
}
```

Every number above is in `VirtualList.json` beside this card.
