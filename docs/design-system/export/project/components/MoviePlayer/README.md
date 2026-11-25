# MoviePlayer

Plays a film of the app, drawn as vectors, with chapters, captions and the sound it computes.

**Family:** Application · **Rust:** `ravenrs::prelude::MoviePlayer`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `width` | number 320–760 by 10 | `640` |  |

## Stories

- **A film**: 640 × 589 pt, settings `{"width":640.0}`
- **Narrow**: 400 × 581 pt, settings `{"width":400.0}`

## Sizing

Hugs its content: 640 × 589 pt in a parent of 760 pt; in a parent of 320 pt it narrows to 638 pt.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story A film:

- **Hover** on "Play": fill bg-300 → mix(ink, bg-300, 6).
- **Focus** on "Play": adds a rect (border 2.0 focus-ring).
- **Pressed** on "Play": border {color:border,width:1.0} → {color:border,width:0.97}, fill bg-300 → mix(ink, bg-300, 12), rect [304.0,264.0,40.0,40.0] → [304.6,264.6,38.8,38.8].

## Behavior

- **Keys** its first control handles once focused: `ArrowRight`, `Enter`, `Space`.
- **Accessibility**: role `Image` "Film".
- **Focus order** (Tab): Button "Play" → Button "Play" → Button "From the start" → Slider "Position" → Button "1×" → Switch "Stop at each step" → Switch "Sound" → Switch "Narration" → Button "A square".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| A film | Image "Film" | 638.0 × 399.0 | 0.0 | on-accent | — | — | play 16.0px ink | 315.0 |
| A film | Button "Play" | 40.0 × 40.0 | radius-md | bg-300 | 1.0 border | — | play 16.0px ink | space-3 |
| A film | Button "Play" | 73.0 × 32.0 | radius-md | accent | — | Inter 600 13.0px on-accent | play 14.0px on-accent | space-3 |
| A film | Button "From the start" | 134.0 × 32.0 | radius-md | transparent | — | Inter 600 13.0px ink | rotate-ccw 14.0px ink | space-3 |
| A film | Slider "Position" | 290.0 × 24.0 | — | — | — | — | — | — |
| A film | Button "1×" | 39.0 × 32.0 | radius-md | transparent | — | Inter 600 13.0px ink | — | space-3 |
| A film | Label "0:00 / 0:06" | 70.0 × 16.0 | — | — | — | Inter 600 12.0px ink-muted | — | 0.0 |
| A film | Switch "Stop at each step" | 174.0 × 24.0 | — | — | — | Inter 400 15.0px ink | — | 50.0 |
| A film | Label "Chapters" | 640.0 × 18.0 | — | — | — | Inter 600 13.0px ink | — | 0.0 |
| A film | Button "A square" | 632.0 × 62.0 | radius-md | bg-300 | — | Inter 600 15.0px ink | play 18.0px ink-muted | 21.0 |
| Narrow | Image "Film" | 398.0 × 249.0 | 0.0 | on-accent | — | — | play 16.0px ink | 195.0 |
| Narrow | Slider "Position" | 120.0 × 24.0 | — | — | — | Inter 600 13.0px ink | — | 96.0 |
| Narrow | Label "0:00 / 0:06" | 44.0 × 32.0 | — | — | — | Inter 600 12.0px ink-muted | — | 0.0 |
| Narrow | Label "A square" | 65.0 × 22.0 | — | — | — | Inter 600 15.0px ink | — | 0.0 |
| Narrow | Label "It goes across the screen." | 159.0 × 18.0 | — | — | — | Inter 400 13.0px ink-muted | — | 0.0 |

## Example

```rust
fn movie_player() -> impl View {
    use ravenrs::core::Primitive;
    use ravenrs::movie::{FilmBuilder, MoviePlayer};
    // Real films come from `Recorder`, which runs the app from a script.
    // ravenrs-lint: allow(raw_color) the film is a recording, and its white page is part of the picture
    let mut film = FilmBuilder::new(Size::new(320.0, 200.0), Color::WHITE);
    for second in 0..=6_u64 {
        let square =
            // ravenrs-lint: allow(raw_color) the square is drawn into the film, not into the interface
            Quad::new(Rect::new(20.0 + second as f32 * 40.0, 80.0, 40.0, 40.0), Color::hex(0x4f46e5));
        film.frame(second * 1000, &[Primitive::Quad(square.radius(8.0))]);
    }
    film.chapter("A square", "It goes across the screen.", 0);
    film.finish(6000).map(|film| MoviePlayer::new(film).width(640.0)).ok()
}
```

Every number above is in `MoviePlayer.json` beside this card.
