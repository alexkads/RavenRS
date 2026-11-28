# DateTile

A day in a small block: the day over the month, and the whole date for screen readers.

**Family:** Display and feedback · **Rust:** `ravenrs::prelude::DateTile`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `tone` | Neutral · Accent · Success · Warning · Danger | `Neutral` |  |
| `size` | number 32–128 by 4 | `56` |  |
| `outlined` | on/off | `false` | Only the outline, over a colored band. |

## Stories

- **Example**: 208 × 72 pt, settings `{"outlined":false,"size":56.0,"tone":"Neutral"}`
- **Tones**: 208 × 72 pt, settings `{"outlined":false,"size":56.0,"tone":"Accent"}`
- **Tone: Success**: 208 × 72 pt, settings `{"outlined":false,"size":56.0,"tone":"Success"}`
- **Tone: Warning**: 208 × 72 pt, settings `{"outlined":false,"size":56.0,"tone":"Warning"}`
- **Tone: Danger**: 208 × 72 pt, settings `{"outlined":false,"size":56.0,"tone":"Danger"}`
- **Sizes**: 224 × 72 pt, settings `{"outlined":false,"size":72.0,"tone":"Neutral"}`
- **Outlined**: 208 × 72 pt, settings `{"outlined":true,"size":56.0,"tone":"Neutral"}`

## Sizing

Hugs its content: 208 × 72 pt in a parent of 760 pt.

## Behavior

- **Accessibility**: role `Image` "Saturday, November 14, 2026".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Image "Saturday, November 14, 2026" | 56.0 × 56.0 | radius-lg | bg-300 | — | Inter 600 17.0px ink | — | 19.0 |
| Example | Image "Sunday, November 15, 2026" | 56.0 × 56.0 | radius-lg | accent | — | Inter 600 17.0px on-accent | — | 19.0 |
| Example | Image "Saturday, November 21, 2026" | 72.0 × 72.0 | radius-lg | bg-300 | — | Inter 600 22.0px ink | — | space-6 |
| Tone: Success | Image "Saturday, November 14, 2026" | 56.0 × 56.0 | radius-lg | success | — | Inter 600 17.0px on-accent | — | 19.0 |
| Tone: Warning | Image "Saturday, November 14, 2026" | 56.0 × 56.0 | radius-lg | warning | — | Inter 600 17.0px on-accent | — | 19.0 |
| Tone: Danger | Image "Saturday, November 14, 2026" | 56.0 × 56.0 | radius-lg | danger | — | Inter 600 17.0px on-accent | — | 19.0 |
| Outlined | Image "Saturday, November 14, 2026" | 56.0 × 56.0 | radius-lg | transparent | 1.0 ink | Inter 600 17.0px ink | — | 19.0 |

## API

Builder methods: `new`, `outlined`, `size`, `tone`.

Every number above is in `DateTile.json` beside this card.
