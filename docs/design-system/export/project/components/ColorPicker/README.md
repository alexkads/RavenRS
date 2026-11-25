# ColorPicker

A color from a palette, a shade square or typed.

**Family:** Actions and inputs · **Rust:** `ravenrs::prelude::ColorPicker`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `alpha` | on/off | `true` |  |
| `label` | text | `Category color` |  |
| `disabled` | on/off | `false` |  |
| `read_only` | on/off | `false` |  |

## Stories

- **Example**: 728 × 40 pt, settings `{"alpha":true,"disabled":false,"label":"Category color","read_only":false}`
- **Disabled**: 728 × 40 pt, settings `{"alpha":true,"disabled":true,"label":"Category color","read_only":false}`
- **Read only**: 728 × 40 pt, settings `{"alpha":true,"disabled":false,"label":"Category color","read_only":true}`

## Sizing

Fills the width of its parent: 288, 728, 1168 pt wide in parents of 320, 760, 1200 pt.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover** on "Category color": border {color:border,width:1.0} → {color:mix(ink, border, 25),width:1.0}.
- **Focus** on "Category color": adds a rect (border 2.0 focus-ring).
- **Pressed** on "Category color": border {color:border,width:1.0} → {color:mix(ink, border, 25),width:1.0}.

## Behavior

- **Keys** its first control handles once focused: `ArrowDown`, `ArrowUp`, `Enter`, `Space`, `Escape`.
- **Accessibility**: role `ComboBox` "Category color".
- **Focus order** (Tab): ComboBox "Category color".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | ComboBox "Category color" | 728.0 × 40.0 | radius-md | bg-300 | 1.0 border | Inter 400 15.0px ink | chevron-down 16.0px ink-muted | 40.0 |
| Disabled | ComboBox "Category color" | 728.0 × 40.0 | radius-md | bg-300@45 | 1.0 border@45 | Inter 400 15.0px ink@45 | chevron-down 16.0px ink-muted@45 | 40.0 |
| Read only | ComboBox "Category color" | 728.0 × 40.0 | radius-md | bg-300@50 | 1.0 border | Inter 400 15.0px ink | chevron-down 16.0px ink-muted | 40.0 |

## API

Builder methods: `alpha`, `disabled`, `label`, `new`, `palette`, `read_only`, `width`.

## Example

```rust
fn color_picker() -> impl View {
    let color = State::new(Some(Color::hex(0x2563EB)));
    ColorPicker::new(color.binding())
        .alpha(true)
        .palette([
            (Color::hex(0x2563EB), "Blue"),
            (Color::hex(0x16A34A), "Green"),
            (Color::hex(0xDC2626), "Red"),
            (Color::hex(0xF59E0B), "Amber"),
        ])
        .label("Category color")
}
```

Every number above is in `ColorPicker.json` beside this card.
