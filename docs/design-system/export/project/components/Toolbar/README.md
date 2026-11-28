# Toolbar

Commands that fold into a menu when out of room.

**Family:** Actions and inputs · **Rust:** `ravenrs::prelude::Toolbar`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `label` | text | `Records` |  |
| `separator` | on/off | `true` |  |
| `aligned_end` | on/off | `false` |  |

## Stories

- **Example**: 728 × 36 pt, settings `{"aligned_end":false,"label":"Records","separator":true}`

## Sizing

Fills the width of its parent: 288, 728, 1168 pt wide in parents of 320, 760, 1200 pt.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover** on "New": fill accent → accent-hover.
- **Focus** on "New": adds a rect (border 2.0 focus-ring).
- **Pressed** on "New": fill accent → accent-pressed, rect [0.0,2.0,75.0,32.0] → [1.13,2.48,72.75,31.04].

## Behavior

- **Accessibility**: role `Group` "Records".
- **Focus order** (Tab): Button "New" → Button "Refresh" → Button "Export".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Button "New" | 75.0 × 32.0 | radius-md | accent | — | Inter 600 13.0px on-accent | plus 14.0px on-accent | space-3 |
| Example | Button "Refresh" | 95.0 × 32.0 | radius-md | transparent | — | Inter 600 13.0px ink | refresh-cw 14.0px ink | space-3 |
| Example | Button "Export" | 88.0 × 32.0 | radius-md | transparent | — | Inter 600 13.0px ink | download 14.0px ink | space-3 |
| Example | Button "Delete" | 87.0 × 32.0 | radius-md | danger@45 | — | Inter 600 13.0px on-accent@45 | trash-2 14.0px on-accent@45 | space-3 |

## API

Builder methods: `action`, `label`, `new`, `separator`, `view`.

## Example

```rust
fn toolbar() -> impl View {
    Toolbar::new()
        .label("Records")
        .action(
            ToolbarAction::new("New")
                .icon(IconName::Plus)
                .style(ButtonStyle::Primary)
                .hotkey(Hotkey::command("n"))
                .on_click(|| {}),
        )
        .action(ToolbarAction::new("Refresh").icon(IconName::RefreshCw).on_click(|| {}))
        .separator()
        .action(ToolbarAction::new("Export").icon(IconName::Download).on_click(|| {}))
        .action(ToolbarAction::new("Delete").icon(IconName::Trash2).style(ButtonStyle::Danger).disabled(true))
}
```

Every number above is in `Toolbar.json` beside this card.
