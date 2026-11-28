# DataGrid

Virtualized grid for 100,000 rows: sorting, filters, grouping and export.

**Family:** Data · **Rust:** `ravenrs::prelude::DataGrid`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `height` | number 0–1520 by 1 | `380` |  |
| `detail_height` | number 0–960 by 1 | `240` |  |
| `empty_text` | text | `` | Empty leaves it as the component has it. |
| `export_title` | text | `` | Empty leaves it as the component has it. |
| `filter_builder` | on/off | `false` |  |
| `filter_row` | on/off | `true` |  |
| `footer` | on/off | `true` |  |
| `horizontal_lines` | on/off | `true` |  |
| `row_height` | number 0–144 by 1 | `36` |  |
| `search_panel` | on/off | `true` |  |
| `selection_mode` | None · Single · Multiple | `Single` |  |
| `striped` | on/off | `false` |  |
| `toolbar` | on/off | `true` |  |
| `vertical_lines` | on/off | `false` |  |
| `rows` | number 0–1000 by 1 | `1000` | How many rows the example has; none shows the empty text. |

## Stories

- **Example**: 728 × 380 pt, settings `{"detail_height":240.0,"empty_text":"","export_title":"","filter_builder":false,"filter_row":true,"footer":true,"height":380.0,"horizontal_lines":true,"row_height":36.0,"rows":1000.0,"search_panel":true,"selection_mode":"Single","striped":false,"toolbar":true,"vertical_lines":false}`
- **Striped**: 728 × 380 pt, settings `{"detail_height":240.0,"empty_text":"","export_title":"","filter_builder":false,"filter_row":true,"footer":true,"height":380.0,"horizontal_lines":true,"row_height":36.0,"rows":1000.0,"search_panel":true,"selection_mode":"Single","striped":true,"toolbar":true,"vertical_lines":false}`
- **Compact**: 728 × 380 pt, settings `{"detail_height":240.0,"empty_text":"","export_title":"","filter_builder":false,"filter_row":true,"footer":true,"height":380.0,"horizontal_lines":true,"row_height":28.0,"rows":1000.0,"search_panel":true,"selection_mode":"Single","striped":true,"toolbar":true,"vertical_lines":false}`
- **Bare**: 728 × 380 pt, settings `{"detail_height":240.0,"empty_text":"","export_title":"","filter_builder":false,"filter_row":false,"footer":false,"height":380.0,"horizontal_lines":true,"row_height":36.0,"rows":1000.0,"search_panel":false,"selection_mode":"Single","striped":false,"toolbar":false,"vertical_lines":false}`
- **Vertical lines**: 728 × 380 pt, settings `{"detail_height":240.0,"empty_text":"","export_title":"","filter_builder":false,"filter_row":true,"footer":true,"height":380.0,"horizontal_lines":true,"row_height":36.0,"rows":1000.0,"search_panel":true,"selection_mode":"Single","striped":false,"toolbar":true,"vertical_lines":true}`
- **Selection mode: None**: 728 × 380 pt, settings `{"detail_height":240.0,"empty_text":"","export_title":"","filter_builder":false,"filter_row":true,"footer":true,"height":380.0,"horizontal_lines":true,"row_height":36.0,"rows":1000.0,"search_panel":true,"selection_mode":"None","striped":false,"toolbar":true,"vertical_lines":false}`
- **Selection mode: Multiple**: 728 × 380 pt, settings `{"detail_height":240.0,"empty_text":"","export_title":"","filter_builder":false,"filter_row":true,"footer":true,"height":380.0,"horizontal_lines":true,"row_height":36.0,"rows":1000.0,"search_panel":true,"selection_mode":"Multiple","striped":false,"toolbar":true,"vertical_lines":false}`
- **Empty**: 728 × 380 pt, settings `{"detail_height":240.0,"empty_text":"No customers yet.","export_title":"","filter_builder":false,"filter_row":true,"footer":true,"height":380.0,"horizontal_lines":true,"row_height":36.0,"rows":0.0,"search_panel":true,"selection_mode":"Single","striped":false,"toolbar":true,"vertical_lines":false}`
- **Empty, bare**: 728 × 380 pt, settings `{"detail_height":240.0,"empty_text":"No customers yet.","export_title":"","filter_builder":false,"filter_row":false,"footer":false,"height":380.0,"horizontal_lines":true,"row_height":36.0,"rows":0.0,"search_panel":false,"selection_mode":"Single","striped":false,"toolbar":false,"vertical_lines":false}`

