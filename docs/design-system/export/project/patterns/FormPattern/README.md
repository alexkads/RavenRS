# FormPattern

A form: labeled fields in two columns, a hint, an error and the actions at the end, the one that matters accented.

**Family:** Patterns · **Rust:** `ravenrs::prelude::FormPattern`

## Stories

- **Pattern**: 728 × 380 pt, settings `{}`

## Sizing

Fills the width of its parent: 288, 728, 1168 pt wide in parents of 320, 760, 1200 pt; its height follows the width, as its content wraps.

## Behavior

- **Accessibility**: role `Label` "Customer".
- **Focus order** (Tab): TextInput "Name" → TextInput "Email" → TextInput "Phone" → ComboBox "Plan" → TextInput "Notes" → Button "Cancel" → Button "Save customer".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Pattern | Label "Customer" | 728.0 × 24.0 | — | — | — | Inter 600 17.0px ink | — | 0.0 |
| Pattern | Label "Name" | 37.0 × 18.0 | — | — | — | Inter 600 13.0px ink | — | 0.0 |
| Pattern | Label "*" | 8.0 × 18.0 | — | — | — | Inter 600 13.0px danger | — | 0.0 |
| Pattern | TextInput "Name" | 356.0 × 40.0 | radius-md | bg-300 | 1.0 control-border | Inter 400 15.0px ink | — | space-3 |
| Pattern | Label "Enter an email like name@example.com." | 248.0 × 18.0 | — | — | — | Inter 400 13.0px danger | — | 0.0 |
| Pattern | TextInput "Phone" | 356.0 × 40.0 | radius-md | bg-300 | 1.0 control-border | Inter 400 15.0px ink-muted | — | space-3 |
| Pattern | Label "For the delivery only." | 356.0 × 18.0 | — | — | — | Inter 400 13.0px ink-muted | — | 0.0 |
| Pattern | ComboBox "Plan" | 220.0 × 40.0 | radius-md | bg-300 | 1.0 control-border | Inter 400 15.0px ink | chevron-down 16.0px ink-muted | space-3 |
| Pattern | TextInput "Notes" | 728.0 × 56.0 | radius-md | bg-300 | 1.0 control-border | — | — | — |
| Pattern | Button "Cancel" | 87.0 × 40.0 | radius-md | bg-300 | 1.0 border | Inter 600 15.0px ink | — | 18.0 |
| Pattern | Button "Save customer" | 169.0 × 40.0 | radius-md | accent | — | Inter 600 15.0px on-accent | check 16.0px on-accent | 18.0 |

Every number above is in `FormPattern.json` beside this card.
