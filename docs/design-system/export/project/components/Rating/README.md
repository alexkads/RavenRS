# Rating

Stars for a score, to read or to give with the pointer and the keyboard.

**Family:** Display and feedback · **Rust:** `ravenrs::prelude::Rating`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `label` | text | `Photo rating` |  |
| `clearable` | on/off | `true` |  |
| `max` | number 0–20 by 1 | `5` |  |
| `size` | number 0–72 by 1 | `18` |  |
| `tone` | Neutral · Accent · Success · Warning · Danger | `Warning` |  |
| `score` | number 0–12 by 1 | `3` | How the example starts. |

## Stories

- **Example**: 90 × 44 pt, settings `{"clearable":true,"label":"Photo rating","max":5.0,"score":3.0,"size":18.0,"tone":"Warning"}`
- **Ten stars**: 180 × 44 pt, settings `{"clearable":true,"label":"Photo rating","max":10.0,"score":7.0,"size":18.0,"tone":"Warning"}`
- **Tone: Neutral**: 90 × 44 pt, settings `{"clearable":true,"label":"Photo rating","max":5.0,"score":3.0,"size":18.0,"tone":"Neutral"}`
- **Tone: Accent**: 90 × 44 pt, settings `{"clearable":true,"label":"Photo rating","max":5.0,"score":3.0,"size":18.0,"tone":"Accent"}`
- **Tone: Success**: 90 × 44 pt, settings `{"clearable":true,"label":"Photo rating","max":5.0,"score":3.0,"size":18.0,"tone":"Success"}`
- **Tone: Danger**: 90 × 44 pt, settings `{"clearable":true,"label":"Photo rating","max":5.0,"score":3.0,"size":18.0,"tone":"Danger"}`

## Sizing

Hugs its content: 90 × 44 pt in a parent of 760 pt.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover**: "Photo rating" does not change.
- **Focus** on "Photo rating": fill warning → ink-muted@35; fill warning → ink-muted@35; adds a rect (border 2.0 focus-ring); fill warning → ink-muted@35.
- **Pressed**: "Photo rating" does not change.

## Behavior

- **Keys** its first control handles once focused: `ArrowDown`, `ArrowUp`, `ArrowLeft`, `ArrowRight`, `Escape`, `Home`, `End`.
- **Accessibility**: role `Slider` "Photo rating".
- **Focus order** (Tab): Slider "Photo rating".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Slider "Photo rating" | 90.0 × 18.0 | — | — | — | — | — | — |
| Example | Image "Rating" | 70.0 × 14.0 | — | — | — | — | — | — |

## API

Builder methods: `clearable`, `color`, `label`, `max`, `new`, `read_only`, `size`, `tone`.

## Example

```rust
fn rating() -> impl View {
    let score = State::new(3u32);
    VStack::new((
        Rating::new(score.binding()).label("Photo rating").clearable(),
        Rating::read_only(4).size(14.0),
    ))
    .spacing(12.0)
}
```

Every number above is in `Rating.json` beside this card.
