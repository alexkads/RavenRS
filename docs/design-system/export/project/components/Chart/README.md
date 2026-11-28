# Chart

Area, bar, line, pie, doughnut, radar and radial charts in the patterns of the shadcn/ui catalog, with tooltips, keyboard, a focus that glides between categories, lines of reference and a fixed axis; scatter, sparklines and candlesticks too.

**Family:** Data · **Rust:** `ravenrs::prelude::Chart`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `title` | text | `Sales` |  |
| `height` | number 0–1040 by 1 | `260` |  |
| `align_axis_ends` | on/off | `false` |  |
| `alt` | text | `` | Empty leaves it as the component has it. |
| `animate_changes` | on/off | `false` |  |
| `concentric` | on/off | `false` |  |
| `dots` | on/off | `false` |  |
| `gradient` | on/off | `false` |  |
| `grid_dashed` | on/off | `false` |  |
| `hide_category_axis` | on/off | `false` |  |
| `hide_legend` | on/off | `false` |  |
| `hide_tooltip` | on/off | `false` |  |
| `horizontal` | on/off | `false` |  |
| `label_guides` | on/off | `false` |  |
| `labels_at_zero` | on/off | `false` |  |
| `legend_at` | Bottom · Top | `Bottom` |  |
| `lines_only` | on/off | `false` |  |
| `no_appear` | on/off | `false` |  |
| `no_hover_focus` | on/off | `false` |  |
| `no_hover_motion` | on/off | `false` |  |
| `no_separator` | on/off | `false` |  |
| `no_spokes` | on/off | `false` |  |
| `polar_fill` | on/off | `false` |  |
| `radius_axis` | on/off | `false` |  |
| `show_value_axis` | on/off | `false` |  |
| `tight_scale` | on/off | `false` |  |
| `tracks` | on/off | `false` |  |
| `vary_colors` | on/off | `false` |  |

## Stories

- **Example**: 728 × 260 pt, settings `{"align_axis_ends":false,"alt":"","animate_changes":false,"concentric":false,"dots":false,"gradient":false,"grid_dashed":false,"height":260.0,"hide_category_axis":false,"hide_legend":false,"hide_tooltip":false,"horizontal":false,"label_guides":false,"labels_at_zero":false,"legend_at":"Bottom","lines_only":false,"no_appear":false,"no_hover_focus":false,"no_hover_motion":false,"no_separator":false,"no_spokes":false,"polar_fill":false,"radius_axis":false,"show_value_axis":false,"tight_scale":false,"title":"Sales","tracks":false,"vary_colors":false}`
- **With target line**: 732 × 260 pt, settings `{"align_axis_ends":false,"alt":"","animate_changes":false,"concentric":false,"dots":false,"gradient":false,"grid_dashed":false,"height":260.0,"hide_category_axis":false,"hide_legend":false,"hide_tooltip":false,"horizontal":false,"label_guides":false,"labels_at_zero":false,"legend_at":"Bottom","lines_only":false,"no_appear":false,"no_hover_focus":false,"no_hover_motion":false,"no_separator":false,"no_spokes":false,"polar_fill":false,"radius_axis":false,"show_value_axis":false,"tight_scale":false,"title":"Sales","tracks":false,"vary_colors":false}`
- **Legend at: Top**: 728 × 265 pt, settings `{"align_axis_ends":false,"alt":"","animate_changes":false,"concentric":false,"dots":false,"gradient":false,"grid_dashed":false,"height":260.0,"hide_category_axis":false,"hide_legend":false,"hide_tooltip":false,"horizontal":false,"label_guides":false,"labels_at_zero":false,"legend_at":"Top","lines_only":false,"no_appear":false,"no_hover_focus":false,"no_hover_motion":false,"no_separator":false,"no_spokes":false,"polar_fill":false,"radius_axis":false,"show_value_axis":false,"tight_scale":false,"title":"Sales","tracks":false,"vary_colors":false}`
- **Sparkline**: 120 × 24 pt, settings `{"align_axis_ends":false,"alt":"","animate_changes":false,"concentric":false,"dots":false,"gradient":false,"grid_dashed":false,"height":260.0,"hide_category_axis":false,"hide_legend":false,"hide_tooltip":false,"horizontal":false,"label_guides":false,"labels_at_zero":false,"legend_at":"Bottom","lines_only":false,"no_appear":false,"no_hover_focus":false,"no_hover_motion":false,"no_separator":false,"no_spokes":false,"polar_fill":false,"radius_axis":false,"show_value_axis":false,"tight_scale":false,"title":"Sales","tracks":false,"vary_colors":false}`

