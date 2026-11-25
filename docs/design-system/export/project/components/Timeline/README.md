# Timeline

Blocks along time on named tracks.

**Family:** Forms · **Rust:** `ravenrs::prelude::Timeline`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `position_ms` | number 0–16000 by 1 | `4000` |  |
| `heads_width` | number 0–448 by 1 | `112` |  |
| `pixels_per_second` | number 0–112 by 1 | `28` |  |

## Stories

- **Example**: 728 × 138 pt, settings `{"heads_width":112.0,"pixels_per_second":28.0,"position_ms":4000.0}`

## Sizing

Fills the width of its parent: 288, 728, 1168 pt wide in parents of 320, 760, 1200 pt.

## Behavior

- **Events**: `on_edit`, `on_seek`, `on_select`.
- **Accessibility**: role `Group`.

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Label "Notes" | 37.0 × 18.0 | — | — | — | Inter 400 13.0px ink | — | 0.0 |
| Example | Button "Callout" | 82.0 × 26.0 | radius-sm | ink-muted@34 | 2.0 focus-ring | Inter 600 12.0px ink | — | 10.0 |
| Example | Button "Zoom" | 110.0 × 26.0 | radius-sm | success@18 | 1.0 success | Inter 600 12.0px ink | — | 10.0 |
| Example | Button "Take 1" | 390.0 × 36.0 | radius-sm | #8196f92e | 1.0 mix(accent, focus-ring, 35) | Inter 600 12.0px ink | — | 10.0 |
| Example | Button "Chapter two" | 10.0 × 10.0 | radius-full | warning | — | — | — | — |

## API

Builder methods: `heads_width`, `markers`, `new`, `on_edit`, `on_seek`, `on_select`, `pixels_per_second`, `position_ms`, `total_height`.

`TimelineEdit`: `Began`, `Moved`, `TrimmedEnd`, `TrimmedStart`.

## Example

```rust
fn timeline() -> impl View {
    Timeline::new(vec![
        TimelineTrack::new(
            "Notes",
            34.0,
            vec![TimelineItem::new(1, 2_000, 3_000, "Callout").selected(true).movable()],
        ),
        TimelineTrack::new(
            "Screen",
            34.0,
            vec![TimelineItem::new(2, 6_000, 4_000, "Zoom").tone(Tone::Success)],
        ),
        TimelineTrack::new(
            "Film",
            44.0,
            vec![TimelineItem::new(3, 0, 14_000, "Take 1").tone(Tone::Accent).trimmable()],
        ),
    ])
    .markers(vec![TimelineMarker { at_ms: 6_000, title: "Chapter two".to_owned() }])
    .position_ms(4_000)
}
```

Every number above is in `Timeline.json` beside this card.
