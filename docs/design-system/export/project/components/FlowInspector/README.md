# FlowInspector

The settings of the node or connection selected on a FlowCanvas.

**Family:** Data · **Rust:** `ravenrs::prelude::FlowInspector`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `node_selected` | on/off | `true` | A node chosen on the canvas; off, the panel says to choose one. |
| `read_only` | on/off | `false` |  |
| `width` | number 200–480 by 8 | `288` |  |

## Stories

- **A node**: 348 × 508 pt, settings `{"node_selected":true,"read_only":false,"width":288.0}`
- **Nothing selected**: 288 × 68 pt, settings `{"node_selected":false,"read_only":false,"width":288.0}`
- **Read only**: 348 × 518 pt, settings `{"node_selected":true,"read_only":true,"width":288.0}`

## Sizing

Hugs its content: 348 × 508 pt in a parent of 760 pt.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story A node:

- **Hover** on "Title": border {color:control-border,width:1.0} → {color:mix(ink, control-border, 25),width:1.0}.
- **Focus** on "Title": border {color:control-border,width:1.0} → {color:accent,width:1.5}; adds a rect (accent); adds a rect (border 2.0 focus-ring).
- **Pressed** on "Title": border {color:control-border,width:1.0} → {color:accent,width:1.5}; adds a rect (accent).

## Behavior

- **Events**: `on_change`.
- **Keys** its first control handles once focused: `ArrowUp`, `ArrowLeft`, `Space`, `Escape`, `Home`.
- **Accessibility**: role `Group` "Properties".
- **Focus order** (Tab): TextInput "Title" → TextInput "Line" → ComboBox "Shape" → TextInput "Width" → TextInput "Height" → TextInput "X" → TextInput "Y" → RadioButton "Normal" → RadioButton "Warning" → RadioButton "Error" → RadioButton "Off" → ComboBox "Fill" → ComboBox "Line" → ComboBox "Text" → TextInput "Line width".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| A node | Label "Node" | 33.0 × 16.0 | — | — | — | Inter 600 12.0px ink-muted | — | 0.0 |
| A node | Label "Title" | 28.0 × 18.0 | — | — | — | Inter 600 13.0px ink | — | 0.0 |
| A node | TextInput "Title" | 256.0 × 40.0 | radius-md | bg-300 | 1.0 control-border | Inter 400 15.0px ink | — | space-3 |
| A node | TextInput "Line" | 256.0 × 40.0 | radius-md | bg-300 | 1.0 control-border | — | — | — |
| A node | ComboBox "Shape" | 220.0 × 40.0 | radius-md | bg-300 | 1.0 control-border | Inter 400 15.0px ink | chevron-down 16.0px ink-muted | space-3 |
| A node | RadioButton "Normal" | 74.0 × 30.0 | 7.0 | bg-200 | 1.0 border | Inter 600 13.0px ink | — | 14.0 |
| A node | RadioButton "Warning" | 81.0 × 30.0 | — | — | — | Inter 600 13.0px ink-muted | — | 14.0 |
| A node | ComboBox "Fill" | 140.0 × 40.0 | radius-md | bg-300 | 1.0 border | Inter 400 15.0px ink-muted | chevron-down 16.0px ink-muted | 40.0 |
| Nothing selected | Label "Select a node or a connection to see its properties" | 256.0 × 36.0 | — | — | — | Inter 400 13.0px ink-muted | — | 0.0 |
| Read only | TextInput "Title" | 256.0 × 40.0 | radius-md | bg-300@50 | 1.0 control-border | Inter 400 15.0px ink | — | space-3 |
| Read only | TextInput "Line" | 256.0 × 40.0 | radius-md | bg-300@50 | 1.0 control-border | — | — | — |
| Read only | ComboBox "Shape" | 220.0 × 40.0 | radius-md | bg-300@45 | 1.0 control-border@45 | Inter 400 15.0px ink@45 | chevron-down 16.0px ink-muted@45 | space-3 |
| Read only | TextInput "Width" | 160.0 × 40.0 | radius-md | bg-300@45 | 1.0 control-border@45 | Inter 400 15.0px ink@45 | — | space-3 |
| Read only | Label "Normal" | 256.0 × 18.0 | — | — | — | Inter 400 13.0px ink-muted | — | 0.0 |
| Read only | ComboBox "Fill" | 140.0 × 40.0 | radius-md | bg-300@45 | 1.0 border@45 | Inter 400 15.0px ink-muted@45 | chevron-down 16.0px ink-muted@45 | 40.0 |
| Read only | TextInput "Line width" | 160.0 × 40.0 | radius-md | bg-300@45 | 1.0 control-border@45 | — | — | — |

## API

Builder methods: `history`, `kind`, `new`, `notation`, `on_change`, `read_only`, `width`.

Every number above is in `FlowInspector.json` beside this card.
