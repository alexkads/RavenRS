# OverlayBadge

A small view on a corner of another.

**Family:** Display and feedback · **Rust:** `ravenrs::prelude::OverlayBadge`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `corner` | TopLeft · TopRight · BottomLeft · BottomRight | `BottomRight` |  |
| `inset` | number 0–20 by 0.1 | `5` |  |
| `visible` | on/off | `true` |  |

## Stories

- **Example**: 201 × 40 pt, settings `{"corner":"BottomRight","inset":5.0,"visible":true}`
- **Hidden**: 201 × 40 pt, settings `{"corner":"BottomRight","inset":5.0,"visible":false}`
- **Corner: TopLeft**: 201 × 40 pt, settings `{"corner":"TopLeft","inset":5.0,"visible":true}`
- **Corner: TopRight**: 201 × 40 pt, settings `{"corner":"TopRight","inset":5.0,"visible":true}`
- **Corner: BottomLeft**: 201 × 40 pt, settings `{"corner":"BottomLeft","inset":5.0,"visible":true}`

## Sizing

Hugs its content: 201 × 40 pt in a parent of 760 pt.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover** on "Open inbox": fill bg-300 → mix(ink, bg-300, 6).
- **Focus** on "Open inbox": adds a rect (border 2.0 focus-ring).
- **Pressed** on "Open inbox": border {color:border,width:1.0} → {color:border,width:0.97}, fill bg-300 → mix(ink, bg-300, 12), rect [68.0,0.0,119.0,40.0] → [69.79,0.6,115.43,38.8].

## Behavior

- **Accessibility**: role `Image` "Ana Lima".
- **Focus order** (Tab): Button "Open inbox".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Image "Ana Lima" | 36.0 × 36.0 | radius-full | focus-ring@28 | 1.0 focus-ring@50 | Inter 600 14.0px mix(ink, focus-ring, 50) | — | 9.0 |
| Example | Image "Online" | 10.0 × 10.0 | radius-full | success | — | — | — | — |
| Example | Button "Open inbox" | 119.0 × 40.0 | radius-md | bg-300 | 1.0 border | Inter 600 15.0px ink | — | 18.0 |
| Example | Label "3" | 9.0 × 16.0 | — | — | — | Inter 600 12.0px mix(ink, danger, 30) | — | 0.0 |

## API

Builder methods: `corner`, `inset`, `new`, `visible`.

## Example

```rust
fn overlay_badge() -> impl View {
    HStack::new((
        OverlayBadge::new(Avatar::new("Ana Lima"), Dot::new().tone(Tone::Success).size(10.0).label("Online"))
            .corner(FloatingCorner::BottomRight)
            .inset(5.0),
        OverlayBadge::new(
            Button::new("Open inbox").style(ButtonStyle::Secondary),
            Badge::new("3").tone(Tone::Danger),
        ),
    ))
    .spacing(32.0)
    .align(Align::Center)
}
```

Every number above is in `OverlayBadge.json` beside this card.
