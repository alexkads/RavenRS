# TextArea

Text on several lines, scrolling past its height or growing with the text.

**Family:** Actions and inputs · **Rust:** `ravenrs::prelude::TextArea`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `fixed` | on/off | `false` | A fixed height in lines; off, it grows from 2 to 6 lines as you type. |
| `rows` | number 1–20 by 1 | `4` | The lines of a fixed height; 4 by default. |
| `placeholder` | text | `Notes` |  |
| `disabled` | on/off | `false` |  |
| `read_only` | on/off | `false` |  |

## Stories

- **Example**: 728 × 56 pt, settings `{"disabled":false,"fixed":false,"placeholder":"Notes","read_only":false,"rows":4.0}`
- **Fixed height**: 728 × 100 pt, settings `{"disabled":false,"fixed":true,"placeholder":"Notes","read_only":false,"rows":4.0}`
- **Disabled**: 728 × 56 pt, settings `{"disabled":true,"fixed":false,"placeholder":"Notes","read_only":false,"rows":4.0}`
- **Read only**: 728 × 56 pt, settings `{"disabled":false,"fixed":false,"placeholder":"Notes","read_only":true,"rows":4.0}`

## Sizing

Fills the width of its parent: 288, 728, 1168 pt wide in parents of 320, 760, 1200 pt.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover** on "Notes": border {color:control-border,width:1.0} → {color:mix(ink, control-border, 25),width:1.0}.
- **Focus** on "Notes": border {color:control-border,width:1.0} → {color:accent,width:1.5}; adds a rect (accent); adds a rect (border 2.0 focus-ring).
- **Pressed** on "Notes": border {color:control-border,width:1.0} → {color:accent,width:1.5}; adds a rect (accent).

## Behavior

- **Keys** its first control handles once focused: `ArrowUp`, `ArrowLeft`, `Enter`, `Space`, `Escape`, `Home`.
- **Accessibility**: role `TextInput` "Notes".
- **Focus order** (Tab): TextInput "Notes".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | TextInput "Notes" | 728.0 × 56.0 | radius-md | bg-300 | 1.0 control-border | Inter 400 15.0px ink | — | space-3 |
| Fixed height | TextInput "Notes" | 728.0 × 100.0 | radius-md | bg-300 | 1.0 control-border | Inter 400 15.0px ink | — | space-3 |
| Disabled | TextInput "Notes" | 728.0 × 56.0 | radius-md | bg-300@45 | 1.0 control-border@45 | Inter 400 15.0px ink@45 | — | space-3 |
| Read only | TextInput "Notes" | 728.0 × 56.0 | radius-md | bg-300@50 | 1.0 control-border | Inter 400 15.0px ink | — | space-3 |

## API

Builder methods: `disabled`, `grows`, `new`, `placeholder`, `read_only`, `rows`, `width`.

## Example

```rust
fn text_area() -> impl View {
    let notes = State::new(String::from("Two lines\nto start with."));
    // Grows from 2 to 6 lines as you type; `.rows(4)` would fix the height.
    TextArea::new(notes.binding()).placeholder("Notes").grows(2, 6)
}
```

Every number above is in `TextArea.json` beside this card.
