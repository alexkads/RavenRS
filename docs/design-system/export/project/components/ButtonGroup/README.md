# ButtonGroup

Buttons glued side by side, as one control.

**Family:** Actions and inputs · **Rust:** `ravenrs::prelude::ButtonGroup`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `label` | text | `Period` |  |
| `period` | number 0–5 by 1 | `0` | How the example starts. |

## Stories

- **Example**: 225 × 40 pt, settings `{"label":"Period","period":0.0}`

## Sizing

Hugs its content: 225 × 40 pt in a parent of 760 pt.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover** on "Day": fill accent → accent-hover.
- **Focus** on "Day": adds a rect (border 2.0 focus-ring).
- **Pressed** on "Day": fill accent → accent-pressed.

## Behavior

- **Accessibility**: role `Group` "Period".
- **Focus order** (Tab): Button "Day" → Button "Week" → Button "Month".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Button "Day" | 65.0 × 40.0 | radius-md | accent | — | Inter 600 15.0px on-accent | — | 18.0 |
| Example | Button "Week" | 77.0 × 40.0 | — | — | — | Inter 600 15.0px ink | — | 18.0 |

## API

Builder methods: `label`, `new`.

## Example

```rust
fn button_group() -> impl View {
    let period = State::new(0);
    view_fn(move || {
        let chosen = period.get();
        let item = |index, label: &str| {
            let period = period.clone();
            let style = if chosen == index { ButtonStyle::Primary } else { ButtonStyle::Secondary };
            Button::new(label).style(style).on_click(move || period.set(index))
        };
        ButtonGroup::new([item(0, "Day"), item(1, "Week"), item(2, "Month")]).label("Period")
    })
}
```

Every number above is in `ButtonGroup.json` beside this card.
