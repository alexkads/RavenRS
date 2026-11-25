# Link

Text that opens an address, or scrolls to an anchor.

**Family:** Layout · **Rust:** `ravenrs::prelude::Link`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `muted` | on/off | `false` |  |

## Stories

- **Example**: 253 × 64 pt, settings `{"muted":false}`
- **Muted**: 253 × 64 pt, settings `{"muted":true}`

## Sizing

Hugs its content: 253 × 64 pt in a parent of 760 pt.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover** on "Read the getting started guide": fill transparent → ink@6; color mix(ink, accent, 40) → ink; color mix(ink, accent, 40) → ink.
- **Focus** on "Read the getting started guide": fill transparent → ink@6; color mix(ink, accent, 40) → ink; color mix(ink, accent, 40) → ink; adds a rect (border 2.0 focus-ring).
- **Pressed** on "Read the getting started guide": fill transparent → ink@6; color mix(ink, accent, 40) → ink; color mix(ink, accent, 40) → ink.

## Behavior

- **Keys** its first control handles once focused: `Escape`.
- **Accessibility**: role `Link` "Read the getting started guide".
- **Focus order** (Tab): Link "Read the getting started guide" → Link "Components".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Link "Read the getting started guide" | 253.0 × 30.0 | radius-sm | transparent | — | Inter 600 15.0px mix(ink, accent, 40) | arrow-right 16.0px mix(ink, accent, 40) | 6.0 |
| Example | Link "Components" | 105.0 × 30.0 | radius-sm | transparent | — | Inter 600 15.0px ink-muted | — | 6.0 |
| Muted | Link "Read the getting started guide" | 253.0 × 30.0 | radius-sm | transparent | — | Inter 600 15.0px ink-muted | arrow-right 16.0px ink-muted | 6.0 |

## API

Builder methods: `icon`, `muted`, `new`, `style`.

## Example

```rust
fn link() -> impl View {
    VStack::new((
        Link::new(
            "Read the getting started guide",
            "https://github.com/alexkads/RavenRS/blob/master/docs/GETTING_STARTED.md",
        )
        .icon(IconName::ArrowRight),
        Link::new("Components", "https://alexkads.github.io/RavenRS/components/").muted(),
    ))
    .spacing(4.0)
}
```

Every number above is in `Link.json` beside this card.
