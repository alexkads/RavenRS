# Collapse

Animates its content's height as it opens and closes.

**Family:** Layout · **Rust:** `ravenrs::prelude::Collapse`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `open` | on/off | `true` | How the example starts. |

## Stories

- **Example**: 312 × 74 pt, settings `{"open":true}`
- **Closed**: 86 × 40 pt, settings `{"open":false}`

## Sizing

Hugs its content: 312 × 74 pt in a parent of 760 pt; in a parent of 320 pt it narrows to 254 pt; its height follows the width, as its content wraps.

- **Closed**: Hugs its content: 86 × 40 pt in a parent of 760 pt.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover** on "Toggle": fill bg-300 → mix(ink, bg-300, 6).
- **Focus** on "Toggle": adds a rect (border 2.0 focus-ring).
- **Pressed** on "Toggle": border {color:border,width:1.0} → {color:border,width:0.97}, fill bg-300 → mix(ink, bg-300, 12), rect [0.0,0.0,86.0,40.0] → [1.29,0.6,83.42,38.8].

## Behavior

- **Keys** its first control handles once focused: `Enter`, `Space`.
- **Accessibility**: role `Button` "Toggle".
- **Focus order** (Tab): Button "Toggle".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Button "Toggle" | 86.0 × 40.0 | radius-md | bg-300 | 1.0 border | Inter 600 15.0px ink | — | 18.0 |
| Example | Label "The height animates, whatever the content." | 307.0 × 22.0 | — | — | — | Inter 400 15.0px ink | — | 0.0 |

## API

Builder methods: `boxed`, `new`.

## Example

```rust
fn collapse() -> impl View {
    let open = State::new(true);
    let flip = open.clone();
    view_fn(move || {
        let toggle = flip.clone();
        VStack::new((
            Button::new("Toggle").style(ButtonStyle::Secondary).on_click(move || toggle.update(|o| *o = !*o)),
            Collapse::new(open.get(), Text::new("The height animates, whatever the content.")),
        ))
        .spacing(12.0)
    })
}
```

Every number above is in `Collapse.json` beside this card.
