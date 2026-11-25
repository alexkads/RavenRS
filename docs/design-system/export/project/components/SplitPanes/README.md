# SplitPanes

Any number of panes with a bar between each two, and limits.

**Family:** Layout · **Rust:** `ravenrs::prelude::SplitPanes`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `height` | number 0–760 by 1 | `190` |  |

## Stories

- **Example**: 728 × 190 pt, settings `{"height":190.0}`

## Sizing

Fills the width of its parent: 288, 728, 1168 pt wide in parents of 320, 760, 1200 pt.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover** on "Resize": fill border → ink-muted.
- **Focus** on "Resize": fill border → ink-muted; adds a rect (border 2.0 focus-ring).
- **Pressed** on "Resize": fill border → ink-muted.

## Behavior

- **Events**: `on_end`.
- **Keys** its first control handles once focused: `ArrowDown`, `ArrowUp`, `ArrowLeft`, `ArrowRight`, `Escape`, `Home`, `End`.
- **Accessibility**: role `Label` "Sessions".
- **Focus order** (Tab): Slider "Resize" → Slider "Resize".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Label "Sessions" | 148.0 × 22.0 | — | — | — | Inter 400 15.0px ink | — | 0.0 |
| Example | Slider "Resize" | 10.0 × 190.0 | — | — | — | — | — | — |

## API

Builder methods: `height`, `horizontal`, `on_end`, `pane`, `sizes`, `vertical`.

## Example

```rust
fn split_panes() -> impl View {
    SplitPanes::horizontal()
        .pane(SplitPane::new(Text::new("Sessions").padding(16.0)).size(180.0).range(120.0, 260.0))
        .pane(SplitPane::new(Text::new("Receipt").padding(16.0)).range(120.0, f32::MAX))
        .pane(SplitPane::new(Text::new("Activity").padding(16.0)).size(180.0).range(120.0, 260.0))
        .height(190.0)
}
```

Every number above is in `SplitPanes.json` beside this card.
