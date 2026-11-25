# ScrollEdgeArrow

A strip that scrolls, with an arrow at each end that has more to show.

**Family:** Actions and inputs · **Rust:** `ravenrs::prelude::ScrollEdgeArrow`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `spacing` | number 0–32 by 0.1 | `8` |  |
| `vertical` | on/off | `false` |  |

## Stories

- **Example**: 728 × 72 pt, settings `{"spacing":8.0,"vertical":false}`

## Sizing

Fills the width of its parent: 288, 728, 1168 pt wide in parents of 320, 760, 1200 pt.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover** on "Scroll forward": color ink@70 → ink.
- **Focus** on "Scroll forward": adds a rect (border 2.0 focus-ring).
- **Pressed** on "Scroll forward": color ink@70 → ink.

## Behavior

- **Keys** its first control handles once focused: `Enter`, `Space`.
- **Accessibility**: role `Label` "Photo 1".
- **Focus order** (Tab): Button "Scroll forward".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Label "Photo 1" | 45.0 × 18.0 | — | — | — | Inter 400 13.0px ink | — | 0.0 |
| Example | Label "Photo 6" | 48.0 × 18.0 | — | — | — | Inter 400 13.0px ink | chevron-right 20.0px ink@70 | 0.0 |
| Example | Label "Photo 7" | 47.0 × 18.0 | — | — | — | — | — | — |
| Example | Button "Scroll forward" | 40.0 × 72.0 | 0.0 | {"end":[-16.0,-15.5],"from":"bg-200","start":[-15.0,-15.5],"to":"transparent"} | — | — | chevron-right 20.0px ink@70 | 10.0 |

## API

Builder methods: `fade`, `height`, `new`, `padding`, `spacing`, `vertical`.

## Example

```rust
fn scroll_edge_arrow() -> impl View {
    let photos: Vec<AnyView> = (1..=24)
        .map(|n| -> AnyView {
            Box::new(
                Text::new(format!("Photo {n}"))
                    .style(TextStyle::Callout)
                    .centered()
                    .frame(Some(120.0), Some(72.0))
                    .background(Semantic::Control)
                    .corner_radius(6.0),
            )
        })
        .collect();
    ScrollEdgeArrow::new(photos).fade(Semantic::SurfaceElevated)
}
```

Every number above is in `ScrollEdgeArrow.json` beside this card.
