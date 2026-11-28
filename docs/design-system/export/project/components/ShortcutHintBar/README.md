# ShortcutHintBar

Line of key hints under what they act on.

**Family:** Display and feedback · **Rust:** `ravenrs::prelude::ShortcutHintBar`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `leading` | text | `3 of 40` |  |

## Stories

- **Example**: 294 × 19 pt, settings `{"leading":"3 of 40"}`

## Sizing

Hugs its content: 294 × 19 pt in a parent of 760 pt; in a parent of 320 pt it narrows to 230 pt; its height follows the width, as its content wraps.

## Behavior

- **Accessibility**: role `Label` "3 of 40".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Label "3 of 40" | 45.0 × 16.0 | — | — | — | Inter 600 12.0px ink-muted | — | 0.0 |
| Example | Label "←" | 12.0 × 16.0 | — | — | — | Inter 400 12.0px ink-muted | — | 0.0 |

## API

Builder methods: `hint`, `leading`, `new`.

## Example

```rust
fn shortcut_hint_bar() -> impl View {
    ShortcutHintBar::new()
        .leading("3 of 40")
        .hint(["←", "→"], "move")
        .hint(["1", "5"], "rate")
        .hint(["P"], "pick")
}
```

Every number above is in `ShortcutHintBar.json` beside this card.
