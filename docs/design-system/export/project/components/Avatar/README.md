# Avatar

A person's initials or picture, in a circle or a rounded square.

**Family:** Display and feedback · **Rust:** `ravenrs::prelude::Avatar`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `name` | text | `Ana Lima` | The initials come from it. |
| `size` | number 20–96 by 4 | `32` |  |
| `shape` | Circle · Rounded | `Circle` | A circle, or a square with radius-md corners as a profile picture is often shown. |
| `picture` | on/off | `false` | A picture in place of the initials, cropped to the shape. |

## Stories

- **Default**: 32 × 32 pt, settings `{"name":"Ana Lima","picture":false,"shape":"Circle","size":32.0}`
- **Large**: 44 × 44 pt, settings `{"name":"Carla Dias","picture":false,"shape":"Circle","size":44.0}`
- **Shape: Rounded**: 32 × 32 pt, settings `{"name":"Ana Lima","picture":false,"shape":"Rounded","size":32.0}`
- **Rounded, with a picture**: 32 × 32 pt, settings `{"name":"Ana Lima","picture":true,"shape":"Rounded","size":32.0}`

## Sizing

Hugs its content: 32 × 32 pt in a parent of 760 pt.

## Behavior

- **Accessibility**: role `Image` "Ana Lima".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Default | Image "Ana Lima" | 32.0 × 32.0 | radius-full | focus-ring@28 | 1.0 focus-ring@50 | Inter 600 13.0px mix(ink, focus-ring, 50) | — | space-2 |
| Large | Image "Carla Dias" | 44.0 × 44.0 | radius-full | success@28 | 1.0 success@50 | Inter 600 18.0px mix(ink, success, 50) | — | 9.0 |
| Shape: Rounded | Image "Ana Lima" | 32.0 × 32.0 | radius-md | focus-ring@28 | 1.0 focus-ring@50 | Inter 600 13.0px mix(ink, focus-ring, 50) | — | space-2 |
| Rounded, with a picture | Image "Ana Lima" | 32.0 × 32.0 | radius-md | transparent | — | — | — | — |

## API

Builder methods: `border`, `image`, `new`, `shape`, `size`.

`AvatarShape`: `Circle`, `Rounded`.

## Example

```rust
fn avatar() -> impl View {
    HStack::new((
        Avatar::new("Ana Lima"),
        Avatar::new("Bruno Reis").border(2.0, Semantic::Accent),
        Avatar::new("Carla Dias").size(44.0).image(Image::new(picture(1.0))),
    ))
    .spacing(8.0)
    .align(Align::Center)
}
```

Every number above is in `Avatar.json` beside this card.
