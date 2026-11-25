# MovieAssistant

Finds the part of a film that answers a question, by its chapters, and plays it.

**Family:** Application · **Rust:** `ravenrs::prelude::MovieAssistant`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `width` | number 320–760 by 10 | `640` |  |

## Stories

- **A library**: 640 × 704 pt, settings `{"width":640.0}`

## Sizing

Hugs its content: 640 × 704 pt in a parent of 760 pt; in a parent of 320 pt it narrows to 638 pt.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story A library:

- **Hover** on "Ask how to do something": border {color:control-border,width:1.0} → {color:mix(ink, control-border, 25),width:1.0}.
- **Focus** on "Ask how to do something": border {color:control-border,width:1.0} → {color:accent,width:1.5}; adds a rect (accent); adds a rect (border 2.0 focus-ring); color ink-muted → on-accent.
- **Pressed** on "Ask how to do something": border {color:control-border,width:1.0} → {color:accent,width:1.5}; adds a rect (accent); color ink-muted → on-accent.

## Behavior

- **Keys** its first control handles once focused: `Space`, `Escape`.
- **Accessibility**: role `TextInput` "Ask how to do something".
- **Focus order** (Tab): TextInput "Ask how to do something" → Button "Play" → Button "Play" → Button "From the start" → Slider "Position" → Button "1×" → Switch "Stop at each step" → Switch "Sound" → Switch "Narration" → Button "Move a square" → Button "Save your work".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| A library | TextInput "Ask how to do something" | 640.0 × 40.0 | radius-md | bg-300 | 1.0 control-border | Inter 400 15.0px ink-muted | search 16.0px ink-muted | space-3 |
| A library | Image "Film" | 638.0 × 399.0 | 0.0 | on-accent | — | — | play 16.0px ink | 315.0 |
| A library | Button "Play" | 40.0 × 40.0 | radius-md | bg-300 | 1.0 border | — | play 16.0px ink | space-3 |
| A library | Button "Play" | 73.0 × 32.0 | radius-md | accent | — | Inter 600 13.0px on-accent | play 14.0px on-accent | space-3 |
| A library | Button "From the start" | 134.0 × 32.0 | radius-md | transparent | — | Inter 600 13.0px ink | rotate-ccw 14.0px ink | space-3 |
| A library | Slider "Position" | 290.0 × 24.0 | — | — | — | — | — | — |
| A library | Button "1×" | 39.0 × 32.0 | radius-md | transparent | — | Inter 600 13.0px ink | — | space-3 |
| A library | Label "0:00 / 0:09" | 70.0 × 16.0 | — | — | — | Inter 600 12.0px ink-muted | — | 0.0 |
| A library | Switch "Stop at each step" | 174.0 × 24.0 | — | — | — | Inter 400 15.0px ink | — | 50.0 |
| A library | Label "Chapters" | 640.0 × 18.0 | — | — | — | Inter 600 13.0px ink | — | 0.0 |
| A library | Button "Move a square" | 632.0 × 62.0 | radius-md | bg-300 | — | Inter 600 15.0px ink | play 18.0px ink-muted | 21.0 |
| A library | Button "Save your work" | 632.0 × 62.0 | radius-md | transparent | — | Inter 600 15.0px ink | play 18.0px ink-muted | 21.0 |

## Example

```rust
fn movie_assistant() -> impl View {
    use ravenrs::core::Primitive;
    use ravenrs::movie::{FilmBuilder, MovieAssistant, MovieLibrary};
    // ravenrs-lint: allow(raw_color) the film is a recording, and its white page is part of the picture
    let mut film = FilmBuilder::new(Size::new(320.0, 200.0), Color::WHITE);
    for second in 0..=9_u64 {
        let square =
            // ravenrs-lint: allow(raw_color) the square is drawn into the film, not into the interface
            Quad::new(Rect::new(20.0 + second as f32 * 25.0, 80.0, 40.0, 40.0), Color::hex(0x4f46e5));
        film.frame(second * 1000, &[Primitive::Quad(square.radius(8.0))]);
    }
    film.chapter("Move a square", "How it goes across the screen.", 0);
    film.chapter("Save your work", "Keep a copy of what you did.", 5000);
    // Accents and case do not matter, and near words count: try "saving".
    film.finish(9000)
        .map(|film| MovieAssistant::new(MovieLibrary::new().film("Manual", film)).width(640.0))
        .ok()
}
```

Every number above is in `MovieAssistant.json` beside this card.
