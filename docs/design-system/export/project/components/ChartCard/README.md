# ChartCard

A chart's card: title, description, what changes it, tabs with totals, and how it trends.

**Family:** Data · **Rust:** `ravenrs::prelude::ChartCard`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `title` | text | `Visitors` |  |
| `description` | text | `January to June` |  |
| `note` | text | `All channels` |  |
| `centered` | on/off | `false` |  |

## Stories

- **Example**: 210 × 384 pt, settings `{"centered":false,"description":"January to June","note":"All channels","title":"Visitors"}`
- **Centered**: 210 × 384 pt, settings `{"centered":true,"description":"January to June","note":"All channels","title":"Visitors"}`

## Sizing

Hugs its content: 210 × 384 pt in a parent of 760 pt; in a parent that stretches its children, as a form column, it takes the whole width.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover** on "Desktop: Jan: 186, Feb: 305, Mar: 237, Apr: 73, May: 209, Jun: 214; Mobile: Jan: 80, Feb: 200, Mar: 120, Apr: 190, May: 130, Jun: 140": adds a rect (ink-muted@55); adds a rect (accent@22); adds a rect (border 2.0 bg-200); adds a rect (chart-2@22).
- **Focus** on "Desktop: Jan: 186, Feb: 305, Mar: 237, Apr: 73, May: 209, Jun: 214; Mobile: Jan: 80, Feb: 200, Mar: 120, Apr: 190, May: 130, Jun: 140": adds a rect (border 2.0 focus-ring).
- **Pressed** on "Desktop: Jan: 186, Feb: 305, Mar: 237, Apr: 73, May: 209, Jun: 214; Mobile: Jan: 80, Feb: 200, Mar: 120, Apr: 190, May: 130, Jun: 140": adds a rect (ink-muted@55); adds a rect (accent@22); adds a rect (border 2.0 bg-200); adds a rect (chart-2@22).

## Behavior

- **Keys** its first control handles once focused: `ArrowDown`, `ArrowUp`, `ArrowLeft`, `ArrowRight`, `Home`, `End`.
- **Accessibility**: role `Label` "Visitors".
- **Focus order** (Tab): Image "Desktop: Jan: 186, Feb: 305, Mar: 237, Apr: 73, May: 209, Jun: 214; Mobile: Jan: 80, Feb: 200, Mar: 120, Apr: 190, May: 130, Jun: 140".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Label "Visitors" | 63.0 × 24.0 | — | — | — | Inter 600 17.0px ink | — | 0.0 |
| Example | Label "January to June" | 100.0 × 18.0 | — | — | — | Inter 400 13.0px ink-muted | — | 0.0 |
| Example | Image "Desktop: Jan: 186, Feb: 305, Mar: 237, Apr: 73, May: 209, Jun: 214; Mobile: Jan: 80, Feb: 200, Mar: 120, Apr: 190, May: 130, Jun: 140" | 162.0 × 196.0 | — | — | — | Inter 400 12.0px ink-muted | — | 2.0 |
| Example | ListItem "Desktop: Jan: 186" | 0.0 × 0.0 | — | — | — | — | — | — |
| Example | Label "Desktop" | 51.0 × 16.0 | — | — | — | Inter 600 12.0px ink-muted | — | 0.0 |
| Example | Label "Up by 5.2% this month" | 140.0 × 18.0 | — | — | — | Inter 400 13.0px ink | — | 0.0 |

## API

Builder methods: `action`, `centered`, `description`, `new`, `note`, `tabs`, `title`, `trend`.

## Example

```rust
fn chart_card() -> impl View {
    ChartCard::new(
        Chart::new(ChartKind::Area)
            .categories(["Jan", "Feb", "Mar", "Apr", "May", "Jun"])
            .series("Desktop", [186.0, 305.0, 237.0, 73.0, 209.0, 214.0])
            .series("Mobile", [80.0, 200.0, 120.0, 190.0, 130.0, 140.0])
            .stack(ChartStack::Normal)
            .gradient()
            .height(220.0),
    )
    .title("Visitors")
    .description("January to June")
    .trend("Up by 5.2% this month", IconName::TrendingUp)
    .note("All channels")
}
```

Every number above is in `ChartCard.json` beside this card.
