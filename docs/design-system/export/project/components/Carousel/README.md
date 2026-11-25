# Carousel

Slides with buttons, position dots and optional autoplay.

**Family:** Media · **Rust:** `ravenrs::prelude::Carousel`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `height` | number 0–880 by 1 | `220` |  |

## Stories

- **Example**: 728 × 264 pt, settings `{"height":220.0}`
- **Short**: 728 × 184 pt, settings `{"height":140.0}`

## Sizing

Fills the width of its parent: 288, 728, 1168 pt wide in parents of 320, 760, 1200 pt.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover** on "Previous slide": fill bg-300 → mix(ink, bg-300, 6).
- **Focus** on "Previous slide": adds a rect (border 2.0 focus-ring).
- **Pressed** on "Previous slide": border {color:border,width:1.0} → {color:border,width:0.97}, fill bg-300 → mix(ink, bg-300, 12), rect [0.0,232.0,32.0,32.0] → [0.48,232.48,31.04,31.04].

## Behavior

- **Keys** its first control handles once focused: `Enter`, `Space`.
- **Accessibility**: role `Button` "Previous slide".
- **Focus order** (Tab): Button "Previous slide" → Button "Next slide".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Button "Previous slide" | 32.0 × 32.0 | radius-md | bg-300 | 1.0 border | — | chevron-left 14.0px ink | 9.0 |
| Example | Button "Slide 1 of 3" | 20.0 × 8.0 | radius-full | accent | — | — | — | — |
| Example | Button "Slide 2 of 3" | 8.0 × 8.0 | radius-full | ink-muted@40 | — | — | — | — |
| Example | Button "Next slide" | 32.0 × 32.0 | radius-md | bg-300 | 1.0 border | — | chevron-right 14.0px ink | 9.0 |

## API

Builder methods: `autoplay`, `height`, `new`.

## Example

```rust
fn carousel() -> impl View {
    let slides: Vec<AnyView> = [1.0, 0.6, 0.2]
        .into_iter()
        .map(|hue| Box::new(Image::new(picture(hue)).stretch().height(220.0)) as AnyView)
        .collect();
    Carousel::new(slides).height(220.0)
}
```

Every number above is in `Carousel.json` beside this card.
