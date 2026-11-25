# CountUp

A number that counts up when it first shows on screen.

**Family:** Layout · **Rust:** `ravenrs::prelude::CountUp`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `seconds` | number 0–6 by 0.1 | `1.5` |  |
| `start_at_value` | on/off | `false` |  |

## Stories

- **Example**: 179 × 68 pt, settings `{"seconds":1.5,"start_at_value":false}`
- **At its value**: 318 × 68 pt, settings `{"seconds":1.5,"start_at_value":true}`

## Sizing

Hugs its content: 179 × 68 pt in a parent of 760 pt.

## Behavior

- **Accessibility**: role `Label` "ROWS".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Label "ROWS" | 39.0 × 16.0 | — | — | — | Inter 600 12.0px ink-muted | — | 0.0 |
| Example | Label "0" | 27.0 × 48.0 | — | — | — | Inter 600 42.0px ink | — | 0.0 |
| At its value | Label "100,000" | 178.0 × 48.0 | — | — | — | Inter 600 42.0px ink | — | 5.0 |

## API

Builder methods: `easing`, `format`, `new`, `seconds`, `start_at_value`, `style`.

## Example

```rust
fn count_up() -> impl View {
    HStack::new((
        VStack::new((
            Text::new("ROWS").style(TextStyle::Caption).foreground(Semantic::InkMuted),
            CountUp::new(100_000.0),
        ))
        .spacing(4.0),
        VStack::new((
            Text::new("COMPONENTS").style(TextStyle::Caption).foreground(Semantic::InkMuted),
            CountUp::new(160.0).format(|v| format!("{v:.0}+")),
        ))
        .spacing(4.0),
    ))
    .spacing(48.0)
}
```

Every number above is in `CountUp.json` beside this card.
