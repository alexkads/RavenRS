# FileBrowser

Folders and files from the app, with a breadcrumb, sorting and selection.

**Family:** Navigation · **Rust:** `ravenrs::prelude::FileBrowser`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `height` | number 0–1200 by 1 | `300` |  |
| `label` | text | `` | Empty leaves it as the component has it. |
| `mode` | None · Single · Multiple | `Multiple` |  |

## Stories

- **Example**: 728 × 300 pt, settings `{"height":300.0,"label":"","mode":"Multiple"}`
- **Mode: None**: 728 × 300 pt, settings `{"height":300.0,"label":"","mode":"None"}`
- **Mode: Single**: 728 × 300 pt, settings `{"height":300.0,"label":"","mode":"Single"}`

## Sizing

Fills the width of its parent: 288, 728, 1168 pt wide in parents of 320, 760, 1200 pt.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover** on "Files": fill transparent → ink@5.
- **Focus** on "Files": border null → {color:accent,width:1.5}; color ink-muted → accent; color ink-muted → accent; adds a rect (border 2.0 focus-ring).
- **Pressed** on "Files": fill transparent → ink@5.

## Behavior

- **Events**: `on_navigate`, `on_open`.
- **Keys** its first control handles once focused: `ArrowDown`, `ArrowUp`, `Space`, `Escape`, `Home`, `End`.
- **Accessibility**: role `ListBox` "Files".
- **Focus order** (Tab): ListBox "Files" → ColumnHeader "Name" → ColumnHeader "Size" → ColumnHeader "Modified".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | ListBox "Files" | 728.0 × 300.0 | — | — | — | Inter 400 13.0px ink-muted | house 14.0px ink-muted | space-3 |
| Example | Label "Backups" | 53.0 × 18.0 | — | — | — | Inter 400 13.0px ink-muted | — | 0.0 |
| Example | Label "Studio" | 41.0 × 18.0 | — | — | — | Inter 600 13.0px ink | — | 0.0 |
| Example | ListBoxOption "Portraits" | 728.0 × 32.0 | radius-sm | transparent | — | Inter 400 13.0px ink | folder 16.0px accent | space-3 |
| Example | ListBoxOption "catalog.db" | 728.0 × 32.0 | radius-sm | transparent | — | Inter 400 13.0px ink | file 16.0px ink-muted | space-3 |
| Example | ListBoxOption "cover.jpg" | 728.0 × 32.0 | radius-sm | transparent | — | Inter 400 13.0px ink | image 16.0px ink-muted | space-3 |
| Example | ListBoxOption "notes.txt" | 728.0 × 32.0 | radius-sm | transparent | — | Inter 400 13.0px ink | file-text 16.0px ink-muted | space-3 |

## API

Builder methods: `height`, `label`, `mode`, `new`, `on_navigate`, `on_open`, `selection`, `sort`.

## Example

```rust
fn file_browser() -> impl View {
    let entries = vec![
        FileEntry::folder("Weddings").modified("Oct 12, 2026", 12),
        FileEntry::folder("Portraits").modified("Sep 3, 2026", 3),
        FileEntry::file("catalog.db", 48_302_112).modified("Nov 2, 2026", 32),
        FileEntry::file("notes.txt", 2_204).modified("Oct 30, 2026", 30).icon(IconName::FileText),
        FileEntry::file("cover.jpg", 3_480_000).modified("Aug 21, 2026", 1).icon(IconName::Image),
    ];
    FileBrowser::new(["Backups", "2026", "Studio"], entries).height(300.0)
}
```

Every number above is in `FileBrowser.json` beside this card.
