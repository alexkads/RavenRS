# HeroBanner

Wide band in a strong color, readable from across the room.

**Family:** Display and feedback · **Rust:** `ravenrs::prelude::HeroBanner`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `tone` | Neutral · Accent · Success · Warning · Danger | `Success` |  |
| `description` | text | `` | Empty leaves it as the component has it. |

## Stories

- **Example**: 340 × 148 pt, settings `{"description":"","tone":"Success"}`
- **With description**: 357 × 170 pt, settings `{"description":"Wait for the beep before you take it out.","tone":"Success"}`
- **Tone: Neutral**: 340 × 148 pt, settings `{"description":"","tone":"Neutral"}`
- **Tone: Accent**: 340 × 148 pt, settings `{"description":"","tone":"Accent"}`
- **Tone: Warning**: 340 × 148 pt, settings `{"description":"","tone":"Warning"}`
- **Tone: Danger**: 340 × 148 pt, settings `{"description":"","tone":"Danger"}`

## Sizing

Hugs its content: 340 × 148 pt in a parent of 760 pt; in a parent of 320 pt it narrows to 324 pt; in a parent that stretches its children, as a form column, it takes the whole width.

## Behavior

- **Accessibility**: role `Status` "You can remove the card".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Status "You can remove the card" | 340.0 × 68.0 | radius-lg | success | — | Inter 600 22.0px scrim | circle-check 32.0px scrim | space-4 |
| Example | Label "You can remove the card" | 264.0 × 28.0 | — | — | — | Inter 600 22.0px scrim | — | 0.0 |
| Example | Alert "Do not remove the card" | 340.0 × 68.0 | radius-lg | danger | — | Inter 600 22.0px scrim | circle-alert 32.0px scrim | space-6 |
| With description | Status "You can remove the card" | 357.0 × 90.0 | radius-lg | success | — | Inter 600 22.0px scrim | circle-check 32.0px scrim | space-4 |
| With description | Label "Wait for the beep before you take it out." | 281.0 × 22.0 | — | — | — | Inter 400 15.0px scrim | — | 0.0 |
| With description | Alert "Do not remove the card" | 357.0 × 68.0 | radius-lg | danger | — | Inter 600 22.0px scrim | circle-alert 32.0px scrim | 33.0 |
| Tone: Neutral | Status "You can remove the card" | 340.0 × 68.0 | radius-lg | ink-muted | — | Inter 600 22.0px scrim | circle-check 32.0px scrim | space-4 |
| Tone: Accent | Status "You can remove the card" | 340.0 × 68.0 | radius-lg | mix(accent, focus-ring, 35) | — | Inter 600 22.0px on-accent | circle-check 32.0px on-accent | space-4 |
| Tone: Accent | Label "You can remove the card" | 264.0 × 28.0 | — | — | — | Inter 600 22.0px on-accent | — | 0.0 |
| Tone: Warning | Alert "You can remove the card" | 340.0 × 68.0 | radius-lg | warning | — | Inter 600 22.0px scrim | circle-check 32.0px scrim | space-4 |
| Tone: Danger | Alert "You can remove the card" | 340.0 × 68.0 | radius-lg | danger | — | Inter 600 22.0px scrim | circle-check 32.0px scrim | space-4 |

## API

Builder methods: `action`, `color`, `description`, `icon`, `new`, `tone`.

## Example

```rust
fn hero_banner() -> impl View {
    VStack::new((
        HeroBanner::new("You can remove the card").icon(IconName::CircleCheck).tone(Tone::Success),
        HeroBanner::new("Do not remove the card").icon(IconName::CircleAlert).tone(Tone::Danger),
    ))
    .spacing(12.0)
    .align(Align::Stretch)
}
```

Every number above is in `HeroBanner.json` beside this card.
