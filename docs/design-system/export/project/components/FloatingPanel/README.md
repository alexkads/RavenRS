# FloatingPanel

A panel that floats over the app in a corner, for an assistant or a chat.

**Family:** Forms · **Rust:** `ravenrs::prelude::FloatingPanel`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `title` | text | `Assistant` |  |
| `open` | on/off | `true` | Closed, it is a button in the corner. |
| `minimized` | on/off | `false` | Only its title bar. |
| `width` | number 240–640 by 10 | `400` |  |
| `height` | number 200–800 by 10 | `520` |  |

## Stories

- **Open**: 729 × 1069 pt, settings `{"height":520.0,"minimized":false,"open":true,"title":"Assistant","width":400.0}`
- **Closed**: 729 × 1069 pt, settings `{"height":520.0,"minimized":false,"open":false,"title":"Assistant","width":400.0}`
- **Minimized**: 729 × 1069 pt, settings `{"height":520.0,"minimized":true,"open":true,"title":"Assistant","width":400.0}`

## Sizing

Hugs its content: 729 × 1069 pt in a parent of 760 pt; in a parent of 320 pt it narrows to 397 pt.

- **Closed**: Fills the width of its parent: 289, 729, 1169 pt wide in parents of 320, 760, 1200 pt.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Open:

- **Hover** on "Minimize": fill transparent → ink@7.
- **Focus** on "Minimize": adds a rect (border 2.0 focus-ring).
- **Pressed** on "Minimize": fill transparent → ink@12, rect [636.0,501.0,32.0,32.0] → [636.48,501.48,31.04,31.04].

## Behavior

- **Keys** its first control handles once focused: `Enter`, `Space`, `Escape`.
- **Accessibility**: role `Group` "Assistant".
- **Focus order** (Tab): Button "Minimize" → Button "Close" → Button "Assistant".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Open | Label "Assistant" | 77.0 × 24.0 | — | — | — | Inter 600 17.0px ink | — | 0.0 |
| Open | Button "Minimize" | 32.0 × 32.0 | radius-md | transparent | — | — | minus 14.0px ink | 9.0 |
| Open | Button "Close" | 32.0 × 32.0 | radius-md | transparent | — | — | x 14.0px ink | 9.0 |
| Open | Label "The app stays usable behind this panel." | 368.0 × 22.0 | — | — | — | Inter 400 15.0px ink | — | 0.0 |
| Open | Button "Assistant" | 60.0 × 60.0 | — | — | — | — | sparkles 16.0px accent | 22.0 |
| Minimized | Button "Restore" | 32.0 × 32.0 | radius-md | transparent | — | — | maximize-2 14.0px ink | 9.0 |

## API

Builder methods: `bottom_inset`, `minimized`, `movable`, `new`, `size`, `summary`.

## Example

```rust
fn floating_panel() -> impl View {
    /// Keeps whether it is open, here while the page rebuilds.
    struct Corner;

    impl View for Corner {
        fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
            let open = cx.use_state(|| false);
            // `.movable` and `.minimized` take bindings, for the app to remember them.
            cx.child(&FloatingPanel::new(open.binding(), "Assistant", || {
                Box::new(Text::new("The app stays usable behind this panel."))
            }))
        }
    }
    Corner
}
```

Every number above is in `FloatingPanel.json` beside this card.
