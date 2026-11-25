# BarChart

Bar chart with labels and faint gridlines.

**Family:** Data · **Rust:** `ravenrs::prelude::BarChart`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `height` | number 0–720 by 1 | `180` |  |

## Stories

- **Example**: 728 × 204 pt, settings `{"height":180.0}`

## Sizing

Fills the width of its parent: 288, 728, 1168 pt wide in parents of 320, 760, 1200 pt.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover** on "Mon: 42, Tue: 58, Wed: 51, Thu: 73, Fri: 66": adds a rect (bg-300@60); adds a rect (border@80); adds a rect (border@80); adds a rect (border@80).
- **Focus** on "Mon: 42, Tue: 58, Wed: 51, Thu: 73, Fri: 66": adds a rect (border@80); adds a rect (border@80); adds a rect (border@80); adds a rect (border@80).
- **Pressed** on "Mon: 42, Tue: 58, Wed: 51, Thu: 73, Fri: 66": adds a rect (bg-300@60); adds a rect (border@80); adds a rect (border@80); adds a rect (border@80).

## Behavior

- **Keys** its first control handles once focused: `ArrowDown`, `ArrowUp`, `ArrowLeft`, `ArrowRight`, `Home`, `End`.
- **Accessibility**: role `Image` "Mon: 42, Tue: 58, Wed: 51, Thu: 73, Fri: 66".
- **Focus order** (Tab): Image "Mon: 42, Tue: 58, Wed: 51, Thu: 73, Fri: 66".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Image "Mon: 42, Tue: 58, Wed: 51, Thu: 73, Fri: 66" | 728.0 × 204.0 | — | — | — | Inter 400 12.0px ink-muted | — | 60.0 |
| Example | ListItem "Mon: 42" | 0.0 × 0.0 | — | — | — | — | — | — |

## API

Builder methods: `height`, `new`.

## Example

```rust
fn bar_chart() -> impl View {
    BarChart::new([("Mon", 42.0), ("Tue", 58.0), ("Wed", 51.0), ("Thu", 73.0), ("Fri", 66.0)]).height(180.0)
}
```

Every number above is in `BarChart.json` beside this card.
