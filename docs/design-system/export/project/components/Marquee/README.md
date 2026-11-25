# Marquee

Marching ants around a rectangle, a path or loose segments.

**Family:** Media · **Rust:** `ravenrs::prelude::Marquee`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `animated` | on/off | `true` |  |
| `label` | text | `` | Empty leaves it as the component has it. |

## Stories

- **Example**: 312 × 140 pt, settings `{"animated":true,"label":""}`
- **Still**: 312 × 140 pt, settings `{"animated":false,"label":""}`

## Sizing

Hugs its content: 312 × 140 pt in a parent of 760 pt.

## Behavior

- **Accessibility**: role `Image` "Selection".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Image "Selection" | 150.0 × 140.0 | — | — | — | — | — | — |

## API

Builder methods: `animated`, `grow`, `label`, `path`, `rect`, `segments`, `size`, `space`, `stage`.

## Example

```rust
fn marquee() -> impl View {
    let lasso =
        [Point::new(20.0, 30.0), Point::new(120.0, 40.0), Point::new(100.0, 120.0), Point::new(30.0, 110.0)];
    HStack::new((
        Marquee::rect(Rect::new(20.0, 20.0, 110.0, 100.0)).size(150.0, 140.0),
        Marquee::path(lasso).size(150.0, 140.0),
    ))
    .spacing(12.0)
}
```

Every number above is in `Marquee.json` beside this card.
