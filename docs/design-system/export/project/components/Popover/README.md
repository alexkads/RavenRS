# Popover

Floating content anchored below a view.

**Family:** Overlays · **Rust:** `ravenrs::prelude::Popover`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `min_width` | number 0–880 by 1 | `220` |  |
| `padding` | number 0–64 by 1 | `16` |  |
| `surface` | on/off | `true` |  |
| `open` | on/off | `false` | How the example starts. |

## Stories

- **Example**: 129 × 40 pt, settings `{"min_width":220.0,"open":false,"padding":16.0,"surface":true}`
- **Open**: 252 × 118 pt, settings `{"min_width":220.0,"open":true,"padding":16.0,"surface":true}`

## Sizing

Hugs its content: 129 × 40 pt in a parent of 760 pt.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover** on "Show details": fill bg-300 → mix(ink, bg-300, 6).
- **Focus** on "Show details": adds a rect (border 2.0 focus-ring).
- **Pressed** on "Show details": border {color:border,width:1.0} → {color:border,width:0.97}, fill bg-300 → mix(ink, bg-300, 12), rect [0.0,0.0,129.0,40.0] → [1.93,0.6,125.13,38.8].

## Behavior

- **Keys** its first control handles once focused: `Enter`, `Space`.
- **Accessibility**: role `Button` "Show details".
- **Focus order** (Tab): Button "Show details".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Button "Show details" | 129.0 × 40.0 | radius-md | bg-300 | 1.0 border | Inter 600 15.0px ink | — | 18.0 |
| Open | Heading "Storage" | 65.0 × 24.0 | — | — | — | Inter 600 17.0px ink | — | 0.0 |
| Open | ProgressIndicator | 220.0 × 6.0 | radius-full | bg-300 | — | — | — | — |

## API

Builder methods: `min_width`, `new`, `padding`, `placement`, `surface`.

## Example

```rust
fn popover() -> impl View {
    let open = State::new(false);
    let flip = open.clone();
    Popover::new(
        open.binding(),
        Button::new("Show details").style(ButtonStyle::Secondary).on_click(move || flip.update(|o| *o = !*o)),
        VStack::new((Text::new("Storage").style(TextStyle::Headline), ProgressBar::new(0.62)))
            .spacing(8.0)
            .width(220.0),
    )
    // `Placement::Above` and the others work too; `.surface(false)` drops the panel.
    .placement(Placement::Below)
}
```

Every number above is in `Popover.json` beside this card.
