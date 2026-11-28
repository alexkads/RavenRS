# Footer

Band that ends a page or app, with left, center and right areas.

**Family:** Layout · **Rust:** `ravenrs::prelude::Footer`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `stacked` | on/off | `false` |  |

## Stories

- **Example**: 728 × 57 pt, settings `{"stacked":false}`
- **Stacked**: 728 × 111 pt, settings `{"stacked":true}`

## Sizing

Fills the width of its parent: 288, 728, 1168 pt wide in parents of 320, 760, 1200 pt; its height follows the width, as its content wraps.

- **Stacked**: Fills the width of its parent: 288, 728, 1168 pt wide in parents of 320, 760, 1200 pt.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover** on "Privacy": fill transparent → ink@7.
- **Focus** on "Privacy": adds a rect (border 2.0 focus-ring).
- **Pressed** on "Privacy": fill transparent → ink@12, rect [258.0,13.0,71.0,32.0] → [259.07,13.48,68.87,31.04].

## Behavior

- **Accessibility**: role `ContentInfo`.
- **Focus order** (Tab): Button "Privacy" → Button "Terms" → Button "Help".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Label "© 2026 Raven Finance" | 141.0 × 16.0 | — | — | — | Inter 600 12.0px ink-muted | — | 0.0 |
| Example | Button "Privacy" | 71.0 × 32.0 | radius-md | transparent | — | Inter 600 13.0px ink | — | space-3 |
| Example | Label "Online" | 40.0 × 16.0 | — | — | — | Inter 600 12.0px mix(success, ink, 70) | — | 0.0 |

## API

Builder methods: `breakpoint`, `center`, `divider`, `left`, `link`, `new`, `right`, `stacked`, `text`.

## Example

```rust
fn footer() -> impl View {
    Footer::new()
        .left((Footer::text("© 2026 Raven Finance"),))
        .center((Footer::link("Privacy"), Footer::link("Terms"), Footer::link("Help")))
        .right((Footer::text("v27.1"), Badge::new("Online").tone(Tone::Success)))
}
```

Every number above is in `Footer.json` beside this card.
