# Playground

Code beside what it makes, run as it is typed, with examples to start from.

**Family:** Forms · **Rust:** `ravenrs::prelude::Playground`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `title` | text | `` | Empty: no title. |
| `output_open` | on/off | `true` | The panel of what the code printed. |
| `height` | number 240–720 by 10 | `380` |  |

## Stories

- **Examples**: 292 × 380 pt, settings `{"height":380.0,"output_open":true,"title":""}`
- **With title**: 292 × 380 pt, settings `{"height":380.0,"output_open":true,"title":"Try it"}`
- **Output closed**: 292 × 380 pt, settings `{"height":380.0,"output_open":false,"title":""}`

## Sizing

Hugs its content: 292 × 380 pt in a parent of 760 pt; in a parent that stretches its children, as a form column, it takes the whole width.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Examples:

- **Hover** on "Example": border {color:control-border,width:1.0} → {color:mix(ink, control-border, 25),width:1.0}.
- **Focus** on "Example": adds a rect (border 2.0 focus-ring).
- **Pressed** on "Example": border {color:control-border,width:1.0} → {color:mix(ink, control-border, 25),width:1.0}.

## Behavior

- **Keys** its first control handles once focused: `ArrowDown`, `ArrowUp`, `Enter`, `Space`, `Escape`.
- **Accessibility**: role `ComboBox` "Example".
- **Focus order** (Tab): ComboBox "Example" → Button "More actions" → RadioButton "Code" → RadioButton "Result" → RadioButton "Output 0" → Button "Run".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Examples | ComboBox "Example" | 220.0 × 40.0 | radius-md | bg-300 | 1.0 control-border | Inter 400 15.0px ink | chevron-down 16.0px ink-muted | space-3 |
| Examples | Button "More actions" | 40.0 × 40.0 | radius-md | transparent | — | — | ellipsis 16.0px ink | space-3 |
| Examples | RadioButton "Code" | 62.0 × 30.0 | — | — | — | Inter 600 13.0px ink-muted | — | 14.0 |
| Examples | RadioButton "Result" | 68.0 × 30.0 | 7.0 | bg-200 | 1.0 border | Inter 600 13.0px ink | — | 14.0 |
| Examples | Label "RESULT" | 48.0 × 16.0 | — | — | — | Inter 600 12.0px ink-muted | — | 0.0 |
| Examples | Heading "hello" | 236.0 × 28.0 | — | — | — | Inter 600 22.0px ink | — | 0.0 |
| Examples | Label "No problems" | 79.0 × 18.0 | — | — | — | Inter 400 13.0px ink | — | 0.0 |
| Examples | Button "Run" | 152.0 × 40.0 | radius-md | accent | — | Inter 600 15.0px on-accent | play 16.0px on-accent | 18.0 |

## API

Builder methods: `action`, `chosen_example`, `download`, `example`, `file_name`, `height`, `language`, `new`, `note`, `output_open`, `source`, `title`.

## Example

```rust
fn playground() -> impl View {
    let run = |code: &str| -> Result<AnyView, Vec<CodeDiagnostic>> {
        match code.trim().strip_prefix("say ") {
            Some(words) => Ok(Box::new(Text::new(words.to_owned()).style(TextStyle::Title))),
            None => Err(vec![CodeDiagnostic::error(1, 1, 1, "Start with `say`").hint("Write `say hello`")]),
        }
    };
    Playground::new(run)
        .language(CodeLanguage::Plain)
        .example("Hello", "say hello")
        .example("Goodbye", "say goodbye")
        .height(380.0)
}
```

Every number above is in `Playground.json` beside this card.
