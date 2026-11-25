# Breadcrumb

Path of locations ending at the current page.

**Family:** Navigation · **Rust:** `ravenrs::prelude::Breadcrumb`

## Stories

- **Example**: 330 × 22 pt, settings `{}`

## Sizing

Hugs its content: 330 × 22 pt in a parent of 760 pt; in a parent that stretches its children, as a form column, it takes the whole width.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover** on "Workspace": fill transparent → ink@6; color ink-muted → ink.
- **Focus** on "Workspace": fill transparent → ink@6; color ink-muted → ink; adds a rect (border 2.0 focus-ring).
- **Pressed** on "Workspace": fill transparent → ink@6; color ink-muted → ink.

## Behavior

- **Events**: `on_navigate`.
- **Keys** its first control handles once focused: `Escape`.
- **Accessibility**: role `Group` "Breadcrumb".
- **Focus order** (Tab): Link "Workspace" → Link "Projects" → Link "RavenRS".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Link "Workspace" | 78.0 × 22.0 | radius-sm | transparent | — | Inter 400 13.0px ink-muted | — | space-1 |
| Example | Label "Settings" | 52.0 × 18.0 | — | — | — | Inter 600 13.0px ink | — | 0.0 |

## API

Builder methods: `new`, `on_navigate`, `root_icon`.

## Example

```rust
fn breadcrumb() -> impl View {
    // `on_navigate` makes every location but the last one a link.
    Breadcrumb::new(["Workspace", "Projects", "RavenRS", "Settings"]).on_navigate(|_level| {})
}
```

Every number above is in `Breadcrumb.json` beside this card.
