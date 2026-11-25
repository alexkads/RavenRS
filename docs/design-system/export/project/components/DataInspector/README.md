# DataInspector

JSON items as a table, a tree, the JSON and a schema, with the field chosen kept between them.

**Family:** Data · **Rust:** `ravenrs::prelude::DataInspector`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `label` | text | `Orders` |  |

## Stories

- **Example**: 275 × 174 pt, settings `{"label":"Orders"}`

## Sizing

Hugs its content: 275 × 174 pt in a parent of 760 pt; in a parent that stretches its children, as a form column, it takes the whole width.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover** on "Tree": color ink-muted → ink.
- **Focus** on "Table": adds a rect (border 2.0 focus-ring).
- **Pressed** on "Tree": color ink-muted → ink.

## Behavior

- **Events**: `on_pick`.
- **Keys** its first control handles once focused: `ArrowDown`, `ArrowUp`, `ArrowLeft`, `ArrowRight`, `End`.
- **Accessibility**: role `RadioGroup` "Orders".
- **Focus order** (Tab): RadioButton "Table" → RadioButton "Tree" → RadioButton "JSON" → RadioButton "Schema".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | RadioButton "Table" | 63.0 × 30.0 | 7.0 | bg-200 | 1.0 border | Inter 600 13.0px ink | — | 14.0 |
| Example | RadioButton "Tree" | 57.0 × 30.0 | — | — | — | Inter 600 13.0px ink-muted | — | 14.0 |
| Example | Row | 275.0 × 36.0 | 0.0 | transparent | — | Inter 600 12.0px ink-muted | — | space-4 |
| Example | Row | 275.0 × 46.0 | 0.0 | transparent | — | Inter 600 15.0px ink | — | space-4 |

## API

Builder methods: `item`, `label`, `limit`, `mode`, `new`, `of`, `on_pick`, `selected`.

## Example

```rust
fn data_inspector() -> impl View {
    DataInspector::new(
        r#"[{ "order": 101, "customer": { "name": "Ana" }, "discount": null },
            { "order": 102, "customer": { "name": "Bia" } }]"#,
    )
    .label("Orders")
}
```

Every number above is in `DataInspector.json` beside this card.
