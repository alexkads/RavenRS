# SegmentedControl

Picks one of a few options laid side by side on a track.

**Family:** Actions and inputs · **Rust:** `ravenrs::prelude::SegmentedControl`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `label` | text | `Period` |  |
| `options` | text | `Day, Week, Month` | The options, separated by commas. |
| `selected` | number 0–5 by 1 | `1` |  |

## Stories

- **Three options**: 196 × 36 pt, settings `{"label":"Period","options":"Day, Week, Month","selected":1.0}`
- **Two options**: 125 × 36 pt, settings `{"label":"View","options":"List, Board","selected":0.0}`

## Sizing

Hugs its content: 196 × 36 pt in a parent of 760 pt.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Three options:

- **Hover** on "Day": color ink-muted → ink.
- **Focus** on "Day": adds a rect (border 2.0 focus-ring).
- **Pressed** on "Day": color ink-muted → ink.

## Behavior

- **Keys** its first control handles once focused: `ArrowDown`, `ArrowUp`, `ArrowLeft`, `ArrowRight`, `Enter`, `Space`, `Home`, `End`.
- **Accessibility**: role `RadioGroup` "Period".
- **Focus order** (Tab): RadioButton "Day" → RadioButton "Week" → RadioButton "Month".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Three options | RadioButton "Day" | 53.0 × 30.0 | — | — | — | Inter 600 13.0px ink-muted | — | 14.0 |
| Three options | RadioButton "Week" | 64.0 × 30.0 | 7.0 | bg-200 | 1.0 border | Inter 600 13.0px ink | — | 14.0 |

## API

Builder methods: `label`, `new`, `with_icons`.

## Example

```rust
fn segmented_control() -> impl View {
    let period = State::new(1);
    SegmentedControl::new(period.binding(), ["Day", "Week", "Month"]).label("Period")
}
```

Every number above is in `SegmentedControl.json` beside this card.
