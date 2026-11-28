# Button

Clickable, focusable button with styles, sizes, icons, loading and selected states.

**Family:** Actions and inputs · **Rust:** `ravenrs::prelude::Button`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `label` | text | `Save` | The words on it; start with a verb. |
| `style` | Primary · Secondary · Ghost · Danger | `Primary` | Primary for the one action of a window. |
| `size` | Mini · Small · Medium · Large | `Medium` |  |
| `icon` | none · plus · check · trash-2 · pencil · search · settings · download | `none` | An icon before the label. |
| `disabled` | on/off | `false` |  |
| `loading` | on/off | `false` | A spinner in place of the icon while it works. |
| `selected` | on/off | `false` | Pressed, for a button that shows a state. |
| `full_width` | on/off | `false` | Fills the width of its column. |
| `shortcut` | text | `` | The key shown at the end, as F8. |

## Stories

- **Primary**: 72 × 40 pt, settings `{"disabled":false,"full_width":false,"icon":"none","label":"Save","loading":false,"selected":false,"shortcut":"","size":"Medium","style":"Primary"}`
- **Secondary**: 87 × 40 pt, settings `{"disabled":false,"full_width":false,"icon":"none","label":"Cancel","loading":false,"selected":false,"shortcut":"","size":"Medium","style":"Secondary"}`
- **Ghost**: 118 × 40 pt, settings `{"disabled":false,"full_width":false,"icon":"none","label":"Show more","loading":false,"selected":false,"shortcut":"","size":"Medium","style":"Ghost"}`
- **Danger with icon**: 107 × 40 pt, settings `{"disabled":false,"full_width":false,"icon":"trash-2","label":"Delete","loading":false,"selected":false,"shortcut":"","size":"Medium","style":"Danger"}`
- **Loading**: 88 × 40 pt, settings `{"disabled":false,"full_width":false,"icon":"none","label":"Upload","loading":true,"selected":false,"shortcut":"","size":"Medium","style":"Secondary"}`
- **Disabled**: 72 × 40 pt, settings `{"disabled":true,"full_width":false,"icon":"none","label":"Save","loading":false,"selected":false,"shortcut":"","size":"Medium","style":"Primary"}`
- **With shortcut**: 111 × 40 pt, settings `{"disabled":false,"full_width":false,"icon":"none","label":"Open","loading":false,"selected":false,"shortcut":"F8","size":"Medium","style":"Ghost"}`
- **All styles**: 401 × 40 pt, settings `{"disabled":false,"full_width":false,"icon":"none","label":"Save","loading":false,"selected":false,"shortcut":"","size":"Medium","style":"Primary"}`
- **All sizes**: 310 × 48 pt, settings `{"disabled":false,"full_width":false,"icon":"none","label":"Save","loading":false,"selected":false,"shortcut":"","size":"Medium","style":"Primary"}`

## Sizing

Hugs its content: 72 × 40 pt in a parent of 760 pt.

- **All styles**: Hugs its content: 401 × 40 pt in a parent of 760 pt; in a parent of 320 pt it narrows to 216 pt; its height follows the width, as its content wraps.
- **All sizes**: Hugs its content: 310 × 48 pt in a parent of 760 pt; in a parent of 320 pt it narrows to 213 pt; its height follows the width, as its content wraps.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Primary:

- **Hover** on "Save": fill accent → accent-hover.
- **Focus** on "Save": adds a rect (border 2.0 focus-ring).
- **Pressed** on "Save": fill accent → accent-pressed, rect [0.0,0.0,72.0,40.0] → [1.08,0.6,69.84,38.8].

## Behavior

- **Events**: `on_click`.
- **Accessibility**: role `Button` "Save".
- **Focus order** (Tab): Button "Save".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Primary | Button "Save" | 72.0 × 40.0 | radius-md | accent | — | Inter 600 15.0px on-accent | — | 18.0 |
| Secondary | Button "Cancel" | 87.0 × 40.0 | radius-md | bg-300 | 1.0 border | Inter 600 15.0px ink | — | 18.0 |
| Ghost | Button "Show more" | 118.0 × 40.0 | radius-md | transparent | — | Inter 600 15.0px ink | — | 18.0 |
| Danger with icon | Button "Delete" | 107.0 × 40.0 | radius-md | danger | — | Inter 600 15.0px on-accent | trash-2 16.0px on-accent | 18.0 |
| Loading | Button "Upload" | 88.0 × 40.0 | radius-md | bg-300 | 1.0 border | Inter 600 15.0px transparent | — | 18.0 |
| Disabled | Button "Save" | 72.0 × 40.0 | radius-md | accent@45 | — | Inter 600 15.0px on-accent@45 | — | 18.0 |
| All styles | Button "Danger" | 89.0 × 40.0 | radius-md | danger | — | Inter 600 15.0px on-accent | — | 18.0 |
| All sizes | Button "Mini" | 43.0 × 22.0 | radius-md | bg-300 | 1.0 border | Inter 600 12.0px ink | — | space-2 |
| All sizes | Button "Small" | 59.0 × 32.0 | radius-md | bg-300 | 1.0 border | Inter 600 13.0px ink | — | space-3 |
| All sizes | Button "Large" | 89.0 × 48.0 | radius-md | bg-300 | 1.0 border | Inter 600 15.0px ink | — | space-6 |

## API

Builder methods: `accessible_description`, `accessible_name`, `disabled`, `full_width`, `icon`, `loading`, `new`, `on_click`, `selected`, `shortcut`, `size`, `style`, `trailing_icon`.

`ButtonSize`: `Large`, `Medium`, `Mini`, `Small`.

`ButtonStyle`: `Danger`, `Ghost`, `Primary`, `Secondary`.

## Example

```rust
fn button() -> impl View {
    let clicks = State::new(0);
    let add = clicks.clone();
    VStack::new((
        HStack::new((
            Button::new("Save").on_click(move || add.update(|n| *n += 1)),
            Button::new("Cancel").style(ButtonStyle::Secondary),
            Button::new("Show more").style(ButtonStyle::Ghost),
            Button::new("Delete").style(ButtonStyle::Danger).icon(IconName::Trash2),
        ))
        .spacing(8.0)
        .wrap(),
        HStack::new((
            Button::new("Upload").style(ButtonStyle::Secondary).loading(true),
            Button::new("Show grid").style(ButtonStyle::Secondary).selected(true),
            Button::new("Open").style(ButtonStyle::Ghost).shortcut("F8"),
        ))
        .spacing(8.0)
        .wrap(),
        Text::bound(&clicks, |n| format!("Saved {n} times")).foreground(Semantic::InkMuted),
    ))
    .spacing(12.0)
}
```

Every number above is in `Button.json` beside this card.
