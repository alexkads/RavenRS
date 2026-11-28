# Checkbox

Check box with its label; clicking the label toggles it.

**Family:** Actions and inputs · **Rust:** `ravenrs::prelude::Checkbox`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `label` | text | `I accept the terms` |  |
| `checked` | on/off | `true` |  |
| `disabled` | on/off | `false` |  |
| `read_only` | on/off | `false` |  |
| `mixed` | on/off | `false` | A select all whose children are partly checked; a click checks it. |
| `optional` | on/off | `false` | An answer that may be unknown, shown mixed; clicks go unknown, on, off. |

## Stories

- **Checked**: 158 × 22 pt, settings `{"checked":true,"disabled":false,"label":"I accept the terms","mixed":false,"optional":false,"read_only":false}`
- **Unchecked**: 193 × 22 pt, settings `{"checked":false,"disabled":false,"label":"Send me product news","mixed":false,"optional":false,"read_only":false}`
- **Disabled**: 158 × 22 pt, settings `{"checked":true,"disabled":true,"label":"I accept the terms","mixed":false,"optional":false,"read_only":false}`
- **Read only**: 158 × 22 pt, settings `{"checked":true,"disabled":false,"label":"I accept the terms","mixed":false,"optional":false,"read_only":true}`
- **Mixed**: 98 × 22 pt, settings `{"checked":true,"disabled":false,"label":"Select all","mixed":true,"optional":false,"read_only":false}`
- **Unknown answer**: 99 × 22 pt, settings `{"checked":true,"disabled":false,"label":"Has a car","mixed":false,"optional":true,"read_only":false}`

## Sizing

Hugs its content: 158 × 22 pt in a parent of 760 pt.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Checked:

- **Hover**: "I accept the terms" does not change.
- **Focus** on "I accept the terms": adds a rect (border 2.0 focus-ring).
- **Pressed**: "I accept the terms" does not change.

## Behavior

- **Keys** its first control handles once focused: `Enter`, `Space`.
- **Accessibility**: role `CheckBox` "I accept the terms".
- **Focus order** (Tab): CheckBox "I accept the terms".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Checked | CheckBox "I accept the terms" | 157.0 × 22.0 | — | — | — | Inter 400 15.0px ink | check 14.0px on-accent | 2.0 |
| Unchecked | CheckBox "Send me product news" | 192.0 × 22.0 | — | — | — | Inter 400 15.0px ink | — | 28.0 |
| Disabled | CheckBox "I accept the terms" | 157.0 × 22.0 | — | — | — | Inter 400 15.0px ink@45 | check 14.0px on-accent | 2.0 |
| Mixed | CheckBox "Select all" | 93.0 × 22.0 | — | — | — | Inter 400 15.0px ink | minus 14.0px on-accent | 2.0 |

## API

Builder methods: `disabled`, `leading`, `mixed`, `new`, `optional`, `read_only`.

## Example

```rust
fn checkbox() -> impl View {
    let terms = State::new(true);
    let news = State::new(false);
    VStack::new((
        Checkbox::new("I accept the terms", terms.binding()),
        Checkbox::new("Send me product news", news.binding()),
    ))
    .spacing(12.0)
}
```

Every number above is in `Checkbox.json` beside this card.
