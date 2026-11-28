# LogView

A build log as it comes: ANSI colors, follows the end, search, sections that fold and lines pointed at by number

**Family:** Data · **Rust:** `ravenrs::prelude::LogView`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `follow_end` | on/off | `true` |  |
| `line_numbers` | on/off | `true` |  |
| `searchable` | on/off | `true` |  |
| `controls` | on/off | `true` | The bar above the lines; off leaves the lines alone. |
| `height` | number 80–1040 by 1 | `220` |  |
| `label` | text | `Build log` |  |
| `stages` | on/off | `false` |  |
| `keywords` | on/off | `false` |  |
| `only_section` | text | `` |  |
| `download` | on/off | `false` |  |

## Stories

- **Example**: 448 × 260 pt, settings `{"controls":true,"download":false,"follow_end":true,"height":220.0,"keywords":false,"label":"Build log","line_numbers":true,"only_section":"","searchable":true,"stages":false}`
- **No numbers**: 414 × 260 pt, settings `{"controls":true,"download":false,"follow_end":true,"height":220.0,"keywords":false,"label":"Build log","line_numbers":false,"only_section":"","searchable":true,"stages":false}`
- **No search**: 448 × 250 pt, settings `{"controls":true,"download":false,"follow_end":true,"height":220.0,"keywords":false,"label":"Build log","line_numbers":true,"only_section":"","searchable":false,"stages":false}`
- **Lines only**: 400 × 220 pt, settings `{"controls":false,"download":false,"follow_end":true,"height":220.0,"keywords":false,"label":"Build log","line_numbers":false,"only_section":"","searchable":true,"stages":false}`
- **From the top**: 448 × 260 pt, settings `{"controls":true,"download":false,"follow_end":false,"height":220.0,"keywords":false,"label":"Build log","line_numbers":true,"only_section":"","searchable":true,"stages":false}`
- **Build stages**: 760 × 300 pt, settings `{"controls":true,"download":true,"follow_end":true,"height":260.0,"keywords":true,"label":"Build log","line_numbers":true,"only_section":"","searchable":true,"stages":true}`
- **One section alone**: 760 × 260 pt, settings `{"controls":true,"download":false,"follow_end":true,"height":220.0,"keywords":true,"label":"Build log","line_numbers":true,"only_section":"Testes","searchable":true,"stages":true}`

## Sizing

Hugs its content: 448 × 260 pt in a parent of 760 pt; in a parent of 320 pt it narrows to 414 pt; in a parent that stretches its children, as a form column, it takes the whole width.

- **No numbers**: Hugs its content: 414 × 260 pt in a parent of 760 pt; in a parent of 320 pt it narrows to 361 pt; in a parent that stretches its children, as a form column, it takes the whole width; its height follows the width, as its content wraps.
- **One section alone**: Hugs its content: 760 × 260 pt in a parent of 760 pt; in a parent of 320 pt it narrows to 588 pt; its height follows the width, as its content wraps.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover** on "Search the log": border {color:control-border,width:1.0} → {color:mix(ink, control-border, 25),width:1.0}.
- **Focus** on "Search the log": border {color:control-border,width:1.0} → {color:accent,width:1.5}; adds a rect (accent); adds a rect (border 2.0 focus-ring).
- **Pressed** on "Search the log": border {color:control-border,width:1.0} → {color:accent,width:1.5}; adds a rect (accent).

## Behavior

- **Events**: `on_line`.
- **Keys** its first control handles once focused: `Space`, `Escape`.
- **Accessibility**: role `TextInput` "Search the log".
- **Focus order** (Tab): TextInput "Search the log" → Switch "Follow the end" → List "Build log".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | TextInput "Search the log" | 260.0 × 32.0 | radius-md | bg-300 | 1.0 control-border | Inter 400 13.0px ink-muted | search 16.0px ink-muted | space-2 |
| Example | Switch "Follow the end" | 146.0 × 22.0 | — | — | — | Inter 400 15.0px ink | — | 42.0 |
| Example | Button "Checkout" | 760.0 × 20.0 | 0.0 | bg-300@60 | — | Fira 500 13.0px ink | chevron-down 14.0px ink-muted | space-2 |
| Example | Button "Point at line 1" | 48.0 × 18.0 | — | — | — | Fira 500 13.0px ink-muted | — | 0.0 |
| Example | Button "Testes" | 760.0 × 20.0 | 0.0 | bg-300@60 | — | Fira 500 13.0px ink | chevron-down 14.0px ink-muted | space-2 |
| Example | Button "Point at line 3" | 48.0 × 18.0 | — | — | — | Fira 500 13.0px ink-muted | — | 0.0 |
| Build stages | Button "Download log" | 132.0 × 32.0 | radius-md | transparent | — | Inter 600 13.0px ink | download 14.0px ink | space-3 |
| One section alone | Label "Only Testes" | 84.0 × 22.0 | — | — | — | Inter 400 15.0px ink-muted | — | 0.0 |
| One section alone | Button "Show all" | 77.0 × 32.0 | radius-md | transparent | — | Inter 600 13.0px ink | — | space-3 |

## API

Builder methods: `controls`, `download`, `follow_end`, `height`, `keyword`, `label`, `line_numbers`, `new`, `on_line`, `only_section`, `searchable`.

## Example

```rust
fn log_view() -> impl View {
    // A build as it runs: the colors the test runner writes, a stage that folds, the search.
    let log = LogBuffer::new();
    log.section("Testes");
    log.push("$ cargo nextest run");
    for n in 0..200 {
        let mark = if n % 37 == 0 { "\u{1b}[31mFAIL\u{1b}[0m" } else { "\u{1b}[32mPASS\u{1b}[0m" };
        log.push(&format!("    {mark} [0.0{}s] suite::test_{n}", n % 10));
    }
    LogView::new(log).height(240.0)
}
```

Every number above is in `LogView.json` beside this card.
