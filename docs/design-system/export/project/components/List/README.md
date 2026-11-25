# List

Bordered list with icons, titles and trailing text.

**Family:** Data · **Rust:** `ravenrs::prelude::List`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `items` | number 1–6 by 1 | `2` |  |
| `subtitles` | on/off | `true` | A second line under each title. |
| `compact` | on/off | `false` | Rows of 48, the subtitle beside the title. |
| `tones` | on/off | `false` | Each icon in the color of its state. |
| `action` | on/off | `false` | A button at the end of the first row. |

## Stories

- **Example**: 258 × 133 pt, settings `{"action":false,"compact":false,"items":2.0,"subtitles":true,"tones":false}`
- **One line each**: 227 × 292 pt, settings `{"action":false,"compact":false,"items":5.0,"subtitles":false,"tones":false}`
- **Compact**: 181 × 246 pt, settings `{"action":false,"compact":true,"items":5.0,"subtitles":false,"tones":true}`
- **Compact queue**: 319 × 99 pt, settings `{"action":true,"compact":true,"items":2.0,"subtitles":true,"tones":true}`
- **With an avatar**: 257 × 70 pt, settings `{"action":false,"compact":false,"items":2.0,"subtitles":true,"tones":false}`

## Sizing

Hugs its content: 258 × 133 pt in a parent of 760 pt; in a parent that stretches its children, as a form column, it takes the whole width.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover** on "Checkout": fill transparent → ink@4.
- **Focus** on "Checkout": adds a rect (border 2.0 focus-ring).
- **Pressed** on "Checkout": fill transparent → ink@4.

## Behavior

- **Accessibility**: role `List`.
- **Focus order** (Tab): Button "Checkout" → Button "Lint".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Button "Checkout" | 250.0 × 62.0 | radius-md | transparent | — | Inter 600 15.0px ink | circle-check 18.0px ink-muted | 21.0 |
| One line each | Button "Checkout" | 219.0 × 56.0 | radius-md | transparent | — | Inter 600 15.0px ink | circle-check 18.0px ink-muted | 21.0 |
| One line each | Button "Tests" | 219.0 × 56.0 | radius-md | transparent | — | Inter 600 15.0px ink | loader-circle 18.0px ink-muted | 21.0 |
| One line each | Button "Image and push" | 219.0 × 56.0 | radius-md | transparent | — | Inter 600 15.0px ink | package 18.0px ink-muted | 21.0 |
| One line each | Button "Status on the PR" | 219.0 × 56.0 | radius-md | transparent | — | Inter 600 15.0px ink | git-branch 18.0px ink-muted | 21.0 |
| Compact | Button "Checkout" | 179.0 × 48.0 | 0.0 | transparent | — | Inter 600 15.0px ink | circle-check 16.0px success | space-4 |
| Compact | Button "Tests" | 179.0 × 48.0 | 0.0 | transparent | — | Inter 600 15.0px ink | loader-circle 16.0px mix(accent, focus-ring, 35) | space-4 |
| Compact | Button "Image and push" | 179.0 × 48.0 | 0.0 | transparent | — | Inter 600 15.0px ink | package 16.0px ink-muted | space-4 |
| Compact | Button "Status on the PR" | 179.0 × 48.0 | 0.0 | transparent | — | Inter 600 15.0px ink | git-branch 16.0px ink-muted | space-4 |
| Compact queue | Button "Run again" | 87.0 × 32.0 | radius-md | bg-300 | 1.0 border | Inter 600 13.0px ink | — | space-3 |
| With an avatar | Image "Ana Lima" | 36.0 × 36.0 | radius-full | focus-ring@28 | 1.0 focus-ring@50 | Inter 600 14.0px mix(ink, focus-ring, 50) | — | 9.0 |

## API

Builder methods: `compact`, `new`.

## Example

```rust
fn list() -> impl View {
    List::new((
        ListItem::new("Ana Lima").subtitle("ana@northwind.dev").avatar("Ana Lima").trailing("Owner"),
        ListItem::new("Report.pdf").subtitle("2.4 MB").icon(IconName::FileText).on_click(|| {}),
    ))
}
```

Every number above is in `List.json` beside this card.
