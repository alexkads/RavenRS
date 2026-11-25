# Ribbon

Office-style command bar with tabs, groups and quick access.

**Family:** Navigation · **Rust:** `ravenrs::prelude::Ribbon`

## Stories

- **Example**: 728 × 162 pt, settings `{}`

## Sizing

Fills the width of its parent: 288, 728, 1168 pt wide in parents of 320, 760, 1200 pt.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover** on "File": fill bg-300 → mix(bg-300, ink, 92).
- **Focus** on "File": adds a rect (border 2.0 focus-ring).
- **Pressed** on "File": fill bg-300 → mix(bg-300, ink, 92).

## Behavior

- **Keys** its first control handles once focused: `Enter`, `Space`, `Escape`.
- **Accessibility**: role `Button` "File".
- **Focus order** (Tab): Button "File" → Button "Undo" → Tab "Home" → Tab "Insert" → Button "Collapse the ribbon" → Button "Paste" → Button "Copy" → Button "Print".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Button "File" | 47.0 × 32.0 | radius-md | bg-300 | 1.0 border | Inter 600 13.0px ink | — | space-3 |
| Example | Button "Undo" | 28.0 × 26.0 | radius-sm | transparent | — | — | undo-2 16.0px mix(ink, accent, 45) | 6.0 |
| Example | Tab "Home" | 62.0 × 34.0 | — | — | — | Inter 600 13.0px ink | — | space-3 |
| Example | Tab "Insert" | 61.0 × 34.0 | — | — | — | Inter 600 13.0px ink-muted | — | space-3 |
| Example | Button "Collapse the ribbon" | 32.0 × 32.0 | radius-md | transparent | — | — | chevron-up 14.0px ink | 9.0 |
| Example | Toolbar "Clipboard" | 137.0 × 114.0 | — | — | — | Inter 600 12.0px ink | clipboard-paste 28.0px mix(ink, accent, 45) | space-4 |
| Example | Button "Paste" | 52.0 × 86.0 | radius-sm | transparent | — | Inter 600 12.0px ink | clipboard-paste 28.0px mix(ink, accent, 45) | 10.0 |
| Example | Button "Copy" | 67.0 × 26.0 | radius-sm | transparent | — | Inter 400 13.0px ink | copy 16.0px mix(ink, accent, 45) | 6.0 |
| Example | Toolbar "Output" | 68.0 × 114.0 | — | — | — | Inter 600 12.0px ink | printer 28.0px mix(ink, accent, 45) | 18.0 |
| Example | Button "Print" | 52.0 × 86.0 | radius-sm | transparent | — | Inter 600 12.0px ink | printer 28.0px mix(ink, accent, 45) | space-3 |

## API

Builder methods: `app_menu`, `new`, `page`, `quick_access`, `search`, `selected_page`.

`RibbonSize`: `IconOnly`, `Large`, `Small`.

## Example

```rust
fn ribbon() -> impl View {
    Ribbon::new()
        .app_menu("File", vec![MenuItem::new("Save").icon(IconName::Save).shortcut("⌘S")])
        .quick_access(RibbonItem::button("Undo", IconName::Undo2))
        .page(
            RibbonPage::new("Home")
                .group(
                    RibbonGroup::new("Clipboard")
                        .item(RibbonItem::button("Paste", IconName::ClipboardPaste).large())
                        .item(RibbonItem::button("Copy", IconName::Copy).small()),
                )
                .group(
                    RibbonGroup::new("Output").item(RibbonItem::button("Print", IconName::Printer).large()),
                ),
        )
        .page(
            RibbonPage::new("Insert")
                .group(RibbonGroup::new("Items").item(RibbonItem::button("Image", IconName::Image).large())),
        )
}
```

Every number above is in `Ribbon.json` beside this card.
