# DockArea

Panels in a tree of splits and tabs that move by a drag, by keys or by a menu, resize, collapse, close, and save their arrangement as text.

**Family:** Layout · **Rust:** `ravenrs::prelude::DockArea`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `auto_hide` | on/off | `true` |  |
| `floatable` | on/off | `true` |  |
| `free_placement` | on/off | `false` |  |
| `label` | text | `` | Empty leaves it as the component has it. |
| `movable` | on/off | `true` |  |
| `tab_bar` | Always · WhenSeveral · Never | `Always` |  |

## Stories

- **Example**: 728 × 420 pt, settings `{"auto_hide":true,"floatable":true,"free_placement":false,"label":"","movable":true,"tab_bar":"Always"}`
- **Tab bar: WhenSeveral**: 728 × 420 pt, settings `{"auto_hide":true,"floatable":true,"free_placement":false,"label":"","movable":true,"tab_bar":"WhenSeveral"}`
- **Tab bar: Never**: 728 × 420 pt, settings `{"auto_hide":true,"floatable":true,"free_placement":false,"label":"","movable":true,"tab_bar":"Never"}`

## Sizing

Fills the width of its parent: 288, 728, 1168 pt wide in parents of 320, 760, 1200 pt.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover** on "Close Folders": fill transparent → ink@7.
- **Focus** on "Folders": adds a rect (border 2.0 focus-ring).
- **Pressed** on "Close Folders": fill transparent → ink@12, rect [101.0,5.0,22.0,22.0] → [101.33,5.33,21.34,21.34].

## Behavior

- **Events**: `on_change`, `on_documents_empty`, `on_end`.
- **Keys** its first control handles once focused: `ArrowDown`, `ArrowUp`, `ArrowLeft`, `ArrowRight`, `End`.
- **Accessibility**: role `Group` "Docked panels".
- **Focus order** (Tab): Tab "Folders" → Button "Close Folders" → Tab "Search" → Button "Close Search" → Button "Scroll forward" → Button "Auto-hide Folders" → Button "Options of Folders" → Tab "Layers" → Button "Close Layers" → Button "Auto-hide Layers" → Button "Options of Layers" → Tab "History" → Button "Close History" → Button "Auto-hide History" → Button "Options of History" → Tab "Log" → Button "Close Log" → Button "Auto-hide Log" → Button "Options of Log" → Tab "Filters" → Button "Close Filters" → Button "Auto-hide Filters" → Button "Options of Filters" → Tab "Photo" → Tab "Grid" → Button "Close Grid" → Button "Options of Photo" → Slider "Resize left dock" → Slider "Resize between Layers and History" → Slider "Resize right dock" → Slider "Resize bottom dock" → Slider "Resize top dock" → Button "Hide left dock" → Button "Hide right dock" → Button "Hide top dock" → Button "Hide bottom dock".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Tab "Folders" | 115.0 × 32.0 | radius-sm | bg-100 | — | Inter 400 13.0px ink | folder 14.0px ink | space-3 |
| Example | Button "Close Folders" | 22.0 × 22.0 | radius-md | transparent | — | — | x 12.0px ink | 5.0 |
| Example | Tab "Search" | 113.0 × 32.0 | — | — | — | Inter 400 13.0px ink-muted | search 14.0px ink-muted | space-3 |
| Example | Button "Scroll forward" | 40.0 × 32.0 | 0.0 | {"end":[-16.0,-15.5],"from":"bg-000","start":[-15.0,-15.5],"to":"transparent"} | — | Inter 400 13.0px ink-muted | search 14.0px ink-muted | 5.0 |
| Example | Button "Auto-hide Folders" | 22.0 × 22.0 | radius-md | transparent | — | — | pin 12.0px ink | 5.0 |
| Example | Button "Options of Folders" | 22.0 × 22.0 | radius-md | transparent | — | — | x 12.0px ink | 15.0 |
| Example | Label "What folders shows" | 141.0 × 22.0 | — | — | — | Inter 400 15.0px ink | — | 0.0 |
| Example | Tab "Layers" | 88.0 × 32.0 | radius-sm | bg-100 | — | Inter 400 13.0px ink | x 12.0px ink | space-3 |
| Example | Button "Options of Layers" | 22.0 × 22.0 | radius-md | transparent | — | — | ellipsis 12.0px ink | 5.0 |
| Example | Tab "Filters" | 107.0 × 32.0 | radius-sm | bg-100 | — | Inter 400 13.0px ink | filter 14.0px ink | space-3 |
| Example | Tab "Photo" | 83.0 × 32.0 | radius-sm | bg-100 | — | Inter 400 13.0px ink | camera 14.0px ink | space-3 |
| Example | Tab "Grid" | 88.0 × 32.0 | — | — | — | Inter 400 13.0px ink-muted | x 12.0px ink | space-3 |
| Example | Slider "Resize left dock" | 10.0 × 420.0 | — | — | — | — | — | — |
| Example | Slider "Resize between Layers and History" | 200.0 × 10.0 | — | — | — | — | — | — |
| Example | Button "Hide left dock" | 12.0 × 420.0 | 0.0 | transparent | — | — | chevron-left 10.0px ink-muted | 1.0 |
| Example | Button "Hide right dock" | 12.0 × 420.0 | 0.0 | transparent | — | — | chevron-right 10.0px ink-muted | 1.0 |
| Example | Button "Hide top dock" | 264.0 × 12.0 | 0.0 | transparent | — | — | chevron-up 10.0px ink-muted | 127.0 |
| Example | Button "Hide bottom dock" | 264.0 × 12.0 | 0.0 | transparent | — | — | chevron-down 10.0px ink-muted | 127.0 |
| Example | Status "" | 0.0 × 0.0 | — | — | — | — | — | — |

## API

Builder methods: `auto_hide`, `center_min`, `dock_limits`, `edge_toggles`, `floatable`, `free_placement`, `height`, `label`, `movable`, `new`, `on_change`, `on_documents_empty`, `on_end`, `pane_min`, `tab_bar`.

## Example

```rust
fn dock_area() -> impl View {
    let layout = State::new(
        DockLayout::new()
            .center(DockNode::tabs(["photo", "grid"]))
            .left(DockNode::tabs(["folders", "search"]), 220.0)
            .right(DockNode::vertical([DockNode::tabs(["layers"]), DockNode::tabs(["history"])]), 200.0)
            .bottom(DockNode::tabs(["log"]), 110.0)
            .top(DockNode::tabs(["filters"]), 64.0),
    );
    let panels = vec![
        DockPanel::new("photo", "Photo").icon(IconName::Camera).closable(false),
        DockPanel::new("grid", "Grid").unsaved(true),
        DockPanel::new("folders", "Folders").icon(IconName::Folder),
        DockPanel::new("search", "Search").icon(IconName::Search),
        DockPanel::new("layers", "Layers"),
        DockPanel::new("history", "History"),
        DockPanel::new("log", "Log"),
        DockPanel::new("filters", "Filters").icon(IconName::Filter),
    ];
    DockArea::new(layout.binding(), panels, |id| -> AnyView {
        Box::new(VStack::new((Text::new(format!("What {id} shows")),)).padding(12.0))
    })
    // Keep this text, and give it to DockLayout::parse the next time.
    .on_end(|layout| println!("{}", layout.to_text()))
    .height(420.0)
}
```

Every number above is in `DockArea.json` beside this card.
