# HelpTip

A small question mark that explains something.

**Family:** Overlays · **Rust:** `ravenrs::prelude::HelpTip`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `how` | text | `Choose it when you save.` |  |

## Stories

- **Example**: 109 × 32 pt, settings `{"how":"Choose it when you save."}`

## Sizing

Hugs its content: 109 × 32 pt in a parent of 760 pt.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover** on "Help: Recurring": fill transparent → ink@7; adds a rect (border 1.0 border); adds a text (null); adds a text (null).
- **Focus** on "Help: Recurring": adds a rect (border 2.0 focus-ring); adds a rect (border 1.0 border); adds a text (null); adds a text (null).
- **Pressed** on "Help: Recurring": adds a rect (ink@12); adds a icon (null); adds a rect (border 1.0 border); adds a text (null).

## Behavior

- **Keys** its first control handles once focused: `Escape`.
- **Accessibility**: role `Label` "Recurring".
- **Focus order** (Tab): Button "Help: Recurring".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Label "Recurring" | 69.0 × 22.0 | — | — | — | Inter 400 15.0px ink | — | 0.0 |
| Example | Button "Help: Recurring" | 32.0 × 32.0 | radius-md | transparent | — | — | circle-help 14.0px ink | 9.0 |

## API

Builder methods: `how`, `new`.

## Example

```rust
fn help_tip() -> impl View {
    HStack::new((
        Text::new("Recurring"),
        HelpTip::new("Recurring", "It comes back on the same day of each month.")
            .how("Choose it when you save."),
    ))
    .spacing(8.0)
}
```

Every number above is in `HelpTip.json` beside this card.
