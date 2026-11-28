# Barcode

QR Code and linear barcodes that scanners can read.

**Family:** Data · **Rust:** `ravenrs::prelude::Barcode`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `alt` | text | `Link to the RavenRS site` |  |
| `hide_text` | on/off | `false` |  |

## Stories

- **Example**: 424 × 160 pt, settings `{"alt":"Link to the RavenRS site","hide_text":false}`
- **Without text**: 424 × 160 pt, settings `{"alt":"Link to the RavenRS site","hide_text":true}`

## Sizing

Hugs its content: 424 × 160 pt in a parent of 760 pt.

## Behavior

- **Accessibility**: role `Image` "Link to the RavenRS site".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Image "Link to the RavenRS site" | 160.0 × 160.0 | — | — | — | — | — | — |
| Example | Image "789123456789" | 240.0 × 110.0 | — | — | — | Inter 400 12.0px scrim | — | 74.0 |

## API

Builder methods: `alt`, `hide_text`, `new`, `qr`, `size`.

## Example

```rust
fn barcode() -> impl View {
    HStack::new((
        Barcode::qr("https://ravenrs.dev").alt("Link to the RavenRS site"),
        Barcode::new(Symbology::Ean13, "789123456789"),
    ))
    .spacing(24.0)
}
```

Every number above is in `Barcode.json` beside this card.
