# SankeyChart

How much flows from what to what: ribbons between nodes, arrow keys, and a clear message when the flows go in a circle.

**Family:** Data · **Rust:** `ravenrs::prelude::SankeyChart`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `title` | text | `Where the money goes` |  |
| `height` | number 0–1200 by 1 | `300` |  |
| `alt` | text | `` | Empty leaves it as the component has it. |
| `hide_tooltip` | on/off | `false` |  |
| `no_appear` | on/off | `false` |  |
| `no_gradient` | on/off | `false` |  |

## Stories

- **Example**: 728 × 305 pt, settings `{"alt":"","height":300.0,"hide_tooltip":false,"no_appear":false,"no_gradient":false,"title":"Where the money goes"}`
- **Flat ribbons**: 728 × 305 pt, settings `{"alt":"","height":300.0,"hide_tooltip":false,"no_appear":false,"no_gradient":true,"title":"Where the money goes"}`

## Sizing

Fills the width of its parent: 288, 728, 1168 pt wide in parents of 320, 760, 1200 pt.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover** on "Where the money goes": fill Linear { start: Point { x: 12.0, y: 53.168903 }, end: Point { x: 716.0, y: 31.902431 }, stops: [ColorStop { offset: 0.0, color: Color { r: 0.29803923, g: 0.4, b: 0.93333334, a: 0.38 } }, ColorStop { offset: 1.0, color: Color { r: 0.1764706, g: 0.83137256, b: 0.7490196, a: 0.38 } }] } → Linear { start: Point { x: 12.0, y: 53.168903 }, end: Point { x: 716.0, y: 31.902431 }, stops: [ColorStop { offset: 0.0, color: Color { r: 0.29803923, g: 0.4, b: 0.93333334, a: 0.114 } }, ColorStop { offset: 1.0, color: Color { r: 0.1764706, g: 0.83137256, b: 0.7490196, a: 0.114 } }] }; fill Linear { start: Point { x: 12.0, y: 53.168903 }, end: Point { x: 716.0, y: 31.902431 }, stops: [ColorStop { offset: 0.0, color: Color { r: 0.29803923, g: 0.4, b: 0.93333334, a: 0.38 } }, ColorStop { offset: 1.0, color: Color { r: 0.1764706, g: 0.83137256, b: 0.7490196, a: 0.38 } }] } → Linear { start: Point { x: 12.0, y: 124.949394 }, end: Point { x: 358.0, y: 120.78994 }, stops: [ColorStop { offset: 0.0, color: Color { r: 0.29803923, g: 0.4, b: 0.93333334, a: 0.7 } }, ColorStop { offset: 1.0, color: Color { r: 0.654902, g: 0.54509807, b: 0.98039216, a: 0.7 } }] }; fill Linear { start: Point { x: 12.0, y: 53.168903 }, end: Point { x: 716.0, y: 31.902431 }, stops: [ColorStop { offset: 0.0, color: Color { r: 0.29803923, g: 0.4, b: 0.93333334, a: 0.38 } }, ColorStop { offset: 1.0, color: Color { r: 0.1764706, g: 0.83137256, b: 0.7490196, a: 0.38 } }] } → Linear { start: Point { x: 12.0, y: 186.09575 }, end: Point { x: 358.0, y: 209.71729 }, stops: [ColorStop { offset: 0.0, color: Color { r: 0.29803923, g: 0.4, b: 0.93333334, a: 0.114 } }, ColorStop { offset: 1.0, color: Color { r: 0.9843137, g: 0.7490196, b: 0.14117648, a: 0.114 } }] }; fill Linear { start: Point { x: 12.0, y: 53.168903 }, end: Point { x: 716.0, y: 31.902431 }, stops: [ColorStop { offset: 0.0, color: Color { r: 0.29803923, g: 0.4, b: 0.93333334, a: 0.38 } }, ColorStop { offset: 1.0, color: Color { r: 0.1764706, g: 0.83137256, b: 0.7490196, a: 0.38 } }] } → Linear { start: Point { x: 12.0, y: 237.31525 }, end: Point { x: 358.0, y: 246.9368 }, stops: [ColorStop { offset: 0.0, color: Color { r: 0.95686275, g: 0.44705883, b: 0.7137255, a: 0.114 } }, ColorStop { offset: 1.0, color: Color { r: 0.9843137, g: 0.7490196, b: 0.14117648, a: 0.114 } }] }.
- **Focus** on "Where the money goes": fill Linear { start: Point { x: 12.0, y: 53.168903 }, end: Point { x: 716.0, y: 31.902431 }, stops: [ColorStop { offset: 0.0, color: Color { r: 0.29803923, g: 0.4, b: 0.93333334, a: 0.38 } }, ColorStop { offset: 1.0, color: Color { r: 0.1764706, g: 0.83137256, b: 0.7490196, a: 0.38 } }] } → Linear { start: Point { x: 12.0, y: 124.949394 }, end: Point { x: 358.0, y: 120.78994 }, stops: [ColorStop { offset: 0.0, color: Color { r: 0.29803923, g: 0.4, b: 0.93333334, a: 0.38 } }, ColorStop { offset: 1.0, color: Color { r: 0.654902, g: 0.54509807, b: 0.98039216, a: 0.38 } }] }; fill Linear { start: Point { x: 12.0, y: 53.168903 }, end: Point { x: 716.0, y: 31.902431 }, stops: [ColorStop { offset: 0.0, color: Color { r: 0.29803923, g: 0.4, b: 0.93333334, a: 0.38 } }, ColorStop { offset: 1.0, color: Color { r: 0.1764706, g: 0.83137256, b: 0.7490196, a: 0.38 } }] } → Linear { start: Point { x: 12.0, y: 186.09575 }, end: Point { x: 358.0, y: 209.71729 }, stops: [ColorStop { offset: 0.0, color: Color { r: 0.29803923, g: 0.4, b: 0.93333334, a: 0.38 } }, ColorStop { offset: 1.0, color: Color { r: 0.9843137, g: 0.7490196, b: 0.14117648, a: 0.38 } }] }; fill Linear { start: Point { x: 12.0, y: 53.168903 }, end: Point { x: 716.0, y: 31.902431 }, stops: [ColorStop { offset: 0.0, color: Color { r: 0.29803923, g: 0.4, b: 0.93333334, a: 0.38 } }, ColorStop { offset: 1.0, color: Color { r: 0.1764706, g: 0.83137256, b: 0.7490196, a: 0.38 } }] } → Linear { start: Point { x: 12.0, y: 237.31525 }, end: Point { x: 358.0, y: 246.9368 }, stops: [ColorStop { offset: 0.0, color: Color { r: 0.95686275, g: 0.44705883, b: 0.7137255, a: 0.38 } }, ColorStop { offset: 1.0, color: Color { r: 0.9843137, g: 0.7490196, b: 0.14117648, a: 0.38 } }] }; fill Linear { start: Point { x: 12.0, y: 53.168903 }, end: Point { x: 716.0, y: 31.902431 }, stops: [ColorStop { offset: 0.0, color: Color { r: 0.29803923, g: 0.4, b: 0.93333334, a: 0.38 } }, ColorStop { offset: 1.0, color: Color { r: 0.1764706, g: 0.83137256, b: 0.7490196, a: 0.38 } }] } → Linear { start: Point { x: 370.0, y: 110.15579 }, end: Point { x: 716.0, y: 107.048775 }, stops: [ColorStop { offset: 0.0, color: Color { r: 0.654902, g: 0.54509807, b: 0.98039216, a: 0.38 } }, ColorStop { offset: 1.0, color: Color { r: 0.055, g: 0.647, b: 0.914, a: 0.38 } }] }.
- **Pressed** on "Where the money goes": fill Linear { start: Point { x: 12.0, y: 53.168903 }, end: Point { x: 716.0, y: 31.902431 }, stops: [ColorStop { offset: 0.0, color: Color { r: 0.29803923, g: 0.4, b: 0.93333334, a: 0.38 } }, ColorStop { offset: 1.0, color: Color { r: 0.1764706, g: 0.83137256, b: 0.7490196, a: 0.38 } }] } → Linear { start: Point { x: 12.0, y: 53.168903 }, end: Point { x: 716.0, y: 31.902431 }, stops: [ColorStop { offset: 0.0, color: Color { r: 0.29803923, g: 0.4, b: 0.93333334, a: 0.114 } }, ColorStop { offset: 1.0, color: Color { r: 0.1764706, g: 0.83137256, b: 0.7490196, a: 0.114 } }] }; fill Linear { start: Point { x: 12.0, y: 53.168903 }, end: Point { x: 716.0, y: 31.902431 }, stops: [ColorStop { offset: 0.0, color: Color { r: 0.29803923, g: 0.4, b: 0.93333334, a: 0.38 } }, ColorStop { offset: 1.0, color: Color { r: 0.1764706, g: 0.83137256, b: 0.7490196, a: 0.38 } }] } → Linear { start: Point { x: 12.0, y: 124.949394 }, end: Point { x: 358.0, y: 120.78994 }, stops: [ColorStop { offset: 0.0, color: Color { r: 0.29803923, g: 0.4, b: 0.93333334, a: 0.7 } }, ColorStop { offset: 1.0, color: Color { r: 0.654902, g: 0.54509807, b: 0.98039216, a: 0.7 } }] }; fill Linear { start: Point { x: 12.0, y: 53.168903 }, end: Point { x: 716.0, y: 31.902431 }, stops: [ColorStop { offset: 0.0, color: Color { r: 0.29803923, g: 0.4, b: 0.93333334, a: 0.38 } }, ColorStop { offset: 1.0, color: Color { r: 0.1764706, g: 0.83137256, b: 0.7490196, a: 0.38 } }] } → Linear { start: Point { x: 12.0, y: 186.09575 }, end: Point { x: 358.0, y: 209.71729 }, stops: [ColorStop { offset: 0.0, color: Color { r: 0.29803923, g: 0.4, b: 0.93333334, a: 0.114 } }, ColorStop { offset: 1.0, color: Color { r: 0.9843137, g: 0.7490196, b: 0.14117648, a: 0.114 } }] }; fill Linear { start: Point { x: 12.0, y: 53.168903 }, end: Point { x: 716.0, y: 31.902431 }, stops: [ColorStop { offset: 0.0, color: Color { r: 0.29803923, g: 0.4, b: 0.93333334, a: 0.38 } }, ColorStop { offset: 1.0, color: Color { r: 0.1764706, g: 0.83137256, b: 0.7490196, a: 0.38 } }] } → Linear { start: Point { x: 12.0, y: 237.31525 }, end: Point { x: 358.0, y: 246.9368 }, stops: [ColorStop { offset: 0.0, color: Color { r: 0.95686275, g: 0.44705883, b: 0.7137255, a: 0.114 } }, ColorStop { offset: 1.0, color: Color { r: 0.9843137, g: 0.7490196, b: 0.14117648, a: 0.114 } }] }.

