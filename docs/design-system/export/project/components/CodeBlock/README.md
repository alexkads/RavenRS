# CodeBlock

A command to copy.

**Family:** Forms · **Rust:** `ravenrs::prelude::CodeBlock`

## Stories

- **Example**: 201 × 38 pt, settings `{}`

## Sizing

Hugs its content: 201 × 38 pt in a parent of 760 pt; in a parent that stretches its children, as a form column, it takes the whole width.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover** on "Copy": fill transparent → ink@7.
- **Focus** on "Copy": adds a rect (border 2.0 focus-ring).
- **Pressed** on "Copy": fill transparent → ink@12, rect [128.0,8.0,65.0,22.0] → [128.98,8.33,63.05,21.34].

## Behavior

- **Keys** its first control handles once focused: `Enter`, `Space`.
- **Accessibility**: role `Label` "cargo add ravenrs".
- **Focus order** (Tab): Button "Copy".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Label "cargo add ravenrs" | 112.0 × 16.0 | — | — | — | Inter 600 12.0px ink | — | 0.0 |
| Example | Button "Copy" | 65.0 × 22.0 | radius-md | transparent | — | Inter 600 12.0px ink | copy 12.0px ink | space-2 |

## API

Builder methods: `new`.

## Example

```rust
fn code_block() -> impl View {
    CodeBlock::new("cargo add ravenrs")
}
```

Every number above is in `CodeBlock.json` beside this card.
