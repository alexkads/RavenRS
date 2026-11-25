# Card

A grouped surface lifted off the page by its fill and a soft shadow.

**Family:** Layout · **Rust:** `ravenrs::prelude::Card`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `title` | text | `Quarterly report` |  |
| `text` | text | `A grouped surface, lifted off the page.` |  |
| `padding` | number 0–48 by 4 | `24` | Inside space, in points; space-6 by default. |
| `selected` | on/off | `false` | The chosen card of a group. |
| `no_shadow` | on/off | `false` | Flat, for a card inside another surface. |
| `tone` | Neutral · Accent · Success · Warning · Danger | `Neutral` | What the card is about, by color: a tint of the tone and a hairline of it. |

## Stories

- **Default**: 319 × 98 pt, settings `{"no_shadow":false,"padding":24.0,"selected":false,"text":"A grouped surface, lifted off the page.","title":"Quarterly report","tone":"Neutral"}`
- **Selected**: 319 × 98 pt, settings `{"no_shadow":false,"padding":24.0,"selected":true,"text":"A grouped surface, lifted off the page.","title":"Quarterly report","tone":"Neutral"}`
- **Flat**: 319 × 98 pt, settings `{"no_shadow":true,"padding":24.0,"selected":false,"text":"A grouped surface, lifted off the page.","title":"Quarterly report","tone":"Neutral"}`
- **Tone: Accent**: 319 × 98 pt, settings `{"no_shadow":false,"padding":24.0,"selected":false,"text":"A grouped surface, lifted off the page.","title":"Quarterly report","tone":"Accent"}`
- **Tone: Success**: 319 × 98 pt, settings `{"no_shadow":false,"padding":24.0,"selected":false,"text":"A grouped surface, lifted off the page.","title":"Quarterly report","tone":"Success"}`
- **Tone: Warning**: 319 × 98 pt, settings `{"no_shadow":false,"padding":24.0,"selected":false,"text":"A grouped surface, lifted off the page.","title":"Quarterly report","tone":"Warning"}`
- **Tone: Danger**: 319 × 98 pt, settings `{"no_shadow":false,"padding":24.0,"selected":false,"text":"A grouped surface, lifted off the page.","title":"Quarterly report","tone":"Danger"}`

## Sizing

Hugs its content: 319 × 98 pt in a parent of 760 pt; in a parent of 320 pt it narrows to 275 pt; in a parent that stretches its children, as a form column, it takes the whole width; its height follows the width, as its content wraps.

## Behavior

- **Events**: `on_click`.
- **Accessibility**: role `Group`.

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Default | Heading "Quarterly report" | 133.0 × 24.0 | — | — | — | Inter 600 17.0px ink | — | 0.0 |
| Default | Label "A grouped surface, lifted off the page." | 271.0 × 22.0 | — | — | — | Inter 400 15.0px ink-muted | — | 0.0 |

## API

Builder methods: `border`, `corner_radius`, `fill`, `new`, `no_shadow`, `on_click`, `padding`, `selected`, `shadow`, `tone`.

## Example

```rust
fn card() -> impl View {
    Card::new(
        VStack::new((
            Text::new("Quarterly report").style(TextStyle::Headline),
            Text::new("A grouped surface, lifted off the page.").foreground(Semantic::InkMuted),
        ))
        .spacing(4.0),
    )
    // Set it on the chosen card of a group.
    .selected(false)
}
```

Every number above is in `Card.json` beside this card.
