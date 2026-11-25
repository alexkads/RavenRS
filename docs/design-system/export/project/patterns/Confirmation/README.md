# Confirmation

Asking before an action that cannot be undone: the question, what follows and the two answers.

**Family:** Patterns · **Rust:** `ravenrs::prelude::Confirmation`

## Stories

- **Pattern**: 594 × 634 pt, settings `{}`

## Sizing

Fills the width of its parent: 288, 460, 460 pt wide in parents of 320, 760, 1200 pt; in a parent of 320 pt it narrows to 288 pt; its height follows the width, as its content wraps.

## Behavior

- **Accessibility**: role `Dialog` "Delete this invoice?".
- **Focus order** (Tab): Button "Close" → Button "Cancel" → Button "Delete".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Pattern | Dialog "Delete this invoice?" | 460.0 × 200.0 | radius-xl | bg-200 | 1.0 border | Inter 600 22.0px ink | x 14.0px ink | space-6 |
| Pattern | Heading "Delete this invoice?" | 368.0 × 28.0 | — | — | — | Inter 600 22.0px ink | — | 0.0 |
| Pattern | Button "Close" | 32.0 × 32.0 | radius-md | transparent | — | — | x 14.0px ink | 9.0 |
| Pattern | Label "INV-1042 goes away with its payments. This cannot be undone." | 412.0 × 44.0 | — | — | — | Inter 400 15.0px ink-muted | — | 0.0 |
| Pattern | Button "Cancel" | 87.0 × 40.0 | radius-md | bg-300 | 1.0 border | Inter 600 15.0px ink | — | 18.0 |
| Pattern | Button "Delete" | 107.0 × 40.0 | radius-md | danger | — | Inter 600 15.0px on-accent | trash-2 16.0px on-accent | 18.0 |

Every number above is in `Confirmation.json` beside this card.
