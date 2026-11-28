# Badge

Short label for a state or a count, in a tone that says what it means.

**Family:** Display and feedback · **Rust:** `ravenrs::prelude::Badge`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `text` | text | `New` |  |
| `tone` | Neutral · Accent · Success · Warning · Danger | `Accent` |  |
| `icon` | none · plus · check · trash-2 · pencil · search · settings · download | `none` |  |
| `outline` | on/off | `false` | Only the edge, for a draft or a quiet tag. |

## Stories

- **Accent**: 46 × 22 pt, settings `{"icon":"none","outline":false,"text":"New","tone":"Accent"}`
- **Neutral**: 64 × 22 pt, settings `{"icon":"none","outline":false,"text":"Neutral","tone":"Neutral"}`
- **Success with icon**: 61 × 22 pt, settings `{"icon":"check","outline":false,"text":"Paid","tone":"Success"}`
- **Danger**: 71 × 22 pt, settings `{"icon":"none","outline":false,"text":"Overdue","tone":"Danger"}`
- **Outline**: 49 × 22 pt, settings `{"icon":"none","outline":true,"text":"Draft","tone":"Neutral"}`
- **Warning**: 69 × 22 pt, settings `{"icon":"none","outline":false,"text":"Pending","tone":"Warning"}`

## Sizing

Hugs its content: 46 × 22 pt in a parent of 760 pt.

## Behavior

- **Accessibility**: role `Label` "New".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Accent | Label "New" | 28.0 × 16.0 | — | — | — | Inter 600 12.0px #a1b1f8 | — | 0.0 |
| Neutral | Label "Neutral" | 46.0 × 16.0 | — | — | — | Inter 600 12.0px mix(ink, ink-muted, 30) | — | 0.0 |
| Success with icon | Label "Paid" | 27.0 × 16.0 | — | — | — | Inter 600 12.0px mix(success, ink, 70) | — | 0.0 |
| Danger | Label "Overdue" | 53.0 × 16.0 | — | — | — | Inter 600 12.0px mix(ink, danger, 30) | — | 0.0 |
| Warning | Label "Pending" | 51.0 × 16.0 | — | — | — | Inter 600 12.0px mix(ink, warning, 30) | — | 0.0 |

## API

Builder methods: `color`, `height`, `icon`, `new`, `outline`, `tone`.

## Example

```rust
fn badge() -> impl View {
    HStack::new((
        Badge::new("Neutral"),
        Badge::new("New").tone(Tone::Accent),
        Badge::new("Paid").tone(Tone::Success).icon(IconName::Check),
        Badge::new("Overdue").tone(Tone::Danger),
        Badge::new("Draft").outline(),
        // ravenrs-lint: allow(raw_color) a tag keeps the color its owner picked for it, in every theme
        Badge::new("Family").color(Color::hex(0x8b5cf6)).height(26.0),
    ))
    .spacing(8.0)
    .wrap()
}
```

Every number above is in `Badge.json` beside this card.
