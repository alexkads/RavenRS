# RubberBand

A rectangle drawn with a drag, in the units of the content, selected on release.

**Family:** Media · **Rust:** `ravenrs::prelude::RubberBand`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `disabled` | on/off | `false` |  |
| `keep` | on/off | `true` |  |
| `label` | text | `` | Empty leaves it as the component has it. |
| `chosen` | text | `Drag over the box` | How the example starts. |

## Stories

- **Example**: 320 × 160 pt, settings `{"chosen":"Drag over the box","disabled":false,"keep":true,"label":""}`
- **Disabled**: 320 × 160 pt, settings `{"chosen":"Drag over the box","disabled":true,"keep":true,"label":""}`

## Sizing

Hugs its content: 320 × 160 pt in a parent of 760 pt; in a parent of 320 pt it narrows to 128 pt.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover**: "Selection area" does not change.
- **Focus** on "Selection area": adds a rect (border 2.0 focus-ring).
- **Pressed**: "Selection area" does not change.

## Behavior

- **Events**: `on_select`.
- **Keys** its first control handles once focused: `ArrowDown`, `ArrowUp`, `ArrowLeft`, `ArrowRight`.
- **Accessibility**: role `Label` "Drag over the box".
- **Focus order** (Tab): Group "Selection area".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Label "Drag over the box" | 127.0 × 22.0 | — | — | — | Inter 400 15.0px ink | — | 0.0 |

## API

Builder methods: `disabled`, `grow`, `keep`, `label`, `new`, `on_select`, `size`, `space`, `stage`.

## Example

```rust
fn rubber_band() -> impl View {
    let selection = State::new(None::<Rect>);
    let chosen = State::new(String::from("Drag over the box"));
    let told = chosen.clone();
    VStack::new((
        Text::bound(&chosen, Clone::clone),
        RubberBand::new(selection.binding())
            .on_select(move |r| told.set(format!("{:.0} × {:.0}", r.width(), r.height())))
            .size(320.0, 130.0),
    ))
    .spacing(8.0)
}
```

Every number above is in `RubberBand.json` beside this card.
