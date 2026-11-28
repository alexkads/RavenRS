# RadioGroup

One choice among several, shown as radio buttons.

**Family:** Actions and inputs · **Rust:** `ravenrs::prelude::RadioGroup`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `label` | text | `Plan` |  |
| `plan` | number 0–5 by 1 | `1` | How the example starts. |

## Stories

- **Example**: 101 × 86 pt, settings `{"label":"Plan","plan":1.0}`

## Sizing

Hugs its content: 101 × 86 pt in a parent of 760 pt; in a parent that stretches its children, as a form column, it takes the whole width.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover** on "Starter": border {color:control-border,width:1.5} → {color:ink,width:1.5}.
- **Focus** on "Starter": adds a rect (border 2.0 focus-ring).
- **Pressed** on "Starter": border {color:control-border,width:1.5} → {color:ink,width:1.5}.

## Behavior

- **Keys** its first control handles once focused: `ArrowDown`, `ArrowUp`, `ArrowLeft`, `ArrowRight`, `Enter`, `Space`, `Home`, `End`.
- **Accessibility**: role `RadioGroup` "Plan".
- **Focus order** (Tab): RadioButton "Starter" → RadioButton "Team" → RadioButton "Enterprise".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | RadioButton "Starter" | 77.0 × 22.0 | — | — | — | Inter 400 15.0px ink | — | 28.0 |

## API

Builder methods: `disable_option`, `disabled`, `label`, `new`, `read_only`.

## Example

```rust
fn radio_group() -> impl View {
    let plan = State::new(1);
    RadioGroup::new(plan.binding(), ["Starter", "Team", "Enterprise"]).label("Plan")
}
```

Every number above is in `RadioGroup.json` beside this card.
