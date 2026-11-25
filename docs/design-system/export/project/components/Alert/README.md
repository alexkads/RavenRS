# Alert

A message about the page, with a tone, an icon and an optional title.

**Family:** Display and feedback · **Rust:** `ravenrs::prelude::Alert`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `title` | text | `Deployment finished` | Leave it empty when it would only repeat where it is. |
| `message` | text | `The site is live.` |  |
| `tone` | Neutral · Accent · Success · Warning · Danger | `Success` |  |
| `compact` | on/off | `false` |  |
| `action` | text | `` | A button at the end that says what to do; empty shows none. |

## Stories

- **Success**: 728 × 70 pt, settings `{"action":"","compact":false,"message":"The site is live.","title":"Deployment finished","tone":"Success"}`
- **Danger**: 728 × 70 pt, settings `{"action":"","compact":false,"message":"Update your card.","title":"Payment failed","tone":"Danger"}`
- **With an action**: 728 × 70 pt, settings `{"action":"Go to line 13","compact":false,"message":"Line 13 runs free Groovy, which the server does not run.","title":"1 part not converted","tone":"Danger"}`
- **Message only**: 728 × 24 pt, settings `{"action":"","compact":true,"message":"Moving deletes the originals.","title":"","tone":"Warning"}`
- **All tones**: 179 × 382 pt, settings `{"action":"","compact":false,"message":"The site is live.","title":"Deployment finished","tone":"Success"}`

## Sizing

Fills the width of its parent: 288, 728, 1168 pt wide in parents of 320, 760, 1200 pt.

- **With an action**: Fills the width of its parent: 288, 728, 1168 pt wide in parents of 320, 760, 1200 pt; its height follows the width, as its content wraps.
- **All tones**: Hugs its content: 179 × 382 pt in a parent of 760 pt; in a parent that stretches its children, as a form column, it takes the whole width.

## Behavior

- **Accessibility**: role `Status`.

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Success | Status | 728.0 × 70.0 | radius-lg | success@10 | 1.0 success@30 | Inter 600 15.0px ink | circle-check 18.0px success | space-4 |
| Success | Label "Deployment finished" | 150.0 × 22.0 | — | — | — | Inter 600 15.0px ink | — | 0.0 |
| Success | Label "The site is live." | 92.0 × 18.0 | — | — | — | Inter 400 13.0px ink-muted | — | 0.0 |
| Danger | Alert | 728.0 × 70.0 | radius-lg | danger@10 | 1.0 danger@30 | Inter 600 15.0px ink | circle-alert 18.0px danger | space-4 |
| With an action | Button "Go to line 13" | 101.0 × 32.0 | radius-md | bg-300 | 1.0 border | Inter 600 13.0px ink | — | space-3 |
| Message only | Status | 728.0 × 24.0 | radius-sm | warning@15 | — | Inter 600 12.0px mix(ink, warning, 30) | triangle-alert 14.0px warning | 10.0 |
| Message only | Label "Moving deletes the originals." | 179.0 × 16.0 | — | — | — | Inter 600 12.0px mix(ink, warning, 30) | — | 0.0 |
| All tones | Status | 179.0 × 70.0 | radius-lg | ink-muted@10 | 1.0 ink-muted@30 | Inter 600 15.0px ink | info 18.0px ink-muted | space-4 |
| All tones | Status | 179.0 × 70.0 | radius-lg | #8196f91a | 1.0 #8196f94d | Inter 600 15.0px ink | info 18.0px mix(accent, focus-ring, 35) | space-4 |
| All tones | Status | 179.0 × 70.0 | radius-lg | warning@10 | 1.0 warning@30 | Inter 600 15.0px ink | triangle-alert 18.0px warning | space-4 |

## API

Builder methods: `action`, `compact`, `message`, `new`, `tone`.

## Example

```rust
fn alert() -> impl View {
    VStack::new((
        Alert::new("Deployment finished", "The site is live.").tone(Tone::Success),
        Alert::new("Payment failed", "Update your card.").tone(Tone::Danger),
        Alert::message("Moving deletes the originals.").tone(Tone::Warning).compact(),
    ))
    .spacing(12.0)
    .align(Align::Stretch)
}
```

Every number above is in `Alert.json` beside this card.
