# Image

Shows an image, with fit modes.

**Family:** Media · **Rust:** `ravenrs::prelude::Image`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `corner_radius` | number 0–48 by 0.1 | `12` |  |
| `crossfade` | number 0–1 by 0.01 | `0.2` |  |
| `alt` | text | `` | Empty leaves it as the component has it. |
| `content_mode` | Cover · Fit · Fill | `Cover` |  |
| `opacity` | number 0–1 by 0.05 | `1` |  |
| `stretch` | on/off | `false` |  |

## Stories

- **Example**: 262 × 90 pt, settings `{"alt":"","content_mode":"Cover","corner_radius":12.0,"crossfade":0.2,"opacity":1.0,"stretch":false}`
- **With a description**: 262 × 90 pt, settings `{"alt":"A crow on a branch","content_mode":"Cover","corner_radius":12.0,"crossfade":0.2,"opacity":1.0,"stretch":false}`
- **Square corners**: 262 × 90 pt, settings `{"alt":"","content_mode":"Cover","corner_radius":0.0,"crossfade":0.2,"opacity":1.0,"stretch":false}`
- **Content mode: Fit**: 262 × 90 pt, settings `{"alt":"","content_mode":"Fit","corner_radius":12.0,"crossfade":0.2,"opacity":1.0,"stretch":false}`
- **Content mode: Fill**: 262 × 90 pt, settings `{"alt":"","content_mode":"Fill","corner_radius":12.0,"crossfade":0.2,"opacity":1.0,"stretch":false}`

## Sizing

Hugs its content: 262 × 90 pt in a parent of 760 pt.

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| With a description | Image "A crow on a branch" | 160.0 × 90.0 | — | — | — | — | — | — |

## API

Builder methods: `alt`, `content_mode`, `corner_radius`, `crossfade`, `from_bytes`, `height`, `new`, `opacity`, `size`, `stretch`, `width`.

## Example

```rust
fn image() -> impl View {
    HStack::new((
        // `.crossfade` dissolves into the next image the app hands it.
        Image::new(picture(1.0)).size(160.0, 90.0).corner_radius(12.0).crossfade(0.2),
        Image::new(picture(0.3)).size(90.0, 90.0).corner_radius(45.0),
    ))
    .spacing(12.0)
}
```

Every number above is in `Image.json` beside this card.
