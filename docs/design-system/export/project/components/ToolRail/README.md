# ToolRail

A narrow bar of tools with their names under their icons.

**Family:** Forms · **Rust:** `ravenrs::prelude::ToolRail`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `separator` | on/off | `false` |  |
| `open` | number 0–5 by 1 | `0` | How the example starts. |

## Stories

- **Example**: 72 × 237 pt, settings `{"open":0.0,"separator":false}`

## Sizing

Hugs its content: 72 × 237 pt in a parent of 760 pt.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover** on "Zoom": fill transparent → ink@5.
- **Focus** on "Text": adds a rect (border 2.0 focus-ring); color ink → ink-muted; color ink → ink-muted; color ink → ink-muted.
- **Pressed** on "Zoom": fill transparent → ink@5.

## Behavior

- **Events**: `on_drag_tool`.
- **Keys** its first control handles once focused: `Escape`.
- **Accessibility**: role `Button` "Text".
- **Focus order** (Tab): Button "Text" → Button "Zoom" → Button "Fades" → Button "Shape".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Button "Text" | 68.0 × 56.0 | radius-md | accent@20 | — | Inter 600 12.0px ink | type 18.0px ink | 21.0 |
| Example | Button "Zoom" | 68.0 × 56.0 | radius-md | transparent | — | Inter 600 12.0px ink-muted | zoom-in 18.0px ink-muted | 17.0 |
| Example | Button "Fades" | 68.0 × 56.0 | radius-md | transparent | — | Inter 600 12.0px ink-muted | layers 18.0px ink-muted | space-4 |
| Example | Button "Shape" | 68.0 × 56.0 | radius-md | transparent | — | Inter 600 12.0px ink-muted | square 18.0px ink-muted | 15.0 |

## API

Builder methods: `columns`, `icons_only`, `new`, `on_drag_tool`, `separator`, `tool`, `width`.

## Example

```rust
fn tool_rail() -> impl View {
    let open = State::new(0);
    ToolRail::new(
        open.binding(),
        [(IconName::Type, "Text"), (IconName::ZoomIn, "Zoom"), (IconName::Layers, "Fades")],
    )
    // Hold the button, right-click or press Right for the tools that share its place.
    .separator()
    .tool(ToolRailItem::new(IconName::Square, "Shape").variant(IconName::Circle, "Ellipse"))
}
```

Every number above is in `ToolRail.json` beside this card.
