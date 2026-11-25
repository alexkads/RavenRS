# Sheet

Panel that slides over the UI from one edge.

**Family:** Overlays · **Rust:** `ravenrs::prelude::Sheet`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `description` | text | `Narrow down the accounts shown.` |  |
| `side` | Left · Right | `Right` |  |
| `width` | number 0–1600 by 1 | `400` |  |
| `open` | on/off | `false` | How the example starts. |
| `active` | on/off | `true` | How the example starts. |

## Stories

- **Example**: 121 × 40 pt, settings `{"active":true,"description":"Narrow down the accounts shown.","open":false,"side":"Right","width":400.0}`
- **Side: Left**: 384 × 118 pt, settings `{"active":true,"description":"Narrow down the accounts shown.","open":true,"side":"Left","width":400.0}`
- **Open**: 744 × 118 pt, settings `{"active":true,"description":"Narrow down the accounts shown.","open":true,"side":"Right","width":400.0}`

## Sizing

Hugs its content: 121 × 40 pt in a parent of 760 pt.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover** on "Open filters": fill bg-300 → mix(ink, bg-300, 6).
- **Focus** on "Open filters": adds a rect (border 2.0 focus-ring).
- **Pressed** on "Open filters": border {color:border,width:1.0} → {color:border,width:0.97}, fill bg-300 → mix(ink, bg-300, 12), rect [0.0,0.0,121.0,40.0] → [1.81,0.6,117.37,38.8].

## Behavior

- **Keys** its first control handles once focused: `Enter`, `Space`, `Escape`.
- **Accessibility**: role `Button` "Open filters".
- **Focus order** (Tab): Button "Open filters".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Button "Open filters" | 121.0 × 40.0 | radius-md | bg-300 | 1.0 border | Inter 600 15.0px ink | — | 18.0 |
| Side: Left | Dialog "Filters" | 400.0 × 1100.0 | 0.0 | bg-200 | 1.0 border | Inter 600 15.0px ink | x 14.0px ink | 34.0 |
| Side: Left | Label "Filters" | 67.0 × 28.0 | — | — | — | Inter 600 15.0px ink | — | 10.0 |
| Side: Left | Button "Close" | 32.0 × 32.0 | radius-md | transparent | — | — | x 14.0px ink | 9.0 |
| Side: Left | Label "Narrow down the accounts shown." | 352.0 × 22.0 | — | — | — | Inter 400 15.0px ink-muted | — | 0.0 |
| Side: Left | Switch "Only active accounts" | 199.0 × 24.0 | — | — | — | Inter 400 15.0px ink | — | 50.0 |
| Open | Dialog "Filters" | 400.0 × 1100.0 | 0.0 | bg-200 | 1.0 border | Inter 600 22.0px ink | x 14.0px ink | space-6 |
| Open | Label "Filters" | 67.0 × 28.0 | — | — | — | Inter 600 22.0px ink | — | 0.0 |

## API

Builder methods: `content`, `description`, `new`, `side`, `width`.

## Example

```rust
fn sheet() -> impl View {
    let open = State::new(false);
    let show = open.clone();
    let active = State::new(true);
    VStack::new((
        Button::new("Open filters").style(ButtonStyle::Secondary).on_click(move || show.set(true)),
        Sheet::new(open.binding(), "Filters")
            .description("Narrow down the accounts shown.")
            .content(Toggle::new(active.binding()).label("Only active accounts")),
    ))
}
```

Every number above is in `Sheet.json` beside this card.
