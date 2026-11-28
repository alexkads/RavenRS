# Splitter

Two panes with a draggable divider.

**Family:** Layout · **Rust:** `ravenrs::prelude::Splitter`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `ratio` | number 0.1–0.9 by 0.01 | `0.32` | The first pane's share. |
| `height` | number 0–760 by 1 | `190` |  |

## Stories

- **Example**: 728 × 190 pt, settings `{"height":190.0,"ratio":0.32}`
- **Half and half**: 728 × 190 pt, settings `{"height":190.0,"ratio":0.5}`

## Sizing

Fills the width of its parent: 288, 728, 1168 pt wide in parents of 320, 760, 1200 pt.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover** on "Resize": fill border → ink-muted.
- **Focus** on "Resize": fill border → ink-muted; adds a rect (border 2.0 focus-ring).
- **Pressed** on "Resize": fill border → ink-muted.

## Behavior

- **Keys** its first control handles once focused: `ArrowDown`, `ArrowUp`, `ArrowLeft`, `ArrowRight`, `Escape`, `Home`, `End`.
- **Accessibility**: role `Label` "Explorer".
- **Focus order** (Tab): Slider "Resize".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Label "Explorer" | 198.0 × 22.0 | — | — | — | Inter 400 15.0px ink | — | 0.0 |
| Example | Slider "Resize" | 10.0 × 190.0 | — | — | — | — | — | — |

## API

Builder methods: `height`, `horizontal`, `limits`, `ratio`, `vertical`.

## Example

```rust
fn splitter() -> impl View {
    Splitter::horizontal(Text::new("Explorer").padding(16.0), Text::new("Editor").padding(16.0))
        .ratio(0.32)
        .height(190.0)
}
```

Every number above is in `Splitter.json` beside this card.
