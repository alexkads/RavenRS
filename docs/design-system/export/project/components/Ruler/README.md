# Ruler

Horizontal or vertical ruler in your units, with a round step for any zoom.

**Family:** Media · **Rust:** `ravenrs::prelude::Ruler`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `scale` | number 0–8 by 0.1 | `2` |  |
| `origin` | number -80–80 by 1 | `-20` |  |
| `marker` | number 0–240 by 1 | `60` |  |
| `whole_units` | on/off | `false` |  |

## Stories

- **Example**: 480 × 44 pt, settings `{"marker":60.0,"origin":-20.0,"scale":2.0,"whole_units":false}`

## Sizing

Hugs its content: 480 × 44 pt in a parent of 760 pt.

## Behavior

- **Accessibility**: role `Image` "Ruler".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Image "Ruler" | 480.0 × 18.0 | 0.0 | bg-100 | — | Inter 600 9.0px ink-muted | — | 43.0 |
| Example | Image "Ruler" | 480.0 × 18.0 | 0.0 | bg-100 | — | Inter 600 9.0px ink-muted | — | 3.0 |

## API

Builder methods: `horizontal`, `label`, `marker`, `min_spacing`, `origin`, `scale`, `thickness`, `vertical`, `whole_units`.

## Example

```rust
fn ruler() -> impl View {
    VStack::new((
        Ruler::horizontal().scale(2.0).origin(-20.0).marker(60.0),
        Ruler::horizontal().scale(0.2).whole_units(),
    ))
    .spacing(8.0)
    .align(Align::Stretch)
}
```

Every number above is in `Ruler.json` beside this card.
