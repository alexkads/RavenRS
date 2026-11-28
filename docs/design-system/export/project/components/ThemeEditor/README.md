# ThemeEditor

Changes the tokens of the theme on screen, with live contrast and JSON to copy and paste.

**Family:** Actions and inputs · **Rust:** `ravenrs::prelude::ThemeEditor`

## Stories

- **Example**: 572 × 1458 pt, settings `{}`

## Sizing

Hugs its content: 572 × 1458 pt in a parent of 760 pt; in a parent of 320 pt it narrows to 529 pt; in a parent that stretches its children, as a form column, it takes the whole width; its height follows the width, as its content wraps.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover** on "Theme": fill bg-300 → mix(ink, bg-300, 6).
- **Focus** on "Theme": adds a rect (border 2.0 focus-ring).
- **Pressed** on "Theme": border {color:border,width:1.0} → {color:border,width:0.97}, fill bg-300 → mix(ink, bg-300, 12), rect [0.0,0.0,112.0,32.0] → [1.68,0.48,108.64,31.04].

## Behavior

- **Keys** its first control handles once focused: `Enter`, `Space`, `Escape`.
- **Accessibility**: role `Button` "Theme".
- **Focus order** (Tab): Button "Theme" → Button "Copy tokens" → Button "Paste tokens" → ComboBox "bg-000" → ComboBox "bg-100" → ComboBox "bg-200" → ComboBox "bg-300" → ComboBox "border" → ComboBox "control-border" → ComboBox "ink" → ComboBox "ink-muted" → ComboBox "accent" → ComboBox "accent-hover" → ComboBox "accent-pressed" → ComboBox "on-accent" → ComboBox "focus-ring" → ComboBox "success" → ComboBox "warning" → ComboBox "danger" → ComboBox "chart-2" → ComboBox "chart-3" → ComboBox "chart-4" → ComboBox "chart-5" → SpinButton "radius-sm" → Button "Decrease" → Button "Increase" → SpinButton "radius-md" → Button "Decrease" → Button "Increase" → SpinButton "radius-lg" → Button "Decrease" → Button "Increase" → SpinButton "radius-xl" → Button "Decrease" → Button "Increase" → SpinButton "metric" → Button "Decrease" → Button "Increase" → SpinButton "display" → Button "Decrease" → Button "Increase" → SpinButton "title" → Button "Decrease" → Button "Increase" → SpinButton "headline" → Button "Decrease" → Button "Increase" → SpinButton "body" → Button "Decrease" → Button "Increase" → SpinButton "callout" → Button "Decrease" → Button "Increase" → SpinButton "caption" → Button "Decrease" → Button "Increase".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Button "Theme" | 112.0 × 32.0 | radius-md | bg-300 | 1.0 border | Inter 600 13.0px ink | moon 14.0px ink | space-3 |
| Example | Label "No changes" | 74.0 × 18.0 | — | — | — | Inter 400 13.0px ink-muted | — | 0.0 |
| Example | Button "Restore" | 73.0 × 32.0 | radius-md | transparent | — | Inter 600 13.0px ink@45 | — | space-3 |
| Example | Button "Copy tokens" | 125.0 × 32.0 | radius-md | bg-300 | 1.0 border | Inter 600 13.0px ink | copy 14.0px ink | space-3 |
| Example | Button "Paste tokens" | 105.0 × 32.0 | radius-md | bg-300 | 1.0 border | Inter 600 13.0px ink | — | space-3 |
| Example | Label "Surfaces" | 572.0 × 24.0 | — | — | — | Inter 600 17.0px ink | — | 0.0 |
| Example | Label "bg-000" | 49.0 × 18.0 | — | — | — | Inter 600 13.0px ink | — | 0.0 |
| Example | ComboBox "bg-000" | 180.0 × 40.0 | radius-md | bg-300 | 1.0 border | Inter 400 15.0px ink | chevron-down 16.0px ink-muted | 40.0 |
| Example | SpinButton "radius-sm" | 110.0 × 38.0 | radius-md | bg-300 | 1.0 control-border | Inter 600 15.0px ink | minus 14.0px ink | space-3 |
| Example | Button "Decrease" | 32.0 × 32.0 | radius-md | transparent | — | — | minus 14.0px ink | 9.0 |
| Example | Button "Increase" | 32.0 × 32.0 | radius-md | transparent | — | — | plus 14.0px ink | 9.0 |
| Example | SpinButton "radius-xl" | 110.0 × 38.0 | — | — | — | — | — | — |
| Example | Button "Decrease" | 32.0 × 32.0 | — | — | — | — | — | — |
| Example | Label "Text sizes" | 572.0 × 24.0 | — | — | — | — | — | — |
| Example | Label "metric" | 41.0 × 18.0 | — | — | — | — | — | — |

## API

Builder methods: `new`, `theme`.

## Example

```rust
fn theme_editor() -> impl View {
    ThemeEditor::new()
}
```

Every number above is in `ThemeEditor.json` beside this card.
