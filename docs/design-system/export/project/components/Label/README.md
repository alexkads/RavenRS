# Label

Form label with an optional required marker and hint.

**Family:** Forms · **Rust:** `ravenrs::prelude::Label`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `hint` | text | `` | Empty leaves it as the component has it. |
| `required` | on/off | `false` |  |

## Stories

- **Example**: 99 × 18 pt, settings `{"hint":"","required":false}`

## Sizing

Hugs its content: 99 × 18 pt in a parent of 760 pt.

## Behavior

- **Accessibility**: role `Label` "Company name".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Label "Company name" | 99.0 × 18.0 | — | — | — | Inter 600 13.0px ink | — | 0.0 |

## API

Builder methods: `hint`, `ink`, `new`, `required`, `text`.

## Example

```rust
fn label() -> impl View {
    Label::new("Company name")
}
```

Every number above is in `Label.json` beside this card.
