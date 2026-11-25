# FlowLibrary

The diagrams kept beside a FlowCanvas: open, save, rename and start new ones.

**Family:** Data · **Rust:** `ravenrs::prelude::FlowLibrary`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `title` | text | `` | Empty: no title. |
| `width` | number 160–400 by 8 | `240` |  |

## Stories

- **Library**: 240 × 206 pt, settings `{"title":"","width":240.0}`
- **With title**: 240 × 206 pt, settings `{"title":"Scenarios","width":240.0}`

## Sizing

Hugs its content: 240 × 206 pt in a parent of 760 pt.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Library:

- **Hover** on "New": fill transparent → ink@7.
- **Focus** on "New": adds a rect (border 2.0 focus-ring).
- **Pressed** on "New": fill transparent → ink@12, rect [90.0,16.0,32.0,32.0] → [90.48,16.48,31.04,31.04].

## Behavior

- **Keys** its first control handles once focused: `Enter`, `Space`.
- **Accessibility**: role `Group` "Diagrams".
- **Focus order** (Tab): Button "New" → Button "Save" → Button "Open file" → Button "Save as file" → TextInput "Name of the diagram" → Button "Untitled" → Button "Delete Untitled".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Library | Label "Diagrams" | 59.0 × 16.0 | — | — | — | Inter 600 12.0px ink-muted | — | 0.0 |
| Library | Button "New" | 32.0 × 32.0 | radius-md | transparent | — | — | file-plus 14.0px ink | 9.0 |
| Library | Button "Save" | 32.0 × 32.0 | radius-md | transparent | — | — | save 14.0px ink | 9.0 |
| Library | Button "Open file" | 32.0 × 32.0 | radius-md | transparent | — | — | folder-open 14.0px ink | 9.0 |
| Library | Button "Save as file" | 32.0 × 32.0 | radius-md | transparent | — | — | upload 14.0px ink | 9.0 |
| Library | TextInput "Name of the diagram" | 208.0 × 40.0 | radius-md | bg-300 | 1.0 control-border | Inter 400 15.0px ink | — | space-3 |
| Library | Label "Changes not saved" | 208.0 × 16.0 | — | — | — | Inter 600 12.0px warning | — | 0.0 |
| Library | Button "Untitled" | 157.0 × 62.0 | radius-md | bg-300 | — | Inter 600 15.0px ink | file 18.0px ink-muted | 21.0 |
| Library | Button "Delete Untitled" | 32.0 × 32.0 | radius-md | transparent | — | — | trash-2 14.0px ink | 9.0 |

## API

Builder methods: `format`, `history`, `key`, `new`, `notation`, `selection`, `starter`, `storage`, `title`, `view`, `width`.

Every number above is in `FlowLibrary.json` beside this card.
