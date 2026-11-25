# StatusBar

One fixed line at the foot of an app window.

**Family:** Forms · **Rust:** `ravenrs::prelude::StatusBar`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `center_tone` | Neutral · Accent · Success · Warning · Danger | `Warning` |  |

## Stories

- **Example**: 728 × 29 pt, settings `{"center_tone":"Warning"}`
- **Center tone: Neutral**: 728 × 29 pt, settings `{"center_tone":"Neutral"}`
- **Center tone: Accent**: 728 × 29 pt, settings `{"center_tone":"Accent"}`
- **Center tone: Success**: 728 × 29 pt, settings `{"center_tone":"Success"}`
- **Center tone: Danger**: 728 × 29 pt, settings `{"center_tone":"Danger"}`

## Sizing

Fills the width of its parent: 288, 728, 1168 pt wide in parents of 320, 760, 1200 pt; its height follows the width, as its content wraps.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover** on "Version 27.1": fill transparent → ink@7.
- **Focus** on "Version 27.1": adds a rect (border 2.0 focus-ring).
- **Pressed** on "Version 27.1": fill transparent → ink@12, rect [8.0,4.0,89.0,22.0] → [9.33,4.33,86.33,21.34].

## Behavior

- **Accessibility**: role `ContentInfo`.
- **Focus order** (Tab): Button "Version 27.1" → Button "What's new" → Button "Update".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Button "Version 27.1" | 89.0 × 22.0 | radius-md | transparent | — | Inter 600 12.0px ink | — | space-2 |
| Example | Status | 556.0 × 28.0 | 0.0 | warning@16 | — | Inter 600 12.0px ink-muted | — | space-2 |
| Example | Label "Version 27.2 is available" | 382.0 × 16.0 | — | — | — | Inter 600 12.0px ink-muted | — | 0.0 |
| Example | Button "Update" | 61.0 × 22.0 | radius-md | accent | — | Inter 600 12.0px on-accent | — | space-2 |
| Center tone: Neutral | Status | 556.0 × 28.0 | 0.0 | ink-muted@16 | — | Inter 600 12.0px ink-muted | — | space-2 |
| Center tone: Accent | Status | 556.0 × 28.0 | 0.0 | #8196f929 | — | Inter 600 12.0px ink-muted | — | space-2 |
| Center tone: Success | Status | 556.0 × 28.0 | 0.0 | success@16 | — | Inter 600 12.0px ink-muted | — | space-2 |
| Center tone: Danger | Status | 556.0 × 28.0 | 0.0 | danger@16 | — | Inter 600 12.0px ink-muted | — | space-2 |

## API

Builder methods: `action`, `center`, `center_tone`, `left`, `new`, `right`, `text`.

## Example

```rust
fn status_bar() -> impl View {
    StatusBar::new()
        .left((StatusBar::action("Version 27.1"),))
        .center((
            StatusBar::text("Version 27.2 is available").truncate().grow(1.0),
            StatusBar::action("What's new"),
            StatusBar::action("Update").style(ButtonStyle::Primary),
        ))
        .center_tone(Tone::Warning)
        .right((StatusBar::text("12 items"),))
}
```

Every number above is in `StatusBar.json` beside this card.