## Sizing

Fills the width of its parent: 288, 728, 1168 pt wide in parents of 320, 760, 1200 pt.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover** on "Search all columns": border {color:control-border,width:1.0} → {color:mix(ink, control-border, 25),width:1.0}.
- **Focus** on "Search all columns": border {color:control-border,width:1.0} → {color:accent,width:1.5}; adds a rect (accent); adds a rect (border 2.0 focus-ring).
- **Pressed** on "Search all columns": border {color:control-border,width:1.0} → {color:accent,width:1.5}; adds a rect (accent).

## Behavior

- **Events**: `on_add_row`, `on_cell_edit`, `on_delete_rows`, `on_export`, `on_export_file`, `on_row_activate`, `on_row_open`.
- **Keys** its first control handles once focused: `Space`, `Escape`.
- **Accessibility**: role `TextInput` "Search all columns".
- **Focus order** (Tab): TextInput "Search all columns" → Button "Group" → Button "Columns" → TextInput "Filter ID" → TextInput "Filter Customer" → TextInput "Filter MRR" → Grid "Data grid".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | TextInput "Search all columns" | 240.0 × 32.0 | radius-md | bg-300 | 1.0 control-border | Inter 400 13.0px ink-muted | search 16.0px ink-muted | space-2 |
| Example | Button "Group" | 107.0 × 32.0 | radius-md | bg-300 | 1.0 border | Inter 600 13.0px ink | layers 14.0px ink | space-3 |
| Example | Button "Columns" | 124.0 × 32.0 | radius-md | bg-300 | 1.0 border | Inter 600 13.0px ink | table 14.0px ink | space-3 |
| Example | Label "1,000 of 1,000 rows" | 120.0 × 18.0 | — | — | — | Inter 400 13.0px ink-muted | — | 0.0 |
| Example | ColumnHeader "ID" | 84.0 × 38.0 | — | — | — | Inter 600 12.0px ink-muted | — | 67.0 |
| Example | ColumnHeader "Customer" | 214.0 × 38.0 | — | — | — | Inter 600 12.0px ink-muted | — | space-3 |
| Example | ColumnHeader "MRR" | 144.0 × 38.0 | — | — | — | Inter 600 12.0px ink-muted | — | 112.0 |
| Example | TextInput "Filter ID" | 90.0 × 38.0 | radius-sm | transparent | — | Inter 400 13.0px ink-muted | — | 21.0 |
| Example | TextInput "Filter Customer" | 220.0 × 38.0 | radius-sm | transparent | — | Inter 400 13.0px ink-muted | — | space-3 |
| Example | TextInput "Filter MRR" | 150.0 × 38.0 | radius-sm | transparent | — | Inter 400 13.0px ink-muted | — | 81.0 |
| Example | Row | 460.0 × 36.0 | 0.0 | transparent | — | Inter 400 13.0px ink | — | 69.0 |
| Example | Row | 460.0 × 36.0 | 0.0 | transparent | — | Inter 400 13.0px ink | — | 72.0 |
| Example | Row | 460.0 × 36.0 | 0.0 | transparent | — | Inter 400 13.0px ink | — | 70.0 |
| Example | Row | 460.0 × 36.0 | — | — | — | — | — | — |
| Example | Button "Edit summary ID" | 90.0 × 40.0 | 0.0 | transparent | — | Inter 400 13.0px ink | — | 69.0 |
| Example | Button "Edit summary Customer" | 220.0 × 40.0 | 0.0 | transparent | — | Inter 400 13.0px ink | — | space-3 |
| Example | Button "Edit summary MRR" | 150.0 × 40.0 | 0.0 | transparent | — | Inter 400 13.0px ink | — | 96.0 |
| Striped | Row | 460.0 × 36.0 | 0.0 | bg-300@40 | — | Inter 400 13.0px ink | — | 72.0 |
| Striped | Row | 460.0 × 36.0 | 0.0 | bg-300@40 | — | Inter 400 13.0px ink | — | 69.0 |
| Striped | Row | 460.0 × 36.0 | 0.0 | bg-300@40 | — | Inter 400 13.0px ink | — | 70.0 |
| Compact | Row | 460.0 × 28.0 | 0.0 | transparent | — | Inter 400 13.0px ink | — | 69.0 |
| Compact | Row | 460.0 × 28.0 | 0.0 | bg-300@40 | — | Inter 400 13.0px ink | — | 72.0 |
| Compact | Row | 460.0 × 28.0 | 0.0 | transparent | — | Inter 400 13.0px ink | — | 70.0 |
| Compact | Row | 460.0 × 28.0 | 0.0 | bg-300@40 | — | Inter 400 13.0px ink | — | 69.0 |
| Compact | Row | 460.0 × 28.0 | 0.0 | bg-300@40 | — | Inter 400 13.0px ink | — | 70.0 |
| Compact | Row | 460.0 × 28.0 | — | — | — | — | — | — |
| Compact | Button "Edit summary ID" | 90.0 × 40.0 | 0.0 | transparent | — | Inter 400 13.0px ink | — | 70.0 |
| Compact | Button "Edit summary Customer" | 220.0 × 40.0 | 0.0 | transparent | — | Inter 400 13.0px ink | — | space-3 |
| Compact | Button "Edit summary MRR" | 150.0 × 40.0 | 0.0 | transparent | — | Inter 400 13.0px ink | — | 94.0 |
| Bare | Row | 460.0 × 36.0 | 0.0 | transparent | — | Inter 400 13.0px ink | — | 64.0 |
| Bare | Row | 460.0 × 36.0 | 0.0 | transparent | — | Inter 400 13.0px ink | — | 67.0 |
| Selection mode: Multiple | CheckBox "Select row" | 44.0 × 38.0 | — | — | — | — | — | — |
| Selection mode: Multiple | Row | 504.0 × 36.0 | 0.0 | transparent | — | Inter 400 13.0px ink | — | 113.0 |
| Selection mode: Multiple | CheckBox "Select 0" | 44.0 × 36.0 | — | — | — | — | — | — |
| Selection mode: Multiple | Row | 504.0 × 36.0 | 0.0 | transparent | — | Inter 400 13.0px ink | — | 116.0 |
| Selection mode: Multiple | Row | 504.0 × 36.0 | 0.0 | transparent | — | Inter 400 13.0px ink | — | 114.0 |
| Empty | Label "No customers yet." | 130.0 × 22.0 | — | — | — | Inter 400 15.0px ink-muted | — | 0.0 |
| Empty | Button "Edit summary ID" | 90.0 × 40.0 | 0.0 | transparent | — | — | — | — |
| Empty | Button "Edit summary MRR" | 150.0 × 40.0 | 0.0 | transparent | — | Inter 600 13.0px ink | — | 100.0 |

