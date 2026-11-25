# DropZone

Dashed area around any content, which takes dropped files.

**Family:** Actions and inputs · **Rust:** `ravenrs::prelude::DropZone`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `label` | text | `Files of the session` |  |
| `min_height` | number 0–384 by 1 | `96` |  |
| `choosable` | on/off | `true` |  |
| `disabled` | on/off | `false` |  |
| `fill` | on/off | `false` |  |
| `hint` | text | `` | Empty leaves it as the component has it. |
| `no_hint` | on/off | `false` |  |
| `quiet` | on/off | `false` |  |

## Stories

- **Example**: 728 × 96 pt, settings `{"choosable":true,"disabled":false,"fill":false,"hint":"","label":"Files of the session","min_height":96.0,"no_hint":false,"quiet":false}`
- **Disabled**: 728 × 96 pt, settings `{"choosable":true,"disabled":true,"fill":false,"hint":"","label":"Files of the session","min_height":96.0,"no_hint":false,"quiet":false}`

## Sizing

Fills the width of its parent: 288, 728, 1168 pt wide in parents of 320, 760, 1200 pt.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover**: "Files of the session" does not change.
- **Focus** on "Files of the session": adds a rect (border 2.0 focus-ring).
- **Pressed**: "Files of the session" does not change.

## Behavior

- **Events**: `on_files`, `on_refused`.
- **Accessibility**: role `Button` "Files of the session".
- **Focus order** (Tab): Button "Files of the session".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Button "Files of the session" | 728.0 × 96.0 | — | — | — | Inter 400 13.0px ink-muted | — | space-4 |
| Disabled | Label "0 files received. Drop more, or press Enter here." | 696.0 × 18.0 | — | — | — | Inter 400 13.0px ink-muted | — | 0.0 |

## API

Builder methods: `accept`, `choosable`, `disabled`, `fill`, `hint`, `label`, `min_height`, `most_bytes`, `most_files`, `new`, `no_hint`, `on_files`, `on_refused`, `padding`, `quiet`.

## Example

```rust
fn drop_zone() -> impl View {
    let count = State::new(0_usize);
    let (shown, add) = (count.clone(), count);
    DropZone::new(
        Text::bound(&shown, |n| format!("{n} files received. Drop more, or press Enter here."))
            .style(TextStyle::Callout)
            .foreground(Semantic::InkMuted),
    )
    .label("Files of the session")
    .min_height(96.0)
    .choosable()
    .on_files(move |files| add.update(|n| *n += files.len()))
}
```

Every number above is in `DropZone.json` beside this card.
