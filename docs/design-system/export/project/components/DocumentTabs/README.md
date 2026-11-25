# DocumentTabs

Tabs for open documents that close, move, rename and say what was not saved.

**Family:** Navigation · **Rust:** `ravenrs::prelude::DocumentTabs`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `label` | text | `` | Empty leaves it as the component has it. |

## Stories

- **Example**: 728 × 34 pt, settings `{"label":""}`

## Sizing

Fills the width of its parent: 288, 728, 1168 pt wide in parents of 320, 760, 1200 pt.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover** on "Close tab": fill transparent → ink@7.
- **Focus** on "Wedding": adds a rect (border 2.0 focus-ring).
- **Pressed** on "Close tab": fill transparent → ink@12, rect [98.0,6.0,22.0,22.0] → [98.33,6.33,21.34,21.34].

## Behavior

- **Events**: `on_close`, `on_new`, `on_rename`, `on_reorder`.
- **Keys** its first control handles once focused: `ArrowDown`, `ArrowUp`, `ArrowLeft`, `ArrowRight`, `End`.
- **Accessibility**: role `TabList` "Open documents".
- **Focus order** (Tab): Tab "Wedding" → Button "Close tab" → Tab "Portraits" → Button "Close tab" → Tab "Studio with a long name that does not fit" → Button "Close tab" → Tab "Home" → Button "New tab".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Tab "Wedding" | 128.0 × 34.0 | radius-sm | bg-100 | — | Inter 400 13.0px ink | camera 14.0px ink | space-3 |
| Example | Button "Close tab" | 22.0 × 22.0 | radius-md | transparent | — | — | x 12.0px ink | 5.0 |
| Example | Tab "Portraits" | 118.0 × 34.0 | radius-sm | transparent | — | Inter 400 13.0px ink-muted | x 12.0px ink | space-3 |
| Example | Image "Unsaved changes" | 7.0 × 7.0 | radius-full | mix(accent, focus-ring, 35) | — | — | — | — |
| Example | Tab "Home" | 62.0 × 34.0 | radius-sm | transparent | — | Inter 400 13.0px ink-muted | — | space-3 |
| Example | Button "New tab" | 32.0 × 32.0 | radius-md | transparent | — | — | plus 14.0px ink | 9.0 |
| Example | Status "" | 0.0 × 0.0 | — | — | — | — | — | — |

## API

Builder methods: `fade`, `label`, `menu`, `new`, `on_close`, `on_new`, `on_rename`, `on_reorder`.

## Example

```rust
fn document_tabs() -> impl View {
    let tabs = State::new(vec![
        DocumentTab::new(1, "Wedding").icon(IconName::Camera),
        // ravenrs-lint: allow(raw_color) the tab keeps the color its owner picked for that document
        DocumentTab::new(2, "Portraits").unsaved(true).color(Color::rgb(0.25, 0.55, 0.95)),
        DocumentTab::new(3, "Studio with a long name that does not fit"),
        DocumentTab::new(4, "Home").closable(false),
    ]);
    let active = State::new(1_u64);
    view_fn(move || {
        let (closed, moved, renamed) = (tabs.clone(), tabs.clone(), tabs.clone());
        DocumentTabs::new(tabs.get(), active.binding())
            .on_close(move |id| closed.update(|list| list.retain(|tab| tab.id != id)))
            .on_reorder(move |from, to| {
                moved.update(|list| {
                    let tab = list.remove(from);
                    list.insert(to, tab);
                });
            })
            .on_rename(move |id, name| {
                renamed.update(|list| {
                    if let Some(tab) = list.iter_mut().find(|tab| tab.id == id) {
                        tab.title = name;
                    }
                });
            })
            .on_new(|| {})
            .fade(Semantic::SurfaceElevated)
    })
}
```

Every number above is in `DocumentTabs.json` beside this card.
