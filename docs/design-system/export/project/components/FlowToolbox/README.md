# FlowToolbox

The blocks a person drags onto a FlowCanvas, in groups, with a search.

**Family:** Data · **Rust:** `ravenrs::prelude::FlowToolbox`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `title` | text | `` | Empty: no title. |
| `searchable` | on/off | `true` |  |
| `width` | number 160–400 by 8 | `240` |  |

## Stories

- **Blocks**: 240 × 284 pt, settings `{"searchable":true,"title":"","width":240.0}`
- **With title**: 240 × 284 pt, settings `{"searchable":true,"title":"Blocks","width":240.0}`
- **No search**: 240 × 232 pt, settings `{"searchable":false,"title":"","width":240.0}`

## Sizing

Hugs its content: 240 × 284 pt in a parent of 760 pt.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Blocks:

- **Hover** on "Search": border {color:control-border,width:1.0} → {color:mix(ink, control-border, 25),width:1.0}.
- **Focus** on "Search": border {color:control-border,width:1.0} → {color:accent,width:1.5}; adds a rect (accent); adds a rect (border 2.0 focus-ring); fill accent → mix(accent, bg-200, 8).
- **Pressed** on "Search": border {color:control-border,width:1.0} → {color:accent,width:1.5}; adds a rect (accent); fill accent → mix(accent, bg-200, 8).

## Behavior

- **Keys** its first control handles once focused: `Space`, `Escape`.
- **Accessibility**: role `Group` "Shapes".
- **Focus order** (Tab): TextInput "Search" → Button "Triggers" → Button "New sale" → Button "Actions".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Blocks | Label "Shapes" | 46.0 × 16.0 | — | — | — | Inter 600 12.0px ink-muted | — | 0.0 |
| Blocks | TextInput "Search" | 208.0 × 40.0 | radius-md | bg-300 | 1.0 control-border | Inter 400 15.0px ink-muted | — | space-3 |
| Blocks | Button "Triggers" | 120.0 × 42.0 | 8.0 | transparent | — | Inter 600 15.0px ink | chevron-down 16.0px ink-muted | space-2 |
| Blocks | Button "New sale" | 80.0 × 68.0 | radius-md | bg-200 | 1.0 border | Inter 600 12.0px ink | — | space-3 |
| Blocks | Button "Actions" | 120.0 × 42.0 | 8.0 | transparent | — | Inter 600 15.0px ink | chevron-right 16.0px ink-muted | space-2 |

## API

Builder methods: `arrivals`, `group`, `model`, `new`, `notation`, `searchable`, `title`, `width`.

Every number above is in `FlowToolbox.json` beside this card.
