# TreeView

Items inside items: opens, selects, checks and filters.

**Family:** Data · **Rust:** `ravenrs::prelude::TreeView`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `filter` | text | `` | Empty leaves it as the component has it. |
| `label` | text | `` | Empty leaves it as the component has it. |
| `row_height` | number 0–128 by 1 | `32` |  |

## Stories

- **Example**: 728 × 160 pt, settings `{"filter":"","label":"","row_height":32.0}`

## Sizing

Fills the width of its parent: 288, 728, 1168 pt wide in parents of 320, 760, 1200 pt.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover** on "Tree": fill transparent → ink@5.
- **Focus** on "Tree": border null → {color:accent,width:1.5}; adds a rect (border 2.0 focus-ring).
- **Pressed** on "Tree": border null → {color:accent,width:1.5}; fill transparent → ink@5.

## Behavior

- **Events**: `on_expand`, `on_open`.
- **Keys** its first control handles once focused: `ArrowDown`, `ArrowUp`, `ArrowLeft`, `Escape`, `Home`, `End`.
- **Accessibility**: role `Tree` "Tree".
- **Focus order** (Tab): Tree "Tree".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | TreeItem "Sales" | 728.0 × 32.0 | radius-sm | transparent | — | Inter 400 15.0px ink | chevron-down 16.0px ink-muted | 6.0 |
| Example | TreeItem "Invoices" | 728.0 × 32.0 | radius-sm | accent@14 | — | Inter 400 15.0px ink | file-text 16.0px ink-muted | 50.0 |
| Example | TreeItem "Quotes" | 728.0 × 32.0 | radius-sm | transparent | — | Inter 400 15.0px ink | file-text 16.0px ink-muted | 50.0 |
| Example | TreeItem "Stock" | 728.0 × 32.0 | radius-sm | transparent | — | Inter 400 15.0px ink | chevron-right 16.0px ink-muted | 6.0 |
| Example | TreeItem "Reports" | 728.0 × 32.0 | radius-sm | transparent | — | Inter 400 15.0px ink | file-text 16.0px ink-muted | 30.0 |

## API

Builder methods: `checked`, `context_menu`, `expanded`, `filter`, `height`, `label`, `new`, `on_expand`, `on_open`, `row_height`, `selected`.

## Example

```rust
fn tree_view() -> impl View {
    let selected = State::new(Some(String::from("invoices")));
    let opened = State::new(std::collections::BTreeSet::from([String::from("sales")]));
    TreeView::new(vec![
        TreeNode::new("sales", "Sales")
            .icon(IconName::Folder)
            .child(TreeNode::new("invoices", "Invoices").icon(IconName::FileText))
            .child(TreeNode::new("quotes", "Quotes").icon(IconName::FileText)),
        TreeNode::new("stock", "Stock")
            .icon(IconName::Folder)
            .child(TreeNode::new("items", "Items").icon(IconName::FileText)),
        TreeNode::new("reports", "Reports").icon(IconName::FileText),
    ])
    .selected(selected.binding())
    .expanded(opened.binding())
}
```

Every number above is in `TreeView.json` beside this card.
