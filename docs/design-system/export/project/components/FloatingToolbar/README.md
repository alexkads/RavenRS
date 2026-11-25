# FloatingToolbar

Translucent row of commands over its content, with a handle that moves it.

**Family:** Actions and inputs · **Rust:** `ravenrs::prelude::FloatingToolbar`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `label` | text | `Zoom` |  |
| `separator` | on/off | `true` |  |
| `fill` | on/off | `false` |  |

## Stories

- **Example**: 728 × 200 pt, settings `{"fill":false,"label":"Zoom","separator":true}`

## Sizing

Fills the width of its parent: 288, 728, 1168 pt wide in parents of 320, 760, 1200 pt.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover** on "Move: Zoom": color ink-muted → ink.
- **Focus** on "Move: Zoom": adds a rect (border 2.0 focus-ring).
- **Pressed** on "Move: Zoom": color ink-muted → ink.

## Behavior

- **Keys** its first control handles once focused: `ArrowDown`, `ArrowUp`, `ArrowLeft`, `ArrowRight`.
- **Accessibility**: role `Toolbar` "Zoom".
- **Focus order** (Tab): Button "Move: Zoom" → Button "Fit" → Button "Fill" → Button "Zoom out" → Button "Zoom level: 100%" → Button "Zoom in".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Toolbar "Zoom" | 277.0 × 40.0 | radius-lg | bg-200@92 | 1.0 border | Inter 600 13.0px ink | ellipsis-vertical 14.0px ink-muted | 5.0 |
| Example | Button "Move: Zoom" | 16.0 × 32.0 | — | — | — | — | ellipsis-vertical 14.0px ink-muted | 1.0 |
| Example | Button "Fit" | 40.0 × 32.0 | radius-md | transparent | — | Inter 600 13.0px ink | — | space-3 |
| Example | Button "Zoom out" | 32.0 × 32.0 | radius-md | transparent | — | — | minus 14.0px ink | 9.0 |
| Example | Button "Zoom level: 100%" | 82.0 × 32.0 | radius-md | bg-300 | 1.0 border | Inter 600 13.0px ink | chevron-down 14.0px ink | space-3 |
| Example | Button "Zoom in" | 32.0 × 32.0 | radius-md | transparent | — | — | plus 14.0px ink | 9.0 |

## API

Builder methods: `action`, `fill`, `label`, `margin`, `movable`, `new`, `place`, `separator`, `view`.

## Example

```rust
fn floating_toolbar() -> impl View {
    let (place, level) = (State::new(None::<Point>), State::new(100.0_f32));
    FloatingToolbar::new(
        Spacer::new().frame(None, Some(200.0)).background(Semantic::Control).corner_radius(8.0),
    )
    .label("Zoom")
    .movable(place.binding())
    .action(ToolbarAction::new("Fit"))
    .action(ToolbarAction::new("Fill"))
    .separator()
    .action(ToolbarAction::new("Zoom out").icon(IconName::Minus).icon_only())
    .view(ValueSlider::new(level.binding()).range(10.0, 400.0).step(10.0).unit("%").label("Zoom level"))
    .action(ToolbarAction::new("Zoom in").icon(IconName::Plus).icon_only())
}
```

Every number above is in `FloatingToolbar.json` beside this card.
