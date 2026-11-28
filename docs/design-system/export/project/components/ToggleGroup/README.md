# ToggleGroup

Row of toggle buttons where one or several can be on.

**Family:** Actions and inputs · **Rust:** `ravenrs::prelude::ToggleGroup`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `align` | number 0–5 by 1 | `0` | How the example starts. |

## Stories

- **Example**: 110 × 34 pt, settings `{"align":0.0}`

## Sizing

Hugs its content: 110 × 34 pt in a parent of 760 pt.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover** on "align center": fill transparent → ink@6; color ink-muted → ink.
- **Focus** on "align left": adds a rect (border 2.0 focus-ring).
- **Pressed** on "align center": fill transparent → ink@6; color ink-muted → ink.

## Behavior

- **Keys** its first control handles once focused: `ArrowDown`, `ArrowUp`, `ArrowLeft`, `ArrowRight`, `End`.
- **Accessibility**: role `Toolbar`.
- **Focus order** (Tab): Button "align left" → Button "align center" → Button "align right".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Toolbar | 110.0 × 34.0 | — | — | — | — | align-left 16.0px ink | 9.0 |
| Example | Button "align left" | 34.0 × 34.0 | 8.0 | bg-300 | 1.0 border | — | align-left 16.0px ink | 9.0 |
| Example | Button "align center" | 34.0 × 34.0 | 8.0 | transparent | — | — | align-center 16.0px ink-muted | 9.0 |
| Example | Button "align right" | 34.0 × 34.0 | 8.0 | transparent | — | — | align-right 16.0px ink-muted | 9.0 |

## API

Builder methods: `multiple`, `single`.

## Example

```rust
fn toggle_group() -> impl View {
    let align = State::new(0);
    ToggleGroup::single(
        align.binding(),
        [
            (Some(IconName::AlignLeft), ""),
            (Some(IconName::AlignCenter), ""),
            (Some(IconName::AlignRight), ""),
        ],
    )
}
```

Every number above is in `ToggleGroup.json` beside this card.
