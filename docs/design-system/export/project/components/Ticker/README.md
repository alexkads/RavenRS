# Ticker

A strip of content that keeps moving sideways, still under reduced motion.

**Family:** Layout · **Rust:** `ravenrs::prelude::Ticker`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `speed` | number 0–160 by 1 | `40` |  |
| `gap` | number 0–48 by 0.1 | `12` |  |
| `reverse` | on/off | `false` |  |

## Stories

- **Example**: 1033 × 30 pt, settings `{"gap":12.0,"reverse":false,"speed":40.0}`
- **Reverse**: 1033 × 30 pt, settings `{"gap":12.0,"reverse":true,"speed":40.0}`

## Sizing

Hugs its content: 1033 × 30 pt in a parent of 760 pt.

## Behavior

- **Accessibility**: role `Label` "Button".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Label "Button" | 41.0 × 16.0 | — | — | — | — | — | — |
| Example | Label "DataGrid" | 55.0 × 16.0 | — | — | — | Inter 600 12.0px mix(ink, warning, 30) | — | 51.0 |
| Example | Label "Scheduler" | 63.0 × 16.0 | — | — | — | Inter 600 12.0px mix(ink, ink-muted, 30) | — | 31.0 |
| Example | Label "Chart" | 35.0 × 16.0 | — | — | — | Inter 600 12.0px #a1b1f8 | — | 1.0 |
| Example | Label "CommandPalette" | 106.0 × 16.0 | — | — | — | Inter 600 12.0px mix(ink, ink-muted, 30) | — | 7.0 |
| Reverse | Label "Scheduler" | 63.0 × 16.0 | — | — | — | Inter 600 12.0px #a1b1f8 | — | 44.0 |
| Reverse | Label "Ribbon" | 43.0 × 16.0 | — | — | — | Inter 600 12.0px mix(ink, ink-muted, 30) | — | 22.0 |
| Reverse | Label "Chart" | 35.0 × 16.0 | — | — | — | Inter 600 12.0px mix(success, ink, 70) | — | 34.0 |
| Reverse | Label "CommandPalette" | 106.0 × 16.0 | — | — | — | Inter 600 12.0px mix(ink, ink-muted, 30) | — | 62.0 |

## API

Builder methods: `gap`, `new`, `reverse`, `speed`.

## Example

```rust
fn ticker() -> impl View {
    let chips = HStack::new((
        Badge::new("Button").tone(Tone::Accent),
        Badge::new("DataGrid"),
        Badge::new("Scheduler").tone(Tone::Success),
        Badge::new("Ribbon"),
        Badge::new("Chart").tone(Tone::Warning),
        Badge::new("CommandPalette"),
    ))
    .spacing(12.0);
    VStack::new((Ticker::new(chips).speed(40.0).gap(12.0),)).padding(8.0)
}
```

Every number above is in `Ticker.json` beside this card.
