# Spinner

Indeterminate activity indicator.

**Family:** Display and feedback · **Rust:** `ravenrs::prelude::Spinner`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `size` | number 8–96 by 1 | `20` | Its diameter, in points. |

## Stories

- **Example**: 20 × 20 pt, settings `{"size":20.0}`
- **Large**: 40 × 40 pt, settings `{"size":40.0}`

## Sizing

Hugs its content: 20 × 20 pt in a parent of 760 pt.

## Behavior

- **Accessibility**: role `ProgressIndicator` "Loading".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | ProgressIndicator "Loading" | 20.0 × 20.0 | — | — | — | — | — | — |
| Large | ProgressIndicator "Loading" | 40.0 × 40.0 | — | — | — | — | — | — |

## API

Builder methods: `color`, `new`, `size`.

## Example

```rust
fn spinner() -> impl View {
    Spinner::new()
}
```

Every number above is in `Spinner.json` beside this card.
