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
| `trigger` | on/off | `false` | Opens from a view, here who is signed in, instead of the button. |
| `header` | on/off | `false` | Who is signed in at the top of the menu. |
| `sections` | on/off | `false` | A titled section, Theme, after the actions. |

## Stories

- **Example**: 116 × 40 pt, settings `{"header":false,"icon":"none","icon_only":false,"label":"Actions","open":false,"sections":false,"separator":true,"size":"Medium","style":"Secondary","trigger":false}`
- **Open**: 220 × 273 pt, settings `{"header":false,"icon":"none","icon_only":false,"label":"Actions","open":true,"sections":false,"separator":true,"size":"Medium","style":"Secondary","trigger":false}`
- **Size: Small**: 94 × 32 pt, settings `{"header":false,"icon":"none","icon_only":false,"label":"Actions","open":false,"sections":false,"separator":true,"size":"Small","style":"Secondary","trigger":false}`
- **Size: Large**: 128 × 48 pt, settings `{"header":false,"icon":"none","icon_only":false,"label":"Actions","open":false,"sections":false,"separator":true,"size":"Large","style":"Secondary","trigger":false}`
- **Size: Mini**: 79 × 22 pt, settings `{"header":false,"icon":"none","icon_only":false,"label":"Actions","open":false,"sections":false,"separator":true,"size":"Mini","style":"Secondary","trigger":false}`
- **Style: Primary**: 116 × 40 pt, settings `{"header":false,"icon":"none","icon_only":false,"label":"Actions","open":false,"sections":false,"separator":true,"size":"Medium","style":"Primary","trigger":false}`
- **Style: Ghost**: 116 × 40 pt, settings `{"header":false,"icon":"none","icon_only":false,"label":"Actions","open":false,"sections":false,"separator":true,"size":"Medium","style":"Ghost","trigger":false}`
- **Style: Danger**: 116 × 40 pt, settings `{"header":false,"icon":"none","icon_only":false,"label":"Actions","open":false,"sections":false,"separator":true,"size":"Medium","style":"Danger","trigger":false}`
- **Primary with icon**: 172 × 40 pt, settings `{"header":false,"icon":"plus","icon_only":false,"label":"New project","open":false,"sections":false,"separator":true,"size":"Medium","style":"Primary","trigger":false}`
- **Custom trigger**: 411 × 225 pt, settings `{"header":false,"icon":"none","icon_only":false,"label":"Account","open":true,"sections":false,"separator":true,"size":"Medium","style":"Secondary","trigger":true}`
- **With a header**: 220 × 344 pt, settings `{"header":true,"icon":"none","icon_only":false,"label":"Actions","open":true,"sections":false,"separator":true,"size":"Medium","style":"Secondary","trigger":false}`
- **With sections**: 220 × 378 pt, settings `{"header":false,"icon":"none","icon_only":false,"label":"Actions","open":true,"sections":true,"separator":true,"size":"Medium","style":"Secondary","trigger":false}`

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
| Custom trigger | Button "Account" | 183.0 × 46.0 | — | — | — | Inter 600 14.0px mix(ink, focus-ring, 50) | — | 9.0 |
| Custom trigger | Image "Ana Lima" | 36.0 × 36.0 | radius-md | focus-ring@28 | 1.0 focus-ring@50 | Inter 600 14.0px mix(ink, focus-ring, 50) | — | 9.0 |
| With a header | Menu | 220.0 × 296.0 | radius-lg | bg-200 | 1.0 border | Inter 600 14.0px mix(ink, focus-ring, 50) | pencil 16.0px ink-muted | space-4 |
| With a header | Label "Ana Lima" | 66.0 × 22.0 | — | — | — | Inter 400 15.0px ink | — | 0.0 |
| With a header | Label "ana@example.com" | 135.0 × 22.0 | — | — | — | Inter 400 15.0px ink-muted | — | 0.0 |
| With sections | Menu | 220.0 × 330.0 | radius-lg | bg-200 | 1.0 border | Inter 400 15.0px ink | pencil 16.0px ink-muted | space-4 |
| With sections | MenuItem "Light" | 208.0 × 34.0 | 7.0 | transparent | — | Inter 400 15.0px ink | sun 16.0px ink-muted | 10.0 |
| With sections | MenuItem "Dark" | 208.0 × 34.0 | 7.0 | transparent | — | Inter 600 15.0px ink | moon 16.0px ink-muted | 10.0 |

## API

Builder methods: `header`, `icon`, `icon_only`, `item`, `label`, `new`, `open`, `placement`, `separator`, `size`, `style`, `trigger`.

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