## Sizing

Fills the width of its parent: 288, 728, 1168 pt wide in parents of 320, 760, 1200 pt.

- **With target line**: Hugs its content: 732 × 260 pt in a parent of 760 pt; in a parent of 320 pt it narrows to 292 pt.
- **Sparkline**: Hugs its content: 120 × 24 pt in a parent of 760 pt.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover** on "Sales": adds a rect (ink-muted@55); adds a rect (accent@22); adds a rect (border 2.0 bg-200); adds a rect (chart-2@22).
- **Focus** on "Sales": adds a rect (border 2.0 focus-ring).
- **Pressed** on "Sales": adds a rect (ink-muted@55); adds a rect (accent@22); adds a rect (border 2.0 bg-200); adds a rect (chart-2@22).

## Behavior

- **Keys** its first control handles once focused: `ArrowDown`, `ArrowUp`, `ArrowLeft`, `ArrowRight`, `Home`, `End`.
- **Accessibility**: role `Label` "Sales".
- **Focus order** (Tab): Image "Sales".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Label "Sales" | 728.0 × 18.0 | — | — | — | Inter 600 13.0px ink | — | 0.0 |
| Example | Image "Sales" | 728.0 × 210.0 | — | — | — | Inter 400 12.0px ink-muted | — | 2.0 |
| Example | ListItem "2025: Jan: 12" | 0.0 × 0.0 | — | — | — | — | — | — |
| Example | Label "2025" | 32.0 × 16.0 | — | — | — | Inter 600 12.0px ink-muted | — | 0.0 |
| With target line | Image "Jan: 12, Feb: 18, Mar: 15, Apr: 21, May: 19, Jun: 24" | 728.0 × 260.0 | — | — | — | Inter 400 12.0px ink-muted | — | 7.0 |
| Sparkline | Image "Duration of the last builds" | 120.0 × 24.0 | — | — | — | — | — | — |

## API

Builder methods: `active`, `align_axis_ends`, `alt`, `angles`, `animate_changes`, `appear_key`, `bar_radius`, `bars_from`, `candles`, `categories`, `category_labels`, `center_text`, `concentric`, `curve`, `dot_icon`, `dots`, `fill_opacity`, `format`, `gradient`, `grid`, `grid_columns`, `grid_dashed`, `height`, `hide_category_axis`, `hide_legend`, `hide_tooltip`, `hole`, `horizontal`, `item_colors`, `label_guides`, `labels_at_zero`, `legend_at`, `lines_only`, `min_bar_length`, `negative_color`, `new`, `no_appear`, `no_hover_focus`, `no_hover_motion`, `no_separator`, `no_spokes`, `polar_fill`, `polar_grid`, `positions`, `radius_axis`, `reference`, `reference_line`, `rings`, `series`, `series_color`, `series_icon`, `show_value_axis`, `size`, `slice_radii`, `stack`, `tick_format`, `tight_scale`, `title`, `tooltip`, `tracks`, `value_domain`, `value_labels`, `value_ticks`, `vary_colors`.

`ChartBarFrom`: `Bottom`, `Left`, `Right`, `Top`.

`ChartIndicator`: `Dashed`, `Dot`, `Line`, `None`.

`ChartLegend`: `Bottom`, `Top`.

## Example

```rust
fn chart() -> impl View {
    Chart::new(ChartKind::Line)
        .title("Sales")
        .categories(["Jan", "Feb", "Mar", "Apr", "May", "Jun"])
        .series("2025", [12.0, 18.0, 15.0, 21.0, 19.0, 24.0])
        .series("2026", [14.0, 21.0, 26.0, 24.0, 31.0, 35.0])
        .height(260.0)
}
```

Every number above is in `Chart.json` beside this card.
