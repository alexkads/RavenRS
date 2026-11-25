# MultiProgress

Several progress bars stacked, for work with more than one part.

**Family:** Display and feedback · **Rust:** `ravenrs::prelude::MultiProgress`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `label` | text | `Import` |  |

## Stories

- **Example**: 728 × 108 pt, settings `{"label":"Import"}`

## Sizing

Fills the width of its parent: 288, 728, 1168 pt wide in parents of 320, 760, 1200 pt.

## Behavior

- **Accessibility**: role `Group` "Import".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | ProgressIndicator "Local copy" | 728.0 × 48.0 | — | — | — | Inter 400 13.0px ink | — | 0.0 |

## API

Builder methods: `bar`, `label`, `meter`, `new`.

## Example

```rust
fn multi_progress() -> impl View {
    MultiProgress::new().label("Import").meter("Local copy", 84, 120).meter("Previews", 12, 84)
}
```

Every number above is in `MultiProgress.json` beside this card.