## API

Builder methods: `cell_style`, `current`, `detail_height`, `empty_text`, `even_row`, `expanded_rows`, `export_title`, `filter_builder`, `filter_row`, `footer`, `group_by`, `group_panel`, `height`, `horizontal_lines`, `key_column`, `layout`, `name_column`, `new`, `odd_row`, `on_add_row`, `on_cell_edit`, `on_delete_rows`, `on_export`, `on_export_file`, `on_row_activate`, `on_row_open`, `page_size`, `remote`, `row_action`, `row_detail`, `row_height`, `scenarios`, `search_panel`, `selected_keys`, `selection`, `selection_mode`, `sort_by`, `striped`, `toolbar`, `vertical_lines`.

## Example

```rust
fn data_grid() -> impl View {
    let rows = (0..1000)
        .map(|i| {
            vec![
                Value::Number(f64::from(i)),
                Value::Text(format!("Customer {i}")),
                Value::Number(f64::from(i % 90) * 13.5),
            ]
        })
        .collect();
    DataGrid::new(
        GridData::new(rows),
        [
            GridColumn::number("ID").width(90.0),
            GridColumn::text("Customer").width(220.0).editable(),
            GridColumn::currency("MRR", "R$ ", 2).width(150.0).summary(Summary::Sum),
        ],
    )
    .height(380.0)
}
```

Every number above is in `DataGrid.json` beside this card.
