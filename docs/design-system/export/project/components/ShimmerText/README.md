# ShimmerText

Text with a light passing over it, still when motion is reduced.

**Family:** Display and feedback · **Rust:** `ravenrs::prelude::ShimmerText`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `active` | on/off | `true` |  |

## Stories

- **Example**: 129 × 18 pt, settings `{"active":true}`
- **Still**: 124 × 18 pt, settings `{"active":false}`

## Sizing

Hugs its content: 129 × 18 pt in a parent of 760 pt.

## Behavior

- **Accessibility**: role `Label` "Looking for updates".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Label "Looking for updates" | 123.0 × 18.0 | — | — | — | Inter 400 13.0px ink-muted | — | 0.0 |

## API

Builder methods: `active`, `new`, `style`.

## Example

```rust
fn shimmer_text() -> impl View {
    ShimmerText::new("Looking for updates")
}
```

Every number above is in `ShimmerText.json` beside this card.
