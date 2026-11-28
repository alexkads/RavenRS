# Text

Text in the theme's type scale, with truncation.

**Family:** Display and feedback · **Rust:** `ravenrs::prelude::Text`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `truncate` | on/off | `false` |  |
| `mono` | on/off | `false` | The monospaced family, Fira Mono, for code or the lines of a receipt. |

## Stories

- **Example**: 361 × 160 pt, settings `{"mono":false,"truncate":false}`
- **Truncated**: 361 × 160 pt, settings `{"mono":false,"truncate":true}`
- **Monospaced**: 361 × 160 pt, settings `{"mono":true,"truncate":false}`
- **Type scale**: 192 × 242 pt, settings `{"mono":false,"truncate":false}`

## Sizing

Hugs its content: 361 × 160 pt in a parent of 760 pt.

## Behavior

- **Accessibility**: role `Heading` "Display".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Heading "Display" | 360.0 × 38.0 | — | — | — | Inter 600 32.0px ink | — | 0.0 |
| Example | Heading "Headline" | 360.0 × 24.0 | — | — | — | Inter 600 17.0px ink | — | 0.0 |
| Example | Label "Muted body text" | 360.0 × 22.0 | — | — | — | Inter 400 15.0px ink-muted | — | 0.0 |
| Example | Label "In the middle" | 360.0 × 22.0 | — | — | — | Inter 400 15.0px ink | — | 135.0 |
| Example | Label "At the end" | 360.0 × 22.0 | — | — | — | Inter 400 15.0px ink | — | 288.0 |
| Monospaced | Heading "Display" | 360.0 × 38.0 | — | — | — | Fira 500 32.0px ink | — | 0.0 |
| Monospaced | Heading "Headline" | 360.0 × 24.0 | — | — | — | Fira 500 17.0px ink | — | 0.0 |
| Monospaced | Label "Muted body text" | 360.0 × 22.0 | — | — | — | Fira 500 15.0px ink-muted | — | 0.0 |
| Monospaced | Label "In the middle" | 360.0 × 22.0 | — | — | — | Fira 500 15.0px ink | — | 122.0 |
| Monospaced | Label "At the end" | 360.0 × 22.0 | — | — | — | Fira 500 15.0px ink | — | 271.0 |
| Type scale | Label "Metric 42" | 192.0 × 48.0 | — | — | — | Inter 600 42.0px ink | — | 0.0 |
| Type scale | Heading "Title 22" | 80.0 × 28.0 | — | — | — | Inter 600 22.0px ink | — | 0.0 |
| Type scale | Label "Body 15" | 56.0 × 22.0 | — | — | — | Inter 400 15.0px ink | — | 0.0 |
| Type scale | Label "Callout 13" | 60.0 × 18.0 | — | — | — | Inter 400 13.0px ink | — | 0.0 |
| Type scale | Label "Caption 12" | 65.0 × 16.0 | — | — | — | Inter 600 12.0px ink | — | 0.0 |

## API

Builder methods: `align`, `bound`, `dynamic`, `font_size`, `font_weight`, `foreground`, `mono`, `new`, `style`, `truncate`.

## Example

```rust
fn text() -> impl View {
    VStack::new((
        Text::new("Display").style(TextStyle::Display),
        Text::new("Headline").style(TextStyle::Headline),
        Text::new("Muted body text").foreground(Semantic::InkMuted),
        Text::new("In the middle").align(TextAlign::Center),
        Text::new("At the end").align(TextAlign::End),
    ))
    .spacing(8.0)
    .align(Align::Stretch)
    .width(360.0)
}
```

Every number above is in `Text.json` beside this card.
