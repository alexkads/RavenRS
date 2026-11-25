# FlowLayers

The nodes of a diagram as a list, to find, select and reorder them.

**Family:** Data · **Rust:** `ravenrs::prelude::FlowLayers`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `width` | number 160–400 by 8 | `240` |  |

## Stories

- **Three nodes**: 240 × 228 pt, settings `{"width":240.0}`

## Sizing

Hugs its content: 240 × 228 pt in a parent of 760 pt.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Three nodes:

- **Hover** on "Issue receipt": fill transparent → ink@4.
- **Focus** on "Issue receipt": adds a rect (border 2.0 focus-ring); color ink → ink-muted.
- **Pressed** on "Issue receipt": fill transparent → ink@4.

## Behavior

- **Keys** its first control handles once focused: `Enter`, `Space`, `Escape`.
- **Accessibility**: role `Group` "Layers".
- **Focus order** (Tab): Button "Issue receipt" → Button "Hide Issue receipt" → Button "Bring Issue receipt forward" → Button "Send Issue receipt back" → Button "Check stock" → Button "Hide Check stock" → Button "Bring Check stock forward" → Button "Send Check stock back" → Button "New sale" → Button "Hide New sale" → Button "Bring New sale forward" → Button "Send New sale back".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Three nodes | Label "Layers" | 208.0 × 16.0 | — | — | — | Inter 600 12.0px ink-muted | — | 0.0 |
| Three nodes | Button "Issue receipt" | 194.0 × 56.0 | radius-md | transparent | — | Inter 600 15.0px ink | square 18.0px ink-muted | 21.0 |
| Three nodes | Button "Hide Issue receipt" | 32.0 × 32.0 | radius-md | transparent | — | — | chevron-right 16.0px ink-muted | 13.0 |
| Three nodes | Button "Bring Issue receipt forward" | 32.0 × 32.0 | radius-md | transparent | — | — | chevron-up 14.0px ink | 9.0 |
| Three nodes | Button "Send Issue receipt back" | 32.0 × 32.0 | radius-md | transparent | — | — | chevron-down 14.0px ink | 9.0 |
| Three nodes | Button "Hide New sale" | 32.0 × 32.0 | radius-md | transparent | — | — | chevron-right 16.0px ink-muted | space-1 |

## API

Builder methods: `history`, `new`, `width`.

Every number above is in `FlowLayers.json` beside this card.
