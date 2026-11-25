# Menubar

Horizontal bar of menus such as File, Edit and View.

**Family:** Navigation · **Rust:** `ravenrs::prelude::Menubar`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `menu` | text | `File` |  |
| `separator` | on/off | `true` |  |

## Stories

- **Example**: 97 × 38 pt, settings `{"menu":"File","separator":true}`

## Sizing

Hugs its content: 97 × 38 pt in a parent of 760 pt.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover** on "Edit": fill transparent → ink@6.
- **Focus** on "Edit": adds a rect (border 2.0 focus-ring).
- **Pressed** on "Edit": fill transparent → ink@6.

## Behavior

- **Keys** its first control handles once focused: `ArrowDown`, `ArrowLeft`, `ArrowRight`, `Enter`, `Space`, `End`, `Escape`.
- **Accessibility**: role `MenuBar`.
- **Focus order** (Tab): MenuItem "Edit" → MenuItem "File".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | MenuItem "Edit" | 44.0 × 30.0 | radius-sm | transparent | — | Inter 600 13.0px ink | — | 10.0 |

## API

Builder methods: `item`, `menu`, `new`, `separator`.

## Example

```rust
fn menubar() -> impl View {
    Menubar::new()
        .menu("File")
        .item(MenuItem::new("New").icon(IconName::FilePlus).shortcut("⌘N"))
        .separator()
        .item(MenuItem::new("Print").icon(IconName::Printer).shortcut("⌘P"))
        .menu("Edit")
        .item(MenuItem::new("Undo").icon(IconName::Undo2).shortcut("⌘Z"))
}
```

Every number above is in `Menubar.json` beside this card.
