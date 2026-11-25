# ContextMenu

Menu opened at the pointer on right click.

**Family:** Overlays · **Rust:** `ravenrs::prelude::ContextMenu`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `separator` | on/off | `false` |  |

## Stories

- **Example**: 153 × 90 pt, settings `{"separator":false}`

## Sizing

Hugs its content: 153 × 90 pt in a parent of 760 pt; in a parent that stretches its children, as a form column, it takes the whole width.

## Behavior

- **Accessibility**: role `Label` "Right-click in this box".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Label "Right-click in this box" | 153.0 × 22.0 | — | — | — | Inter 400 15.0px ink-muted | — | 0.0 |

## API

Builder methods: `item`, `new`, `separator`.

## Example

```rust
fn context_menu() -> impl View {
    ContextMenu::new(
        Text::new("Right-click in this box")
            .foreground(Semantic::InkMuted)
            .centered()
            .height(90.0)
            .background(Semantic::Control)
            .corner_radius(12.0),
    )
    .item(MenuItem::new("Copy").icon(IconName::Copy).shortcut("⌘C"))
    .item(MenuItem::new("Sort by").item(MenuItem::new("Name")).item(MenuItem::new("Date")))
    .item(MenuItem::new("Delete").icon(IconName::Trash2).destructive())
}
```

Every number above is in `ContextMenu.json` beside this card.
