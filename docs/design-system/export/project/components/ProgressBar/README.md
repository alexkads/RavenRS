# ProgressBar

How far a task is, with an optional label, percent and detail.

**Family:** Display and feedback · **Rust:** `ravenrs::prelude::ProgressBar`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `value` | number 0–1 by 0.01 | `0.4` |  |
| `tone` | Neutral · Accent · Success · Warning · Danger | `Accent` |  |
| `label` | text | `` |  |
| `percent` | on/off | `false` |  |
| `detail` | text | `` |  |
| `detail_inline` | on/off | `false` | The detail on the line of the label, before the percent. |
| `marker` | number 0–1 by 0.01 | `0` | A value of reference on the track, such as a limit; 0 for none. |
| `marker_label` | text | `` | The marker's name, read after the value. |

## Stories

- **Plain**: 728 × 6 pt, settings `{"detail":"","detail_inline":false,"label":"","marker":0.0,"marker_label":"","percent":false,"tone":"Accent","value":0.25}`
- **Success**: 728 × 6 pt, settings `{"detail":"","detail_inline":false,"label":"","marker":0.0,"marker_label":"","percent":false,"tone":"Success","value":0.9}`
- **With label**: 728 × 48 pt, settings `{"detail":"12 of 30 photos","detail_inline":false,"label":"Copying","marker":0.0,"marker_label":"","percent":true,"tone":"Accent","value":0.4}`
- **Detail inline**: 728 × 28 pt, settings `{"detail":"812 of 1,360 tests","detail_inline":true,"label":"Stage 3 of 5 · Tests","marker":0.0,"marker_label":"","percent":true,"tone":"Accent","value":0.6}`
- **With a marker**: 728 × 31 pt, settings `{"detail":"","detail_inline":false,"label":"Confidence","marker":0.85,"marker_label":"limit 0.85","percent":true,"tone":"Accent","value":0.89}`
- **All tones**: 6 × 78 pt, settings `{"detail":"","detail_inline":false,"label":"","marker":0.0,"marker_label":"","percent":false,"tone":"Accent","value":0.4}`

## Sizing

Fills the width of its parent: 288, 728, 1168 pt wide in parents of 320, 760, 1200 pt.

- **All tones**: Hugs its content: 6 × 78 pt in a parent of 760 pt; in a parent that stretches its children, as a form column, it takes the whole width.

## Behavior

- **Accessibility**: role `ProgressIndicator`.

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Plain | ProgressIndicator | 728.0 × 6.0 | radius-full | bg-300 | — | — | — | — |
| With label | ProgressIndicator "Copying" | 728.0 × 48.0 | — | — | — | Inter 400 13.0px ink | — | 0.0 |
| Detail inline | ProgressIndicator "Stage 3 of 5 · Tests" | 728.0 × 28.0 | — | — | — | Inter 400 13.0px ink | — | 0.0 |

## API

Builder methods: `color`, `detail`, `detail_inline`, `height`, `label`, `marker`, `marker_label`, `new`, `percent`, `tone`.

## Example

```rust
fn progress_bar() -> impl View {
    VStack::new((
        ProgressBar::new(0.25),
        ProgressBar::new(0.9).tone(Tone::Success),
        ProgressBar::new(0.4).label("Copying").percent().detail("12 of 30 photos").height(4.0),
    ))
    .spacing(16.0)
    .align(Align::Stretch)
}
```

Every number above is in `ProgressBar.json` beside this card.
