# Tabs

Switches between views of the same place; each tab builds when it is selected.

**Family:** Navigation · **Rust:** `ravenrs::prelude::Tabs`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `tabs` | text | `Activity, Settings, Billing` | The tabs, separated by commas. |
| `selected` | number 0–5 by 1 | `0` |  |
| `variant` | Underline · Segmented | `Underline` |  |
| `size` | Small · Medium · Large | `Medium` |  |
| `disabled` | on/off | `false` | The open tab cannot change. |

## Stories

- **Underline**: 728 × 84 pt, settings `{"disabled":false,"selected":0.0,"size":"Medium","tabs":"Activity, Settings, Billing","variant":"Underline"}`
- **Segmented**: 728 × 82 pt, settings `{"disabled":false,"selected":1.0,"size":"Medium","tabs":"Activity, Settings, Billing","variant":"Segmented"}`
- **All sizes**: 221 × 146 pt, settings `{"disabled":false,"selected":0.0,"size":"Medium","tabs":"Activity, Settings, Billing","variant":"Underline"}`
- **Disabled**: 728 × 84 pt, settings `{"disabled":true,"selected":1.0,"size":"Medium","tabs":"Activity, Settings, Billing","variant":"Underline"}`
- **Tabs without content**: 728 × 42 pt, settings `{"disabled":false,"selected":0.0,"size":"Medium","tabs":"Activity, Settings, Billing","variant":"Underline"}`
- **Overflow**: 360 × 78 pt, settings `{"disabled":false,"selected":0.0,"size":"Small","tabs":"All, WhatsApp, Instagram, Messenger, Telegram, Site","variant":"Underline"}`

## Sizing

Fills the width of its parent: 288, 728, 1168 pt wide in parents of 320, 760, 1200 pt.

- **All sizes**: Hugs its content: 221 × 146 pt in a parent of 760 pt; in a parent that stretches its children, as a form column, it takes the whole width.
- **Overflow**: Hugs its content: 360 × 78 pt in a parent of 760 pt; in a parent of 320 pt it narrows to 350 pt.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Underline:

- **Hover** on "Settings": color ink-muted → ink.
- **Focus** on "Activity": adds a rect (border 2.0 focus-ring).
- **Pressed** on "Settings": color ink-muted → ink.

## Behavior

- **Keys** its first control handles once focused: `ArrowDown`, `ArrowUp`, `ArrowLeft`, `ArrowRight`, `End`.
- **Accessibility**: role `TabList`.
- **Focus order** (Tab): Tab "Activity" → Tab "Settings" → Tab "Billing".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Underline | Tab "Activity" | 60.0 × 42.0 | — | — | — | Inter 600 15.0px ink | — | 2.0 |
| Underline | Tab "Settings" | 64.0 × 42.0 | — | — | — | Inter 600 15.0px ink-muted | — | 2.0 |
| Underline | Label "The content of Activity." | 728.0 × 22.0 | — | — | — | Inter 400 15.0px ink-muted | — | 0.0 |
| Segmented | Tab "Activity" | 78.0 × 34.0 | — | — | — | Inter 400 15.0px ink-muted | — | space-3 |
| Segmented | Tab "Settings" | 84.0 × 34.0 | radius-sm | bg-200 | 1.0 border | Inter 600 15.0px ink | — | space-3 |
| All sizes | Tab "Activity" | 53.0 × 30.0 | — | — | — | Inter 600 13.0px ink | — | 2.0 |
| All sizes | Tab "Settings" | 56.0 × 30.0 | — | — | — | Inter 600 13.0px ink-muted | — | 2.0 |
| Disabled | Tab "Activity" | 60.0 × 42.0 | — | — | — | Inter 600 15.0px ink-muted@45 | — | 2.0 |
| Disabled | Tab "Settings" | 64.0 × 42.0 | — | — | — | Inter 600 15.0px ink@45 | — | 2.0 |
| Overflow | Button "More tabs" | 20.0 × 30.0 | — | — | — | — | chevron-down 16.0px ink-muted | 2.0 |
| Overflow | Tab "All" | 41.0 × 26.0 | radius-sm | bg-200 | 1.0 border | Inter 600 13.0px ink | — | space-3 |
| Overflow | Tab "WhatsApp" | 88.0 × 26.0 | — | — | — | Inter 400 13.0px ink-muted | — | space-3 |
| Overflow | Button "More tabs" | 26.0 × 26.0 | — | — | — | — | chevron-down 16.0px ink-muted | 5.0 |

## API

Builder methods: `disabled`, `item`, `new`, `size`, `tab`, `variant`.

`TabsVariant`: `Segmented`, `Underline`.

## Example

```rust
fn tabs() -> impl View {
    let tab = State::new(0);
    Tabs::new(tab.binding())
        .tab("Activity", Text::new("Ana merged #482").foreground(Semantic::InkMuted))
        .tab("Settings", Text::new("Each tab builds when selected.").foreground(Semantic::InkMuted))
        .tab("Billing", Stat::new("Next invoice", "R$ 1.240"))
        // `TabsVariant::Segmented` makes it a track, `.item` a tab without content.
        .variant(TabsVariant::Underline)
}
```

Every number above is in `Tabs.json` beside this card.
