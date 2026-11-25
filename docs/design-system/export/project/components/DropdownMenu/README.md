# DropdownMenu

Button that opens a list of actions, with submenus and check marks.

**Family:** Overlays · **Rust:** `ravenrs::prelude::DropdownMenu`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `label` | text | `Actions` | The words on the button; start with a verb. |
| `icon` | none · plus · check · trash-2 · pencil · search · settings · download | `none` | An icon before the label. |
| `separator` | on/off | `true` |  |
| `icon_only` | on/off | `false` |  |
| `size` | Small · Medium · Large · Mini | `Medium` |  |
| `style` | Primary · Secondary · Ghost · Danger | `Secondary` |  |
| `open` | on/off | `false` | The list of actions is open. |

## Stories

- **Example**: 116 × 40 pt, settings `{"icon":"none","icon_only":false,"label":"Actions","open":false,"separator":true,"size":"Medium","style":"Secondary"}`
- **Open**: 220 × 273 pt, settings `{"icon":"none","icon_only":false,"label":"Actions","open":true,"separator":true,"size":"Medium","style":"Secondary"}`
- **Size: Small**: 94 × 32 pt, settings `{"icon":"none","icon_only":false,"label":"Actions","open":false,"separator":true,"size":"Small","style":"Secondary"}`
- **Size: Large**: 128 × 48 pt, settings `{"icon":"none","icon_only":false,"label":"Actions","open":false,"separator":true,"size":"Large","style":"Secondary"}`
- **Size: Mini**: 79 × 22 pt, settings `{"icon":"none","icon_only":false,"label":"Actions","open":false,"separator":true,"size":"Mini","style":"Secondary"}`
- **Style: Primary**: 116 × 40 pt, settings `{"icon":"none","icon_only":false,"label":"Actions","open":false,"separator":true,"size":"Medium","style":"Primary"}`
- **Style: Ghost**: 116 × 40 pt, settings `{"icon":"none","icon_only":false,"label":"Actions","open":false,"separator":true,"size":"Medium","style":"Ghost"}`
- **Style: Danger**: 116 × 40 pt, settings `{"icon":"none","icon_only":false,"label":"Actions","open":false,"separator":true,"size":"Medium","style":"Danger"}`
- **Primary with icon**: 172 × 40 pt, settings `{"icon":"plus","icon_only":false,"label":"New project","open":false,"separator":true,"size":"Medium","style":"Primary"}`

## Sizing

Hugs its content: 116 × 40 pt in a parent of 760 pt.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover** on "Actions": fill bg-300 → mix(ink, bg-300, 6).
- **Focus** on "Actions": adds a rect (border 2.0 focus-ring).
- **Pressed** on "Actions": border {color:border,width:1.0} → {color:border,width:0.97}, fill bg-300 → mix(ink, bg-300, 12), rect [0.0,0.0,116.0,40.0] → [1.74,0.6,112.52,38.8].

## Behavior

- **Keys** its first control handles once focused: `Enter`, `Space`, `Escape`.
- **Accessibility**: role `Button` "Actions".
- **Focus order** (Tab): Button "Actions".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Button "Actions" | 116.0 × 40.0 | radius-md | bg-300 | 1.0 border | Inter 600 15.0px ink | chevron-down 16.0px ink | 18.0 |
| Open | Menu | 220.0 × 225.0 | radius-lg | bg-200 | 1.0 border | Inter 400 15.0px ink | pencil 16.0px ink-muted | space-4 |
| Open | MenuItem "Edit" | 208.0 × 34.0 | 7.0 | transparent | — | Inter 400 15.0px ink | pencil 16.0px ink-muted | 10.0 |
| Open | MenuItem "Share" | 208.0 × 34.0 | 7.0 | transparent | — | Inter 400 15.0px ink | share-2 16.0px ink-muted | 10.0 |
| Open | MenuItem "Export" | 208.0 × 34.0 | 7.0 | transparent | — | Inter 400 15.0px ink | chevron-right 16.0px ink-muted | 10.0 |
| Open | MenuItem "Show grid" | 208.0 × 34.0 | 7.0 | transparent | — | Inter 600 15.0px ink | check 16.0px mix(ink, accent, 30) | 10.0 |
| Open | MenuItem "Label" | 208.0 × 34.0 | 7.0 | transparent | — | Inter 600 12.0px #a1b1f8 | — | 19.0 |
| Open | MenuItem "Delete" | 208.0 × 34.0 | 7.0 | transparent | — | Inter 400 15.0px danger | trash-2 16.0px danger | 10.0 |
| Size: Small | Button "Actions" | 94.0 × 32.0 | radius-md | bg-300 | 1.0 border | Inter 600 13.0px ink | chevron-down 14.0px ink | space-3 |
| Size: Large | Button "Actions" | 128.0 × 48.0 | radius-md | bg-300 | 1.0 border | Inter 600 15.0px ink | chevron-down 16.0px ink | space-6 |
| Size: Mini | Button "Actions" | 79.0 × 22.0 | radius-md | bg-300 | 1.0 border | Inter 600 12.0px ink | chevron-down 12.0px ink | space-2 |
| Style: Primary | Button "Actions" | 116.0 × 40.0 | radius-md | accent | — | Inter 600 15.0px on-accent | chevron-down 16.0px on-accent | 18.0 |
| Style: Ghost | Button "Actions" | 116.0 × 40.0 | radius-md | transparent | — | Inter 600 15.0px ink | chevron-down 16.0px ink | 18.0 |
| Style: Danger | Button "Actions" | 116.0 × 40.0 | radius-md | danger | — | Inter 600 15.0px on-accent | chevron-down 16.0px on-accent | 18.0 |
| Primary with icon | Button "New project" | 172.0 × 40.0 | radius-md | accent | — | Inter 600 15.0px on-accent | plus 16.0px on-accent | 18.0 |

## API

Builder methods: `icon`, `icon_only`, `item`, `new`, `open`, `separator`, `size`, `style`.

## Example

```rust
fn dropdown_menu() -> impl View {
    DropdownMenu::new("Actions")
        .item(MenuItem::new("Edit").icon(IconName::Pencil).shortcut("⌘E"))
        .item(MenuItem::new("Share").icon(IconName::Share2))
        .item(MenuItem::new("Export").item(MenuItem::new("As PDF")).item(MenuItem::new("As CSV")))
        .item(MenuItem::new("Show grid").checked(true))
        .item(MenuItem::new("Label").element(Badge::new("Family").tone(Tone::Accent)))
        .separator()
        .item(MenuItem::new("Delete").icon(IconName::Trash2).destructive())
}
```

Every number above is in `DropdownMenu.json` beside this card.
