# SplitButton

A main command with a menu of alternatives.

**Family:** Actions and inputs · **Rust:** `ravenrs::prelude::SplitButton`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `disabled` | on/off | `false` |  |
| `size` | Small · Medium · Large · Mini | `Medium` |  |
| `style` | Primary · Secondary · Ghost · Danger | `Primary` |  |

## Stories

- **Example**: 137 × 40 pt, settings `{"disabled":false,"size":"Medium","style":"Primary"}`
- **Disabled**: 137 × 40 pt, settings `{"disabled":true,"size":"Medium","style":"Primary"}`
- **Size: Small**: 110 × 32 pt, settings `{"disabled":false,"size":"Small","style":"Primary"}`
- **Size: Large**: 157 × 48 pt, settings `{"disabled":false,"size":"Large","style":"Primary"}`
- **Size: Mini**: 86 × 22 pt, settings `{"disabled":false,"size":"Mini","style":"Primary"}`
- **Style: Secondary**: 137 × 40 pt, settings `{"disabled":false,"size":"Medium","style":"Secondary"}`
- **Style: Ghost**: 137 × 40 pt, settings `{"disabled":false,"size":"Medium","style":"Ghost"}`
- **Style: Danger**: 137 × 40 pt, settings `{"disabled":false,"size":"Medium","style":"Danger"}`

## Sizing

Hugs its content: 137 × 40 pt in a parent of 760 pt.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover** on "Save": fill accent → accent-hover.
- **Focus** on "Save": adds a rect (border 2.0 focus-ring).
- **Pressed** on "Save": fill accent → accent-pressed, rect [0.0,0.0,96.0,40.0] → [1.44,0.6,93.12,38.8].

## Behavior

- **Events**: `on_click`.
- **Accessibility**: role `Button` "Save".
- **Focus order** (Tab): Button "Save" → Button "Save: More".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Button "Save" | 96.0 × 40.0 | radius-md | accent | — | Inter 600 15.0px on-accent | check 16.0px on-accent | 18.0 |
| Example | Button "Save: More" | 40.0 × 40.0 | radius-md | accent | — | — | chevron-down 16.0px on-accent | space-3 |
| Disabled | Button "Save" | 96.0 × 40.0 | radius-md | accent@45 | — | Inter 600 15.0px on-accent@45 | check 16.0px on-accent@45 | 18.0 |
| Disabled | Button "Save: More" | 40.0 × 40.0 | radius-md | accent@45 | — | — | chevron-down 16.0px on-accent@45 | space-3 |
| Size: Small | Button "Save" | 77.0 × 32.0 | radius-md | accent | — | Inter 600 13.0px on-accent | check 14.0px on-accent | space-3 |
| Size: Small | Button "Save: More" | 32.0 × 32.0 | radius-md | accent | — | — | chevron-down 14.0px on-accent | 9.0 |
| Size: Large | Button "Save" | 108.0 × 48.0 | radius-md | accent | — | Inter 600 15.0px on-accent | check 16.0px on-accent | space-6 |
| Size: Large | Button "Save: More" | 48.0 × 48.0 | radius-md | accent | — | — | chevron-down 16.0px on-accent | space-4 |
| Size: Mini | Button "Save" | 63.0 × 22.0 | radius-md | accent | — | Inter 600 12.0px on-accent | check 12.0px on-accent | space-2 |
| Size: Mini | Button "Save: More" | 22.0 × 22.0 | radius-md | accent | — | — | chevron-down 12.0px on-accent | 5.0 |
| Style: Secondary | Button "Save" | 96.0 × 40.0 | radius-md | bg-300 | 1.0 border | Inter 600 15.0px ink | check 16.0px ink | 18.0 |
| Style: Secondary | Button "Save: More" | 40.0 × 40.0 | radius-md | bg-300 | 1.0 border | — | chevron-down 16.0px ink | space-3 |
| Style: Ghost | Button "Save" | 96.0 × 40.0 | radius-md | transparent | — | Inter 600 15.0px ink | check 16.0px ink | 18.0 |
| Style: Ghost | Button "Save: More" | 40.0 × 40.0 | radius-md | transparent | — | — | chevron-down 16.0px ink | space-3 |
| Style: Danger | Button "Save" | 96.0 × 40.0 | radius-md | danger | — | Inter 600 15.0px on-accent | check 16.0px on-accent | 18.0 |
| Style: Danger | Button "Save: More" | 40.0 × 40.0 | radius-md | danger | — | — | chevron-down 16.0px on-accent | space-3 |

## API

Builder methods: `disabled`, `icon`, `item`, `new`, `on_click`, `size`, `style`.

## Example

```rust
fn split_button() -> impl View {
    SplitButton::new("Save")
        .icon(IconName::Check)
        .on_click(|| {})
        .item(MenuItem::new("Save and close").on_select(|| {}))
        .item(MenuItem::new("Save and new").on_select(|| {}))
}
```

Every number above is in `SplitButton.json` beside this card.
