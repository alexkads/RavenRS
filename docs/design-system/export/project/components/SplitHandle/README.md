# SplitHandle

A bar to drag that changes a size.

**Family:** Layout · **Rust:** `ravenrs::prelude::SplitHandle`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `inverted` | on/off | `true` |  |
| `label` | text | `` | Empty leaves it as the component has it. |

## Stories

- **Example**: 346 × 40 pt, settings `{"inverted":true,"label":""}`
- **Not inverted**: 346 × 40 pt, settings `{"inverted":false,"label":""}`

## Sizing

Hugs its content: 346 × 40 pt in a parent of 760 pt; in a parent of 320 pt it narrows to 264 pt; in a parent that stretches its children, as a form column, it takes the whole width; its height follows the width, as its content wraps.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover** on "Resize": fill border → ink-muted.
- **Focus** on "Resize": fill border → ink-muted; adds a rect (border 2.0 focus-ring).
- **Pressed** on "Resize": fill border → ink-muted.

## Behavior

- **Events**: `on_end`.
- **Keys** its first control handles once focused: `Escape`.
- **Accessibility**: role `Label` "Drag the bar, or focus it and use the arrow keys.".
- **Focus order** (Tab): Slider "Resize".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Label "Drag the bar, or focus it and use the arrow keys." | 341.0 × 22.0 | — | — | — | Inter 400 15.0px ink | — | 0.0 |
| Example | Slider "Resize" | 341.0 × 10.0 | — | — | — | — | — | — |

## API

Builder methods: `inverted`, `label`, `new`, `on_end`, `range`.

## Example

```rust
fn split_handle() -> impl View {
    let height = State::new(120.0_f32);
    // The size is yours to use; the bar only changes it.
    VStack::new((
        Text::new("Drag the bar, or focus it and use the arrow keys."),
        SplitHandle::new(Axis::Vertical, height.binding()).range(80.0, 300.0).inverted(),
    ))
    .spacing(8.0)
    .align(Align::Stretch)
}
```

Every number above is in `SplitHandle.json` beside this card.
