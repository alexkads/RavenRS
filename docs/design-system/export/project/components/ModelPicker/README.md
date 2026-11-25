# ModelPicker

Local AI models to download, try, use and remove.

**Family:** Forms · **Rust:** `ravenrs::prelude::ModelPicker`

## Stories

- **Example**: 268 × 174 pt, settings `{}`

## Sizing

Hugs its content: 268 × 174 pt in a parent of 760 pt; in a parent that stretches its children, as a form column, it takes the whole width.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover** on "Model": border {color:control-border,width:1.0} → {color:mix(ink, control-border, 25),width:1.0}.
- **Focus** on "Model": adds a rect (border 2.0 focus-ring).
- **Pressed** on "Model": border {color:control-border,width:1.0} → {color:mix(ink, control-border, 25),width:1.0}.

## Behavior

- **Events**: `on_cancel`, `on_download`, `on_remove`, `on_test`, `on_use`.
- **Keys** its first control handles once focused: `ArrowDown`, `ArrowUp`, `Enter`, `Space`, `Escape`.
- **Accessibility**: role `ComboBox` "Model".
- **Focus order** (Tab): ComboBox "Model" → Button "Cancel".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | ComboBox "Model" | 220.0 × 40.0 | radius-md | bg-300 | 1.0 control-border | Inter 400 15.0px ink | chevron-down 16.0px ink-muted | space-3 |
| Example | Label "1.1 GB · License: Apache 2.0" | 220.0 × 18.0 | — | — | — | Inter 400 13.0px ink-muted | — | 0.0 |
| Example | ProgressIndicator | 220.0 × 6.0 | radius-full | bg-300 | — | — | — | — |
| Example | Button "Cancel" | 90.0 × 32.0 | radius-md | bg-300 | 1.0 border | Inter 600 13.0px ink | x 14.0px ink | space-3 |

## API

Builder methods: `labels`, `new`, `on_cancel`, `on_download`, `on_remove`, `on_test`, `on_use`.

## Example

```rust
fn model_picker() -> impl View {
    let choice = |name: &str, status| ModelChoice {
        name: name.into(),
        details: "1.1 GB · License: Apache 2.0".into(),
        status,
        in_use: false,
        result: None,
        error: None,
    };
    // It shows what it is given. `ravenrs_ai::ModelManager` does the work on the desktop.
    ModelPicker::new(vec![
        choice("To download", ModelStatus::Available),
        choice("Arriving", ModelStatus::Downloading(0.4)),
        ModelChoice {
            result: Some("About 20 words per second.".into()),
            ..choice("On this machine", ModelStatus::Installed)
        },
    ])
    .on_test(|_| {})
    .on_use(|_| {})
}
```

Every number above is in `ModelPicker.json` beside this card.
