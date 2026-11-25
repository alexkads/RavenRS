# ConfirmDialog

Asks before an action that cannot be undone, or one worth a second look.

**Family:** Actions and inputs · **Rust:** `ravenrs::prelude::ConfirmDialog`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `open` | on/off | `true` | Asked as the story draws; off, only the button that asks. |
| `kind` | Danger · Plainly · Typing | `Danger` | Danger for what cannot be undone; Plainly for what can; Typing asks for a word first. |
| `title` | text | `Delete this record?` |  |
| `message` | text | `This cannot be undone.` |  |
| `action` | text | `Delete` | The button that confirms; a verb. |
| `word` | text | `DELETE` | What Typing asks to be typed. |

## Stories

- **Danger**: 594 × 623 pt, settings `{"action":"Delete","kind":"Danger","message":"This cannot be undone.","open":true,"title":"Delete this record?","word":"DELETE"}`
- **Plainly**: 594 × 623 pt, settings `{"action":"Finish","kind":"Plainly","message":"You can open it again later.","open":true,"title":"Finish this task?","word":"DELETE"}`
- **Typing**: 594 × 663 pt, settings `{"action":"Delete","kind":"Typing","message":"This cannot be undone.","open":true,"title":"Delete this record?","word":"DELETE"}`
- **Closed**: 83 × 40 pt, settings `{"action":"Delete","kind":"Danger","message":"This cannot be undone.","open":false,"title":"Delete this record?","word":"DELETE"}`

## Sizing

Hugs its content: 460 × 178 pt in a parent of 760 pt; in a parent of 320 pt it narrows to 288 pt; its height follows the width, as its content wraps.

- **Plainly**: Hugs its content: 460 × 178 pt in a parent of 760 pt; in a parent of 320 pt it narrows to 288 pt.
- **Closed**: Hugs its content: 83 × 40 pt in a parent of 760 pt.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Danger:

- **Hover** on "Close": fill transparent → ink@7.
- **Focus** on "Close": adds a rect (border 2.0 focus-ring).
- **Pressed** on "Close": fill transparent → ink@12, rect [538.0,469.0,32.0,32.0] → [538.48,469.48,31.04,31.04].

## Behavior

- **Keys** its first control handles once focused: `Enter`, `Space`, `Escape`.
- **Accessibility**: role `Dialog` "Delete this record?".
- **Focus order** (Tab): Button "Close" → Button "Cancel" → Button "Delete".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Danger | Dialog "Delete this record?" | 460.0 × 178.0 | radius-xl | bg-200 | 1.0 border | Inter 600 22.0px ink | x 14.0px ink | space-6 |
| Danger | Heading "Delete this record?" | 368.0 × 28.0 | — | — | — | Inter 600 22.0px ink | — | 0.0 |
| Danger | Button "Close" | 32.0 × 32.0 | radius-md | transparent | — | — | x 14.0px ink | 9.0 |
| Danger | Label "This cannot be undone." | 412.0 × 22.0 | — | — | — | Inter 400 15.0px ink-muted | — | 0.0 |
| Danger | Button "Cancel" | 87.0 × 40.0 | radius-md | bg-300 | 1.0 border | Inter 600 15.0px ink | — | 18.0 |
| Danger | Button "Delete" | 83.0 × 40.0 | radius-md | danger | — | Inter 600 15.0px on-accent | — | 18.0 |
| Plainly | Button "Finish" | 80.0 × 40.0 | radius-md | accent | — | Inter 600 15.0px on-accent | — | 18.0 |
| Typing | Dialog "Delete this record?" | 460.0 × 258.0 | radius-xl | bg-200 | 1.0 border | Inter 600 22.0px ink | x 14.0px ink | space-6 |
| Typing | Label "To confirm, type DELETE" | 412.0 × 18.0 | — | — | — | Inter 400 13.0px ink | — | 0.0 |
| Typing | TextInput "DELETE" | 412.0 × 40.0 | radius-md | bg-300 | 1.0 control-border | Inter 400 15.0px ink-muted | — | space-3 |
| Typing | Button "Delete" | 83.0 × 40.0 | radius-md | danger@45 | — | Inter 600 15.0px on-accent@45 | — | 18.0 |

## API

Builder methods: `ask`, `ask_plainly`, `ask_typing`, `new`.

## Example

```rust
fn confirm_dialog() -> impl View {
    let confirm = ConfirmDialog::new();
    let asker = confirm.clone();
    VStack::new((
        Button::new("Delete").style(ButtonStyle::Danger).on_click(move || {
            asker.ask("Delete this record?", "This cannot be undone.", "Delete", || {});
        }),
        confirm,
    ))
}
```

Every number above is in `ConfirmDialog.json` beside this card.
