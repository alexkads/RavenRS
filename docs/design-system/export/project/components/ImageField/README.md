# ImageField

A picture chosen by the person.

**Family:** Actions and inputs · **Rust:** `ravenrs::prelude::ImageField`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `label` | text | `Photo` |  |
| `disabled` | on/off | `false` |  |
| `read_only` | on/off | `false` |  |

## Stories

- **Example**: 180 × 160 pt, settings `{"disabled":false,"label":"Photo","read_only":false}`
- **Disabled**: 180 × 160 pt, settings `{"disabled":true,"label":"Photo","read_only":false}`
- **Read only**: 180 × 120 pt, settings `{"disabled":false,"label":"Photo","read_only":true}`

## Sizing

Hugs its content: 180 × 160 pt in a parent of 760 pt.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover** on "Choose image": fill bg-300 → mix(ink, bg-300, 6).
- **Focus** on "Choose image": adds a rect (border 2.0 focus-ring).
- **Pressed** on "Choose image": border {color:border,width:1.0} → {color:border,width:0.97}, fill bg-300 → mix(ink, bg-300, 12), rect [0.0,128.0,114.0,32.0] → [1.71,128.48,110.58,31.04].

## Behavior

- **Accessibility**: role `Image` "Photo".
- **Focus order** (Tab): Button "Choose image".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Image "Photo" | 180.0 × 120.0 | radius-md | bg-300 | 1.0 border | — | image 28.0px ink-muted | 76.0 |
| Example | Button "Choose image" | 114.0 × 32.0 | radius-md | bg-300 | 1.0 border | Inter 600 13.0px ink | — | space-3 |
| Disabled | Image "Photo" | 180.0 × 120.0 | radius-md | bg-300@45 | 1.0 border@45 | — | image 28.0px ink-muted@45 | 76.0 |
| Disabled | Button "Choose image" | 114.0 × 32.0 | radius-md | bg-300@45 | 1.0 border | Inter 600 13.0px ink@45 | — | space-3 |
| Read only | Image "Photo" | 180.0 × 120.0 | radius-md | bg-300@50 | 1.0 border | — | image 28.0px ink-muted | 76.0 |

## API

Builder methods: `disabled`, `label`, `most_bytes`, `new`, `read_only`, `size`.

## Example

```rust
fn image_field() -> impl View {
    let photo = State::new(None::<PickedFile>);
    ImageField::new(photo.binding()).size(180.0, 120.0).label("Photo")
}
```

Every number above is in `ImageField.json` beside this card.
