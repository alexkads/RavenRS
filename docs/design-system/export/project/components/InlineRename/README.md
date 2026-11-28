# InlineRename

A name that turns into a field by a double click or F2.

**Family:** Actions and inputs · **Rust:** `ravenrs::prelude::InlineRename`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `label` | text | `Preset name` |  |
| `width` | number 0–880 by 1 | `220` |  |
| `disabled` | on/off | `false` |  |
| `placeholder` | text | `` | Empty leaves it as the component has it. |
| `size` | Small · Medium · Large · Mini | `Small` |  |
| `name` | text | `Warm evening` | How the example starts. |

## Stories

- **Example**: 220 × 52 pt, settings `{"disabled":false,"label":"Preset name","name":"Warm evening","placeholder":"","size":"Small","width":220.0}`
- **Disabled**: 220 × 52 pt, settings `{"disabled":true,"label":"Preset name","name":"Warm evening","placeholder":"","size":"Small","width":220.0}`
- **Size: Medium**: 220 × 60 pt, settings `{"disabled":false,"label":"Preset name","name":"Warm evening","placeholder":"","size":"Medium","width":220.0}`
- **Size: Large**: 220 × 68 pt, settings `{"disabled":false,"label":"Preset name","name":"Warm evening","placeholder":"","size":"Large","width":220.0}`
- **Size: Mini**: 220 × 42 pt, settings `{"disabled":false,"label":"Preset name","name":"Warm evening","placeholder":"","size":"Mini","width":220.0}`

## Sizing

Hugs its content: 220 × 52 pt in a parent of 760 pt.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover** on "Warm evening": fill transparent → ink@5.
- **Focus** on "Warm evening": adds a rect (border 2.0 focus-ring).
- **Pressed** on "Warm evening": fill transparent → ink@5.

## Behavior

- **Events**: `on_click`, `on_rename`.
- **Keys** its first control handles once focused: `Enter`, `Space`.
- **Accessibility**: role `Button` "Warm evening".
- **Focus order** (Tab): Button "Warm evening".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Button "Warm evening" | 220.0 × 32.0 | radius-sm | transparent | — | Inter 400 13.0px ink | — | space-2 |
| Example | Label "Double click it, or press F2 on it." | 199.0 × 16.0 | — | — | — | Inter 600 12.0px ink-muted | — | 0.0 |
| Disabled | Label "Warm evening" | 220.0 × 32.0 | radius-sm | transparent | — | Inter 400 13.0px ink | — | space-2 |
| Size: Medium | Button "Warm evening" | 220.0 × 40.0 | radius-sm | transparent | — | Inter 400 15.0px ink | — | space-2 |
| Size: Large | Button "Warm evening" | 220.0 × 48.0 | radius-sm | transparent | — | Inter 400 15.0px ink | — | space-2 |
| Size: Mini | Button "Warm evening" | 220.0 × 22.0 | radius-sm | transparent | — | Inter 600 12.0px ink | — | space-2 |

## API

Builder methods: `disabled`, `editing`, `label`, `new`, `on_click`, `on_rename`, `placeholder`, `size`, `width`.

## Example

```rust
fn inline_rename() -> impl View {
    let name = State::new(String::from("Warm evening"));
    VStack::new((
        InlineRename::new(name.binding()).label("Preset name").width(220.0),
        Text::new("Double click it, or press F2 on it.")
            .style(TextStyle::Caption)
            .foreground(Semantic::InkMuted),
    ))
    .spacing(4.0)
}
```

Every number above is in `InlineRename.json` beside this card.
