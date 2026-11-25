# Section

Titled group of content with an optional description.

**Family:** Layout · **Rust:** `ravenrs::prelude::Section`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `description` | text | `` | Empty leaves it as the component has it. |

## Stories

- **Example**: 217 × 70 pt, settings `{"description":""}`
- **With description**: 254 × 92 pt, settings `{"description":"Who can see and change it."}`

## Sizing

Hugs its content: 217 × 70 pt in a parent of 760 pt; in a parent that stretches its children, as a form column, it takes the whole width.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover** on "Edit": fill bg-300 → mix(ink, bg-300, 6).
- **Focus** on "Edit": adds a rect (border 2.0 focus-ring).
- **Pressed** on "Edit": border {color:border,width:1.0} → {color:border,width:0.97}, fill bg-300 → mix(ink, bg-300, 12), rect [165.0,0.0,48.0,32.0] → [165.72,0.48,46.56,31.04].

## Behavior

- **Accessibility**: role `Heading` "Notifications".
- **Focus order** (Tab): Button "Edit".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Heading "Notifications" | 137.0 × 28.0 | — | — | — | Inter 600 22.0px ink | — | 0.0 |
| Example | Button "Edit" | 48.0 × 32.0 | radius-md | bg-300 | 1.0 border | Inter 600 13.0px ink | — | space-3 |
| Example | Label "Grouped content under a title." | 213.0 × 22.0 | — | — | — | Inter 400 15.0px ink-muted | — | 0.0 |

## API

Builder methods: `action`, `description`, `new`.

## Example

```rust
fn section() -> impl View {
    Section::new(
        "Notifications",
        (Text::new("Grouped content under a title.").foreground(Semantic::InkMuted),),
    )
    .action(Button::new("Edit").style(ButtonStyle::Secondary).size(ButtonSize::Small))
}
```

Every number above is in `Section.json` beside this card.
