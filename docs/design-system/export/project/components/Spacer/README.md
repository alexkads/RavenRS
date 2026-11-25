# Spacer

Flexible space that pushes its siblings apart.

**Family:** Layout · **Rust:** `ravenrs::prelude::Spacer`

## Stories

- **Example**: 131 × 40 pt, settings `{}`

## Sizing

Hugs its content: 131 × 40 pt in a parent of 760 pt; in a parent that stretches its children, as a form column, it takes the whole width.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover** on "Next": fill accent → accent-hover.
- **Focus** on "Next": adds a rect (border 2.0 focus-ring).
- **Pressed** on "Next": fill accent → accent-pressed, rect [60.0,0.0,71.0,40.0] → [61.07,0.6,68.87,38.8].

## Behavior

- **Accessibility**: role `Label` "Left".
- **Focus order** (Tab): Button "Next".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Label "Left" | 28.0 × 22.0 | — | — | — | Inter 400 15.0px ink | — | 0.0 |
| Example | Button "Next" | 71.0 × 40.0 | radius-md | accent | — | Inter 600 15.0px on-accent | — | 18.0 |

## API

Builder methods: `min`, `new`.

## Example

```rust
fn spacer() -> impl View {
    HStack::new((Text::new("Left"), Spacer::new(), Button::new("Next"))).align(Align::Center)
}
```

Every number above is in `Spacer.json` beside this card.
