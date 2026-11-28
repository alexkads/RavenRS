# ShortcutSheet

Dialog listing the keys of a screen, in groups.

**Family:** Display and feedback · **Rust:** `ravenrs::prelude::ShortcutSheet`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `title` | text | `` | Empty leaves it as the component has it. |
| `open` | on/off | `false` | How the example starts. |

## Stories

- **Example**: 174 × 40 pt, settings `{"open":false,"title":""}`
- **Open**: 694 × 619 pt, settings `{"open":true,"title":""}`

## Sizing

Hugs its content: 174 × 40 pt in a parent of 760 pt.

- **Open**: Hugs its content: 660 × 170 pt in a parent of 760 pt; in a parent of 320 pt it narrows to 288 pt; its height follows the width, as its content wraps.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover** on "Show shortcuts": fill bg-300 → mix(ink, bg-300, 6).
- **Focus** on "Show shortcuts": adds a rect (border 2.0 focus-ring).
- **Pressed** on "Show shortcuts": border {color:border,width:1.0} → {color:border,width:0.97}, fill bg-300 → mix(ink, bg-300, 12), rect [0.0,0.0,174.0,40.0] → [2.61,0.6,168.78,38.8].

## Behavior

- **Keys** its first control handles once focused: `Enter`, `Space`, `Escape`.
- **Accessibility**: role `Button` "Show shortcuts".
- **Focus order** (Tab): Button "Show shortcuts".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Button "Show shortcuts" | 174.0 × 40.0 | radius-md | bg-300 | 1.0 border | Inter 600 15.0px ink | keyboard 16.0px ink | 18.0 |
| Open | Dialog "Keyboard shortcuts" | 660.0 × 170.0 | radius-xl | bg-200 | 1.0 border | Inter 600 22.0px ink | x 14.0px ink | space-6 |
| Open | Heading "Keyboard shortcuts" | 568.0 × 28.0 | — | — | — | Inter 600 22.0px ink | — | 0.0 |
| Open | Button "Close" | 32.0 × 32.0 | radius-md | transparent | — | — | x 14.0px ink | 9.0 |
| Open | Heading "Navigate" | 55.0 × 16.0 | — | — | — | Inter 600 12.0px ink-muted | — | 0.0 |
| Open | Label "←" | 12.0 × 16.0 | — | — | — | Inter 400 12.0px ink-muted | — | 0.0 |
| Open | Label "Previous photo" | 93.0 × 18.0 | — | — | — | Inter 400 13.0px ink-muted | — | 0.0 |

## API

Builder methods: `group`, `new`, `title`, `width`.

## Example

```rust
fn shortcut_sheet() -> impl View {
    let open = State::new(false);
    let show = open.clone();
    VStack::new((
        Button::new("Show shortcuts")
            .style(ButtonStyle::Secondary)
            .icon(IconName::Keyboard)
            .on_click(move || show.set(true)),
        ShortcutSheet::new(open.binding())
            .group("Navigate", [("←", "Previous photo"), ("→", "Next photo")])
            .group("Rate", [("1–5", "Give a score"), ("0", "Take it away")]),
    ))
}
```

Every number above is in `ShortcutSheet.json` beside this card.
