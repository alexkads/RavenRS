# ReleaseNotes

An app's versions and what each one brought.

**Family:** Forms · **Rust:** `ravenrs::prelude::ReleaseNotes`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `height` | number 0–1840 by 1 | `460` |  |
| `selected` | number 0–5 by 1 | `0` | How the example starts. |

## Stories

- **Example**: 600 × 460 pt, settings `{"height":460.0,"selected":0.0}`

## Sizing

Hugs its content: 600 × 460 pt in a parent of 760 pt; in a parent of 320 pt it narrows to 364 pt; in a parent that stretches its children, as a form column, it takes the whole width.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover**: "Version 27.2" does not change.
- **Focus** on "Version 27.2": adds a rect (border 2.0 focus-ring).
- **Pressed**: "Version 27.2" does not change.

## Behavior

- **Accessibility**: role `List`.
- **Focus order** (Tab): Button "Version 27.2" → Button "Version 27.1" → Button "Older".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Button "Version 27.2" | 212.0 × 62.0 | radius-md | bg-300 | — | Inter 600 15.0px ink | chevron-right 16.0px ink-muted | space-3 |
| Example | Button "Version 27.1" | 212.0 × 62.0 | radius-md | transparent | — | Inter 600 15.0px ink | chevron-right 16.0px ink-muted | space-3 |
| Example | Label "Version 27.2 · Nov 1, 2026" | 161.0 × 16.0 | — | — | — | Inter 600 12.0px ink-muted | — | 0.0 |
| Example | Label "New" | 28.0 × 16.0 | — | — | — | Inter 600 12.0px mix(success, ink, 70) | — | 0.0 |
| Example | Heading "Status bars and release notes" | 364.0 × 24.0 | — | — | — | Inter 600 17.0px ink | — | 0.0 |
| Example | Label "• A line at the foot of the window" | 203.0 × 18.0 | — | — | — | Inter 400 13.0px ink | — | 0.0 |
| Example | Label "Why update" | 76.0 × 18.0 | — | — | — | Inter 600 13.0px ink | — | 0.0 |
| Example | Label "Tell people about new versions without getting in their way." | 364.0 × 18.0 | — | — | — | Inter 400 13.0px ink-muted | — | 0.0 |
| Example | Button "Newer" | 87.0 × 32.0 | radius-md | transparent | — | Inter 600 13.0px ink@45 | chevron-left 14.0px ink@45 | space-3 |
| Example | Button "Older" | 81.0 × 32.0 | radius-md | transparent | — | Inter 600 13.0px ink | chevron-right 14.0px ink | space-3 |

## API

Builder methods: `height`, `new`.

## Example

```rust
fn release_notes() -> impl View {
    let selected = State::new(0);
    ReleaseNotes::new(
        vec![
            ReleaseNote::new("Version 27.2", "Status bars and release notes")
                .date("Nov 1, 2026")
                .badge("New", Tone::Success)
                .changes(["A line at the foot of the window", "What each version brought"])
                .section("Why update", "Tell people about new versions without getting in their way."),
            ReleaseNote::new("Version 27.1", "The first of the year").badge("Installed", Tone::Neutral),
        ],
        selected.binding(),
    )
}
```

Every number above is in `ReleaseNotes.json` beside this card.