## Behavior

- **Keys** its first control handles once focused: `ArrowDown`, `ArrowUp`, `ArrowLeft`, `ArrowRight`, `Escape`, `Home`, `End`.
- **Accessibility**: role `Label` "Where the money goes".
- **Focus order** (Tab): Image "Where the money goes".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Label "Where the money goes" | 728.0 × 18.0 | — | — | — | Inter 600 13.0px ink | — | 0.0 |
| Example | Image "Where the money goes" | 728.0 × 274.0 | — | — | — | Inter 400 12.0px ink | — | 18.0 |

## API

Builder methods: `align`, `alt`, `appear_key`, `flow`, `format`, `height`, `hide_tooltip`, `new`, `no_appear`, `no_gradient`, `node_color`, `size`, `title`.

## Example

```rust
fn sankey_chart() -> impl View {
    SankeyChart::new()
        .title("Where the money goes")
        .flow("Salary", "Taxes", 1200.0)
        .flow("Salary", "Home", 1500.0)
        .flow("Salary", "Savings", 800.0)
        .flow("Freelance", "Savings", 600.0)
        .flow("Home", "Rent", 1100.0)
        .flow("Home", "Power", 400.0)
        .flow("Savings", "Funds", 900.0)
        .flow("Savings", "Stocks", 500.0)
        .format(|value| format!("$ {value:.0}"))
        .height(300.0)
}
```

Every number above is in `SankeyChart.json` beside this card.
