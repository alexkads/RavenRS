# SidebarBrand

The name of an app at the top of a sidebar.

**Family:** Navigation · **Rust:** `ravenrs::prelude::SidebarBrand`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `subtitle` | text | `Family finances` |  |

## Stories

- **Example**: 128 × 42 pt, settings `{"subtitle":"Family finances"}`

## Sizing

Hugs its content: 128 × 42 pt in a parent of 760 pt.

## Behavior

- **Accessibility**: role `Label` "Raven".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Label "Raven" | 45.0 × 22.0 | — | — | — | Inter 600 15.0px ink | — | 0.0 |
| Example | Label "Family finances" | 96.0 × 18.0 | — | — | — | Inter 400 13.0px ink-muted | — | 0.0 |

## API

Builder methods: `new`, `subtitle`.

## Example

```rust
fn sidebar_brand() -> impl View {
    SidebarBrand::new(Icon::new(IconName::Wallet), "Raven").subtitle("Family finances")
}
```

Every number above is in `SidebarBrand.json` beside this card.
