# SelectionBar

How many are chosen, a way out and what to do with them, with Escape to leave.

**Family:** Forms · **Rust:** `ravenrs::prelude::SelectionBar`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `hint` | text | `` | Empty leaves it as the component has it. |

## Stories

- **Example**: 420 × 48 pt, settings `{"hint":""}`

## Sizing

Hugs its content: 420 × 48 pt in a parent of 760 pt; in a parent of 320 pt it narrows to 412 pt.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover** on "Leave selection": fill transparent → ink@7.
- **Focus** on "Leave selection": adds a rect (border 2.0 focus-ring).
- **Pressed** on "Leave selection": fill transparent → ink@12, rect [8.0,4.0,40.0,40.0] → [8.6,4.6,38.8,38.8].

## Behavior

- **Events**: `on_exit`.
- **Accessibility**: role `Group` "Selection actions".
- **Focus order** (Tab): Button "Leave selection" → Button "Copy" → Button "Favorite".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Button "Leave selection" | 40.0 × 40.0 | radius-md | transparent | — | — | x 16.0px ink | space-3 |
| Example | Status "3 selected" | 64.0 × 18.0 | — | — | — | Inter 400 13.0px ink | — | 0.0 |
| Example | Button "Copy" | 32.0 × 32.0 | radius-md | transparent | — | — | copy 14.0px ink | 9.0 |
| Example | Button "Favorite" | 32.0 × 32.0 | radius-md | transparent | — | — | star 14.0px ink | 9.0 |

## API

Builder methods: `action`, `hint`, `new`, `on_exit`.

## Example

```rust
fn selection_bar() -> impl View {
    SelectionBar::new(3)
        .on_exit(|| {})
        .action(ToolbarAction::new("Copy").icon(IconName::Copy).icon_only().on_click(|| {}))
        .action(ToolbarAction::new("Favorite").icon(IconName::Star).icon_only().on_click(|| {}))
        .width(420.0)
}
```

Every number above is in `SelectionBar.json` beside this card.
