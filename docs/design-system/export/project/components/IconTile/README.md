# IconTile

An icon in a square tinted with its color.

**Family:** Display and feedback · **Rust:** `ravenrs::prelude::IconTile`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `tone` | Neutral · Accent · Success · Warning · Danger | `Success` |  |
| `label` | text | `` | Empty leaves it as the component has it. |
| `size` | number 0–192 by 1 | `48` |  |

## Stories

- **Example**: 152 × 48 pt, settings `{"label":"","size":48.0,"tone":"Success"}`
- **With a label**: 152 × 48 pt, settings `{"label":"Paid","size":48.0,"tone":"Success"}`
- **Small**: 136 × 48 pt, settings `{"label":"","size":32.0,"tone":"Success"}`
- **Tone: Neutral**: 152 × 48 pt, settings `{"label":"","size":48.0,"tone":"Neutral"}`
- **Tone: Accent**: 152 × 48 pt, settings `{"label":"","size":48.0,"tone":"Accent"}`
- **Tone: Warning**: 152 × 48 pt, settings `{"label":"","size":48.0,"tone":"Warning"}`
- **Tone: Danger**: 152 × 48 pt, settings `{"label":"","size":48.0,"tone":"Danger"}`

## Sizing

Hugs its content: 152 × 48 pt in a parent of 760 pt.

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| With a label | Image "Paid" | 48.0 × 48.0 | radius-lg | success@14 | — | — | circle-check 28.0px success | 10.0 |

## API

Builder methods: `color`, `label`, `new`, `size`, `tone`.

## Example

```rust
fn icon_tile() -> impl View {
    HStack::new((
        IconTile::new(IconName::CircleCheck).tone(Tone::Success),
        IconTile::new(IconName::TriangleAlert).tone(Tone::Warning),
        IconTile::new(IconName::Image).size(32.0),
    ))
    .spacing(12.0)
    .align(Align::Center)
}
```

Every number above is in `IconTile.json` beside this card.
