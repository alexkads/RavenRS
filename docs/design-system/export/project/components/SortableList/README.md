# SortableList

Rows put in order by a drag or with Alt and the arrows.

**Family:** Data · **Rust:** `ravenrs::prelude::SortableList`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `grip` | on/off | `true` |  |
| `label` | text | `` | Empty leaves it as the component has it. |
| `spacing` | number 0–8 by 0.1 | `2` |  |

## Stories

- **Example**: 728 × 126 pt, settings `{"grip":true,"label":"","spacing":2.0}`
- **Without grip**: 728 × 126 pt, settings `{"grip":false,"label":"","spacing":2.0}`

## Sizing

Fills the width of its parent: 288, 728, 1168 pt wide in parents of 320, 760, 1200 pt.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover** on "Exposure": fill transparent → ink@5.
- **Focus** on "Exposure": adds a rect (border 2.0 focus-ring).
- **Pressed** on "Exposure": fill transparent → ink@5.

## Behavior

- **Events**: `on_reorder`.
- **Keys** its first control handles once focused: `ArrowDown`, `ArrowUp`, `ArrowLeft`, `ArrowRight`, `End`.
- **Accessibility**: role `List` "Sortable list".
- **Focus order** (Tab): ListItem "Exposure" → ListItem "Curves" → ListItem "Grain" → ListItem "Vignette".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Status "" | 0.0 × 0.0 | — | — | — | — | — | — |

## API

Builder methods: `grip`, `key`, `label`, `new`, `on_reorder`, `row_name`, `spacing`.

## Example

```rust
fn sortable_list() -> impl View {
    let layers = State::new(vec!["Exposure", "Curves", "Grain", "Vignette"]);
    view_fn(move || {
        let shown = layers.get();
        let (names, moved) = (shown.clone(), layers.clone());
        SortableList::new(shown.len(), move |index| Text::new(shown[index]))
            .row_name(move |index| names[index].to_owned())
            .on_reorder(move |from, to| {
                moved.update(|list| {
                    let layer = list.remove(from);
                    list.insert(to, layer);
                });
            })
    })
}
```

Every number above is in `SortableList.json` beside this card.
