# Toaster

Stacked notifications that dismiss themselves.

**Family:** Overlays · **Rust:** `ravenrs::prelude::Toaster`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `position` | TopLeft · TopCenter · TopRight · BottomLeft · BottomCenter · BottomRight | `BottomRight` |  |
| `bottom_inset` | number 0–200 by 1 | `0` | Room left at the foot, as for a status bar. |
| `showing` | on/off | `true` | A notice is up, so the story shows where it sits. |

## Stories

- **Example**: 728 × 1068 pt, settings `{"bottom_inset":0.0,"position":"BottomRight","showing":true}`
- **Over a status bar**: 728 × 1040 pt, settings `{"bottom_inset":28.0,"position":"BottomRight","showing":true}`
- **Position: TopLeft**: 360 × 70 pt, settings `{"bottom_inset":0.0,"position":"TopLeft","showing":true}`
- **Position: TopCenter**: 544 × 70 pt, settings `{"bottom_inset":0.0,"position":"TopCenter","showing":true}`
- **Position: TopRight**: 728 × 70 pt, settings `{"bottom_inset":0.0,"position":"TopRight","showing":true}`
- **Position: BottomLeft**: 360 × 1068 pt, settings `{"bottom_inset":0.0,"position":"BottomLeft","showing":true}`
- **Position: BottomCenter**: 544 × 1068 pt, settings `{"bottom_inset":0.0,"position":"BottomCenter","showing":true}`

## Sizing

Hugs its content: 728 × 1068 pt in a parent of 760 pt; in a parent of 320 pt it narrows to 340 pt.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover** on "Show toast": fill accent → accent-hover.
- **Focus** on "Show toast": adds a rect (border 2.0 focus-ring).
- **Pressed** on "Show toast": fill accent → accent-pressed, rect [0.0,0.0,117.0,40.0] → [1.75,0.6,113.49,38.8].

## Behavior

- **Keys** its first control handles once focused: `Enter`, `Space`.
- **Accessibility**: role `Button` "Show toast".
- **Focus order** (Tab): Button "Show toast" → Button "Dismiss".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Button "Show toast" | 117.0 × 40.0 | radius-md | accent | — | Inter 600 15.0px on-accent | — | 18.0 |
| Example | Status | 360.0 × 70.0 | radius-lg | bg-200 | 1.0 border | Inter 600 15.0px ink | circle-check 18.0px success | space-4 |
| Example | Label "Report exported" | 118.0 × 22.0 | — | — | — | Inter 600 15.0px ink | — | 0.0 |
| Example | Label "sales-q3.pdf is ready." | 134.0 × 18.0 | — | — | — | Inter 400 13.0px ink-muted | — | 0.0 |
| Example | Button "Dismiss" | 32.0 × 32.0 | radius-md | transparent | — | — | x 14.0px ink | 9.0 |
| Position: TopLeft | Button "Show toast" | 117.0 × 40.0 | radius-md | accent | — | Inter 600 15.0px on-accent | circle-check 18.0px success | space-4 |
| Position: TopLeft | Status | 360.0 × 70.0 | radius-lg | bg-200 | 1.0 border | Inter 600 15.0px on-accent | circle-check 18.0px success | space-4 |

## API

Builder methods: `bottom_inset`, `new`, `position`.

## Example

```rust
fn toaster() -> impl View {
    let toasts = Toasts::new();
    let show = toasts.clone();
    VStack::new((
        Button::new("Show toast")
            .on_click(move || show.show("Report exported", "sales-q3.pdf is ready.", Tone::Success)),
        // `toasts.show_until_closed` keeps one up until it is closed.
        Toaster::new(toasts).position(ToastPosition::BottomRight),
    ))
}
```

Every number above is in `Toaster.json` beside this card.
