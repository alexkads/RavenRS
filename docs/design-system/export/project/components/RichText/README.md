# RichText

Bold, italic, code and links in plain text, with the marks chat apps use.

**Family:** Forms · **Rust:** `ravenrs::prelude::RichText`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `links_only` | on/off | `false` |  |
| `opened` | text | `No link opened yet` | How the example starts. |

## Stories

- **Example**: 360 × 90 pt, settings `{"links_only":false,"opened":"No link opened yet"}`

## Sizing

Hugs its content: 360 × 90 pt in a parent of 760 pt; in a parent of 320 pt it narrows to 291 pt.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover** on "https://example.com/orders": fill mix(ink, accent, 30) → mix(ink, accent-hover, 30); color mix(ink, accent, 30) → mix(ink, accent-hover, 30).
- **Focus** on "https://example.com/orders": adds a rect (border 2.0 focus-ring).
- **Pressed** on "https://example.com/orders": fill mix(ink, accent, 30) → mix(accent-pressed, ink, 70); color mix(ink, accent, 30) → mix(accent-pressed, ink, 70).

## Behavior

- **Events**: `on_link`.
- **Accessibility**: role `Label` "Ready: your order left is at Desk 4.
Details at https://example.com/orders
Open until six".
- **Focus order** (Tab): Link "https://example.com/orders".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Label "Ready: your order left is at Desk 4.
Details at https://example.com/orders
Open until six" | 360.0 × 66.0 | — | — | — | Inter 600 15.0px ink | — | 0.0 |
| Example | Link "https://example.com/orders" | 197.0 × 22.0 | — | — | — | Inter 400 15.0px mix(ink, accent, 30) | — | 0.0 |
| Example | Label "No link opened yet" | 360.0 × 16.0 | — | — | — | Inter 600 12.0px ink-muted | — | 0.0 |

## API

Builder methods: `foreground`, `links_only`, `new`, `on_link`, `plain_text`, `style`.

## Example

```rust
fn rich_text() -> impl View {
    let opened = State::new(String::from("No link opened yet"));
    let told = opened.clone();
    VStack::new((
        RichText::new(
            "*Ready:* _your order_ ~left~ is at `Desk 4`.\n* Details at https://example.com/orders\n> Open until six",
        )
        .on_link(move |address| told.set(format!("Asked to open {address}"))),
        Text::bound(&opened, Clone::clone).style(TextStyle::Caption).foreground(Semantic::InkMuted),
    ))
    .spacing(8.0)
    .align(Align::Stretch)
    .width(360.0)
}
```

Every number above is in `RichText.json` beside this card.
