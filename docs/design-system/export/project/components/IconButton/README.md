# IconButton

Square, icon-only button for toolbars; its label is read aloud and shown as a tooltip.

**Family:** Actions and inputs · **Rust:** `ravenrs::prelude::IconButton`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `icon` | plus · check · trash-2 · pencil · search · settings · download | `pencil` |  |
| `label` | text | `Edit` | Read by screen readers and shown on hover. |
| `style` | Primary · Secondary · Ghost · Danger | `Ghost` |  |
| `size` | Mini · Small · Medium · Large | `Medium` |  |
| `disabled` | on/off | `false` |  |
| `selected` | on/off | `false` |  |

## Stories

- **Ghost**: 40 × 40 pt, settings `{"disabled":false,"icon":"pencil","label":"Edit","selected":false,"size":"Medium","style":"Ghost"}`
- **Primary**: 40 × 40 pt, settings `{"disabled":false,"icon":"plus","label":"New","selected":false,"size":"Medium","style":"Primary"}`
- **Selected**: 40 × 40 pt, settings `{"disabled":false,"icon":"pencil","label":"Edit","selected":true,"size":"Medium","style":"Ghost"}`
- **Disabled**: 40 × 40 pt, settings `{"disabled":true,"icon":"pencil","label":"Edit","selected":false,"size":"Medium","style":"Ghost"}`
- **All styles**: 184 × 40 pt, settings `{"disabled":false,"icon":"pencil","label":"Edit","selected":false,"size":"Medium","style":"Ghost"}`
- **All sizes**: 166 × 48 pt, settings `{"disabled":false,"icon":"pencil","label":"Edit","selected":false,"size":"Medium","style":"Ghost"}`

## Sizing

Hugs its content: 40 × 40 pt in a parent of 760 pt.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Ghost:

- **Hover** on "Edit": fill transparent → ink@7.
- **Focus** on "Edit": adds a rect (border 2.0 focus-ring).
- **Pressed** on "Edit": fill transparent → ink@12, rect [0.0,0.0,40.0,40.0] → [0.6,0.6,38.8,38.8].

## Behavior

- **Events**: `on_click`.
- **Accessibility**: role `Button` "Edit".
- **Focus order** (Tab): Button "Edit".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Ghost | Button "Edit" | 40.0 × 40.0 | radius-md | transparent | — | — | pencil 16.0px ink | space-3 |
| Primary | Button "New" | 40.0 × 40.0 | radius-md | accent | — | — | plus 16.0px on-accent | space-3 |
| Selected | Button "Edit" | 40.0 × 40.0 | radius-md | accent@18 | 1.0 accent@55 | — | pencil 16.0px mix(ink, accent, 30) | space-3 |
| Disabled | Button "Edit" | 40.0 × 40.0 | radius-md | transparent | — | — | pencil 16.0px ink@45 | space-3 |
| All styles | Button "Primary" | 40.0 × 40.0 | radius-md | accent | — | — | pencil 16.0px on-accent | space-3 |
| All styles | Button "Secondary" | 40.0 × 40.0 | radius-md | bg-300 | 1.0 border | — | pencil 16.0px ink | space-3 |
| All styles | Button "Danger" | 40.0 × 40.0 | radius-md | danger | — | — | pencil 16.0px on-accent | space-3 |
| All sizes | Button "Mini" | 22.0 × 22.0 | radius-md | transparent | — | — | pencil 12.0px ink | 5.0 |
| All sizes | Button "Small" | 32.0 × 32.0 | radius-md | transparent | — | — | pencil 14.0px ink | 9.0 |
| All sizes | Button "Large" | 48.0 × 48.0 | radius-md | transparent | — | — | pencil 16.0px ink | space-4 |

## API

Builder methods: `description`, `disabled`, `label`, `new`, `on_click`, `selected`, `size`, `style`.

## Example

```rust
fn icon_button() -> impl View {
    HStack::new((
        IconButton::new(IconName::Pencil).label("Edit"),
        IconButton::new(IconName::Copy).label("Duplicate"),
        IconButton::new(IconName::Settings).label("Settings").style(ButtonStyle::Secondary),
        IconButton::new(IconName::Plus).label("New").style(ButtonStyle::Primary),
        IconButton::new(IconName::Bold).label("Bold").selected(true),
    ))
    .spacing(8.0)
}
```

Every number above is in `IconButton.json` beside this card.
