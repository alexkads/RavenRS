# Chronology

Moments down a line: a date, a mark joined to the next, a title, a detail and a badge.

**Family:** Lists and trees · **Rust:** `ravenrs::prelude::Chronology`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `tone` | Neutral · Accent · Success · Warning · Danger | `Accent` |  |
| `badges` | on/off | `false` | A badge beside each title. |
| `label` | text | `Marcos` |  |

## Stories

- **Example**: 293 × 186 pt, settings `{"badges":false,"label":"Marcos","tone":"Accent"}`
- **With badges**: 383 × 186 pt, settings `{"badges":true,"label":"Marcos","tone":"Accent"}`
- **Tones**: 383 × 186 pt, settings `{"badges":true,"label":"Marcos","tone":"Success"}`
- **Tone: Neutral**: 293 × 186 pt, settings `{"badges":false,"label":"Marcos","tone":"Neutral"}`
- **Tone: Warning**: 293 × 186 pt, settings `{"badges":false,"label":"Marcos","tone":"Warning"}`
- **Tone: Danger**: 293 × 186 pt, settings `{"badges":false,"label":"Marcos","tone":"Danger"}`

## Sizing

Hugs its content: 293 × 186 pt in a parent of 760 pt.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover**: "Batismo" does not change.
- **Focus** on "Batismo": adds a rect (border 2.0 focus-ring).
- **Pressed**: "Batismo" does not change.

## Behavior

- **Events**: `on_open`.
- **Keys** its first control handles once focused: `ArrowDown`.
- **Accessibility**: role `List` "Marcos".
- **Focus order** (Tab): ListItem "Batismo" → ListItem "Membro ativo" → ListItem "Transferência pedida".

## API

Builder methods: `label`, `new`, `on_open`.

Every number above is in `Chronology.json` beside this card.
