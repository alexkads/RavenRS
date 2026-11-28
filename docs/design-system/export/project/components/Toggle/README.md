# Toggle

On and off switch for a setting that applies at once.

**Family:** Actions and inputs · **Rust:** `ravenrs::prelude::Toggle`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `label` | text | `Push notifications` |  |
| `on` | on/off | `true` |  |
| `small` | on/off | `false` | The compact switch, for lists and toolbars. |
| `accessible_name` | text | `` | What a screen reader calls it, with no text shown. |
| `disabled` | on/off | `false` |  |
| `read_only` | on/off | `false` |  |

## Stories

- **On**: 178 × 24 pt, settings `{"accessible_name":"","disabled":false,"label":"Push notifications","on":true,"read_only":false,"small":false}`
- **Off**: 140 × 24 pt, settings `{"accessible_name":"","disabled":false,"label":"Email digest","on":false,"read_only":false,"small":false}`
- **Small**: 170 × 22 pt, settings `{"accessible_name":"","disabled":false,"label":"Push notifications","on":true,"read_only":false,"small":true}`
- **Accessible name only**: 32 × 18 pt, settings `{"accessible_name":"Turn docker on","disabled":false,"label":"","on":true,"read_only":false,"small":true}`
- **Disabled**: 178 × 24 pt, settings `{"accessible_name":"","disabled":true,"label":"Push notifications","on":true,"read_only":false,"small":false}`
- **Read only**: 178 × 24 pt, settings `{"accessible_name":"","disabled":false,"label":"Push notifications","on":true,"read_only":true,"small":false}`

## Sizing

Hugs its content: 178 × 24 pt in a parent of 760 pt.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story On:

- **Hover** on "Push notifications": fill accent → accent-hover.
- **Focus** on "Push notifications": adds a rect (border 2.0 focus-ring).
- **Pressed** on "Push notifications": fill accent → accent-hover; rect [19.0,3.0,18.0,18.0] → [15.0,3.0,22.0,18.0].

## Behavior

- **Keys** its first control handles once focused: `Enter`, `Space`.
- **Accessibility**: role `Switch` "Push notifications".
- **Focus order** (Tab): Switch "Push notifications".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| On | Switch "Push notifications" | 177.0 × 24.0 | — | — | — | Inter 400 15.0px ink | — | 50.0 |
| Small | Switch "Push notifications" | 169.0 × 22.0 | — | — | — | Inter 400 15.0px ink | — | 42.0 |
| Accessible name only | Switch "Turn docker on" | 32.0 × 18.0 | radius-full | accent | — | — | — | — |
| Disabled | Switch "Push notifications" | 177.0 × 24.0 | — | — | — | Inter 400 15.0px ink@45 | — | 50.0 |

## API

Builder methods: `accessible_name`, `disabled`, `label`, `new`, `read_only`, `small`.

## Example

```rust
fn toggle() -> impl View {
    let push = State::new(true);
    let email = State::new(false);
    VStack::new((
        Toggle::new(push.binding()).label("Push notifications"),
        Toggle::new(email.binding()).label("Email digest").small(),
    ))
    .spacing(12.0)
}
```

Every number above is in `Toggle.json` beside this card.
