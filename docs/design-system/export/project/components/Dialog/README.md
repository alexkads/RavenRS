# Dialog

Modal window over a dimmed backdrop.

**Family:** Overlays · **Rust:** `ravenrs::prelude::Dialog`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `description` | text | `This cannot be undone.` |  |
| `close_button` | on/off | `true` |  |
| `close_label` | text | `` | Empty leaves it as the component has it. |
| `close_on_backdrop` | on/off | `true` |  |
| `close_on_escape` | on/off | `true` |  |
| `width` | number 0–1840 by 1 | `460` |  |
| `open` | on/off | `false` | How the example starts. |

## Stories

- **Example**: 138 × 40 pt, settings `{"close_button":true,"close_label":"","close_on_backdrop":true,"close_on_escape":true,"description":"This cannot be undone.","open":false,"width":460.0}`
- **Open**: 594 × 623 pt, settings `{"close_button":true,"close_label":"","close_on_backdrop":true,"close_on_escape":true,"description":"This cannot be undone.","open":true,"width":460.0}`
- **Open, no close button**: 594 × 621 pt, settings `{"close_button":false,"close_label":"","close_on_backdrop":true,"close_on_escape":true,"description":"This cannot be undone.","open":true,"width":460.0}`

## Sizing

Hugs its content: 138 × 40 pt in a parent of 760 pt.

- **Open**: Hugs its content: 460 × 178 pt in a parent of 760 pt; in a parent of 320 pt it narrows to 288 pt; its height follows the width, as its content wraps.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover** on "Delete project": fill danger → mix(on-accent, danger, 8).
- **Focus** on "Delete project": adds a rect (border 2.0 focus-ring).
- **Pressed** on "Delete project": fill danger → #c44b4b, rect [0.0,0.0,138.0,40.0] → [2.07,0.6,133.86,38.8].

## Behavior

- **Keys** its first control handles once focused: `Enter`, `Space`, `Escape`.
- **Accessibility**: role `Button` "Delete project".
- **Focus order** (Tab): Button "Delete project".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Button "Delete project" | 138.0 × 40.0 | radius-md | danger | — | Inter 600 15.0px on-accent | — | 18.0 |
| Open | Dialog "Delete the project?" | 460.0 × 178.0 | radius-xl | bg-200 | 1.0 border | Inter 600 22.0px ink | x 14.0px ink | space-6 |
| Open | Heading "Delete the project?" | 368.0 × 28.0 | — | — | — | Inter 600 22.0px ink | — | 0.0 |
| Open | Button "Close" | 32.0 × 32.0 | radius-md | transparent | — | — | x 14.0px ink | 9.0 |
| Open | Label "This cannot be undone." | 412.0 × 22.0 | — | — | — | Inter 400 15.0px ink-muted | — | 0.0 |
| Open | Button "Cancel" | 87.0 × 40.0 | radius-md | bg-300 | 1.0 border | Inter 600 15.0px ink | — | 18.0 |
| Open, no close button | Dialog "Delete the project?" | 460.0 × 174.0 | radius-xl | bg-200 | 1.0 border | Inter 600 22.0px ink | — | space-6 |

## API

Builder methods: `action`, `close_button`, `close_label`, `close_on_backdrop`, `close_on_escape`, `content`, `description`, `new`, `width`.

## Example

```rust
fn dialog() -> impl View {
    let open = State::new(false);
    let (show, cancel, confirm) = (open.clone(), open.clone(), open.clone());
    VStack::new((
        Button::new("Delete project").style(ButtonStyle::Danger).on_click(move || show.set(true)),
        Dialog::new(open.binding(), "Delete the project?")
            .description("This cannot be undone.")
            .action(Button::new("Cancel").style(ButtonStyle::Secondary).on_click(move || cancel.set(false)))
            .action(Button::new("Delete").style(ButtonStyle::Danger).on_click(move || confirm.set(false)))
            // A stray click outside must not close this one.
            .close_on_backdrop(false),
    ))
}
```

Every number above is in `Dialog.json` beside this card.
