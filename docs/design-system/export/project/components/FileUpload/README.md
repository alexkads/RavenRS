# FileUpload

Files from the chooser or dropped, with a reason when refused.

**Family:** Actions and inputs · **Rust:** `ravenrs::prelude::FileUpload`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `most_files` | number 0–20 by 1 | `5` |  |
| `label` | text | `Receipts` |  |
| `disabled` | on/off | `false` |  |
| `read_only` | on/off | `false` |  |

## Stories

- **Example**: 728 × 138 pt, settings `{"disabled":false,"label":"Receipts","most_files":5.0,"read_only":false}`
- **Disabled**: 728 × 138 pt, settings `{"disabled":true,"label":"Receipts","most_files":5.0,"read_only":false}`
- **Read only**: 728 × 32 pt, settings `{"disabled":false,"label":"Receipts","most_files":5.0,"read_only":true}`

## Sizing

Fills the width of its parent: 288, 728, 1168 pt wide in parents of 320, 760, 1200 pt.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover** on "Choose files": fill bg-300 → mix(ink, bg-300, 6).
- **Focus** on "Choose files": adds a rect (border 2.0 focus-ring).
- **Pressed** on "Choose files": border {color:border,width:1.0} → {color:border,width:0.97}, fill bg-300 → mix(ink, bg-300, 12), rect [313.0,90.0,103.0,32.0] → [314.54,90.48,99.91,31.04].

## Behavior

- **Accessibility**: role `List` "Receipts".
- **Focus order** (Tab): Button "Choose files".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Label "Drop files here" | 91.0 × 18.0 | — | — | — | Inter 400 13.0px ink-muted | — | 0.0 |
| Example | Label "pdf, png, jpg" | 78.0 × 16.0 | — | — | — | Inter 600 12.0px ink-muted | — | 0.0 |
| Example | Button "Choose files" | 103.0 × 32.0 | radius-md | bg-300 | 1.0 border | Inter 600 13.0px ink | — | space-3 |
| Disabled | Label "Drop files here" | 91.0 × 18.0 | — | — | — | Inter 400 13.0px ink-muted@45 | — | 0.0 |
| Disabled | Label "pdf, png, jpg" | 78.0 × 16.0 | — | — | — | Inter 600 12.0px ink-muted@45 | — | 0.0 |
| Disabled | Button "Choose files" | 103.0 × 32.0 | radius-md | bg-300@45 | 1.0 border | Inter 600 13.0px ink@45 | — | space-3 |
| Read only | ListItem "contrato.pdf" | 728.0 × 32.0 | — | — | — | Inter 400 15.0px ink | file 16.0px ink-muted | 0.0 |

## API

Builder methods: `accept`, `disabled`, `label`, `most_bytes`, `most_files`, `new`, `read_only`, `width`.

## Example

```rust
fn file_upload() -> impl View {
    let files = State::new(Vec::<PickedFile>::new());
    FileUpload::new(files.binding()).accept(["pdf", "png", "jpg"]).most_files(5).label("Receipts")
}
```

Every number above is in `FileUpload.json` beside this card.
