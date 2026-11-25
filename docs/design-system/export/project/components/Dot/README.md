# Dot

Small round mark of state, such as not saved.

**Family:** Display and feedback · **Rust:** `ravenrs::prelude::Dot`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `tone` | Neutral · Accent · Success · Warning · Danger | `Warning` |  |
| `label` | text | `Not saved` |  |

## Stories

- **Example**: 145 × 18 pt, settings `{"label":"Not saved","tone":"Warning"}`
- **Tone: Neutral**: 145 × 18 pt, settings `{"label":"Not saved","tone":"Neutral"}`
- **Tone: Accent**: 145 × 18 pt, settings `{"label":"Not saved","tone":"Accent"}`
- **Tone: Success**: 145 × 18 pt, settings `{"label":"Not saved","tone":"Success"}`
- **Tone: Danger**: 145 × 18 pt, settings `{"label":"Not saved","tone":"Danger"}`

## Sizing

Hugs its content: 145 × 18 pt in a parent of 760 pt.

## Behavior

- **Accessibility**: role `Image` "Not saved".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Image "Not saved" | 8.0 × 8.0 | radius-full | warning | — | — | — | — |
| Example | Label "Not saved" | 63.0 × 18.0 | — | — | — | Inter 400 13.0px ink | — | 0.0 |
| Example | Image "Online" | 10.0 × 10.0 | radius-full | success | — | — | — | — |
| Tone: Neutral | Image "Not saved" | 8.0 × 8.0 | radius-full | ink-muted | — | — | — | — |
| Tone: Accent | Image "Not saved" | 8.0 × 8.0 | radius-full | mix(accent, focus-ring, 35) | — | — | — | — |
| Tone: Success | Image "Not saved" | 8.0 × 8.0 | radius-full | success | — | — | — | — |
| Tone: Danger | Image "Not saved" | 8.0 × 8.0 | radius-full | danger | — | — | — | — |

## API

Builder methods: `color`, `label`, `new`, `size`, `tone`.

## Example

```rust
fn dot() -> impl View {
    HStack::new((
        Dot::new().tone(Tone::Warning).label("Not saved"),
        Text::new("Not saved").style(TextStyle::Callout),
        Dot::new().tone(Tone::Success).size(10.0).label("Online"),
        Text::new("Online").style(TextStyle::Callout),
    ))
    .spacing(8.0)
    .align(Align::Center)
}
```

Every number above is in `Dot.json` beside this card.
