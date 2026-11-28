# CodeEditor

Edits code with colors, line numbers, completions and the problems marked where they are.

**Family:** Forms · **Rust:** `ravenrs::prelude::CodeEditor`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `language` | Rust · TOML · Plain | `Rust` |  |
| `line_numbers` | on/off | `true` |  |
| `rows` | number 3–40 by 1 | `12` |  |
| `read_only` | on/off | `false` |  |

## Stories

- **Rust**: 395 × 276 pt, settings `{"language":"Rust","line_numbers":true,"read_only":false,"rows":12.0}`
- **TOML**: 554 × 276 pt, settings `{"language":"TOML","line_numbers":true,"read_only":false,"rows":12.0}`
- **Plain**: 395 × 276 pt, settings `{"language":"Plain","line_numbers":true,"read_only":false,"rows":12.0}`
- **Read only, no numbers**: 362 × 276 pt, settings `{"language":"Rust","line_numbers":false,"read_only":true,"rows":12.0}`

## Sizing

Hugs its content: 395 × 276 pt in a parent of 760 pt; in a parent of 320 pt it narrows to 353 pt; in a parent that stretches its children, as a form column, it takes the whole width.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Rust:

- **Hover** on "": border {color:control-border,width:1.0} → {color:mix(ink, control-border, 25),width:1.0}.
- **Focus** on "": border {color:control-border,width:1.0} → {color:accent,width:1.5}; adds a rect (accent); adds a rect (border 2.0 focus-ring).
- **Pressed** on "": border {color:control-border,width:1.0} → {color:accent,width:1.5}; adds a rect (accent).

## Behavior

- **Events**: `on_complete`, `on_go_to`, `on_hover`.
- **Keys** its first control handles once focused: `ArrowUp`, `ArrowLeft`, `Enter`, `Space`, `Escape`.
- **Accessibility**: role `TextInput`.
- **Focus order** (Tab): TextInput.

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Rust | TextInput | 395.0 × 276.0 | radius-md | bg-100 | 1.0 control-border | Fira 500 14.0px focus-ring | — | 45.0 |
| TOML | TextInput | 554.0 × 276.0 | radius-md | bg-100 | 1.0 control-border | Fira 500 14.0px ink-muted | — | 45.0 |
| Plain | TextInput | 395.0 × 276.0 | radius-md | bg-100 | 1.0 control-border | Fira 500 14.0px ink | — | 45.0 |
| Read only, no numbers | TextInput | 362.0 × 276.0 | radius-md | bg-100@50 | 1.0 control-border | Fira 500 14.0px focus-ring | — | space-3 |

## API

Builder methods: `caret`, `completions`, `diagnostics`, `fill`, `focus_request`, `follow_end`, `history`, `hover`, `indent`, `language`, `line_numbers`, `new`, `on_complete`, `on_go_to`, `on_hover`, `read_only`, `rows`.

## Example

```rust
fn code_editor() -> impl View {
    let code = State::new(
        "fn total(prices: &[f64]) -> f64 {\n    // Sum of every price\n    prices.iter().sum()\n}\n"
            .to_owned(),
    );
    CodeEditor::new(code.binding())
        .diagnostics(vec![
            CodeDiagnostic::warning(3, 5, 6, "`prices` could be empty").hint("Return 0.0 first"),
        ])
        .rows(10)
}
```

Every number above is in `CodeEditor.json` beside this card.
