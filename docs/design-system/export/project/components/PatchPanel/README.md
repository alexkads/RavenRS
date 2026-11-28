# PatchPanel

Jacks joined by cables that hang and swing: plug, refuse, take a plug elsewhere, pairs of cords, a front that turns around, all of it with the keys too.

**Family:** Data · **Rust:** `ravenrs::prelude::PatchPanel`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `label` | text | `` | Empty leaves it as the component has it. |
| `retract` | on/off | `false` |  |
| `thickness` | number 0–24 by 0.1 | `6` |  |

## Stories

- **Example**: 400 × 280 pt, settings `{"label":"","retract":false,"thickness":6.0}`
- **Cables retract**: 400 × 280 pt, settings `{"label":"","retract":true,"thickness":6.0}`

## Sizing

Hugs its content: 400 × 280 pt in a parent of 760 pt; in a parent of 320 pt it narrows to 331 pt; in a parent that stretches its children, as a form column, it takes the whole width.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover**: "Left out" does not change.
- **Focus** on "Left out": adds a rect (border 2.0 focus-ring); fill accent → #aebaf76b.
- **Pressed** on "Left out": border {color:accent,width:3.0} → {color:#3ccf8e,width:3.0}; border {color:accent,width:3.0} → {color:mix(control-border, ink-muted, 80),width:3.0}; border {color:#e08a2e,width:3.0} → {color:mix(control-border, ink-muted, 80),width:3.0}; border {color:accent,width:3.0} → {color:mix(control-border, ink-muted, 80),width:3.0}.

## Behavior

- **Events**: `on_connect`, `on_disconnect`, `on_join`.
- **Keys** its first control handles once focused: `Enter`, `Space`, `Escape`.
- **Accessibility**: role `Group` "Patch panel".
- **Focus order** (Tab): Button "Left out" → Button "Right out" → Button "Tap" → Button "Input 1" → Button "Input 2" → Button "Pedal".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Label "Left out" | 48.0 × 16.0 | — | — | — | Inter 600 12.0px #c7cedb | — | 1.0 |
| Example | Button "Left out" | 22.0 × 22.0 | radius-full | mix(border, control-border, 92) | 3.0 accent | — | — | — |
| Example | Label "Right out" | 56.0 × 16.0 | — | — | — | Inter 600 12.0px #c7cedb | — | 0.0 |
| Example | Button "Tap" | 22.0 × 22.0 | 5.0 | mix(border, control-border, 92) | 3.0 #e08a2e | — | — | — |
| Example | Button "Input 1" | 22.0 × 22.0 | radius-full | bg-000 | 3.0 accent | — | — | — |
| Example | Button "Pedal" | 22.0 × 22.0 | 5.0 | bg-000 | 3.0 #e08a2e | — | — | — |

## API

Builder methods: `accept`, `background`, `color_of`, `fit`, `flipped`, `front`, `label`, `new`, `on_connect`, `on_disconnect`, `on_join`, `pair`, `retract`, `size`, `texture_of`, `thickness`.

## Example

```rust
fn patch_panel() -> impl View {
    let links = State::new(vec![PatchLink::new("left", "in-1")]);
    PatchPanel::new(
        vec![
            Jack::output("left", "Left out").signal("audio").at(60.0, 50.0),
            Jack::output("right", "Right out").signal("audio").at(180.0, 50.0),
            Jack::output("tap", "Tap").signal("control").at(320.0, 50.0),
            Jack::input("in-1", "Input 1").signal("audio").at(60.0, 210.0),
            Jack::input("in-2", "Input 2").signal("audio").at(180.0, 210.0),
            Jack::input("pedal", "Pedal").signal("control").at(320.0, 210.0),
        ],
        links.binding(),
    )
    .size(400.0, 280.0)
}
```

Every number above is in `PatchPanel.json` beside this card.
