# ScanField

Field for a barcode scanner: keeps the focus, tells a scanner from fingers and empties after each code.

**Family:** Forms · **Rust:** `ravenrs::prelude::ScanField`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `placeholder` | text | `Scan or type a code and press Enter` |  |
| `disabled` | on/off | `false` |  |

## Stories

- **Example**: 282 × 40 pt, settings `{"disabled":false,"placeholder":"Scan or type a code and press Enter"}`
- **Disabled**: 282 × 40 pt, settings `{"disabled":true,"placeholder":"Scan or type a code and press Enter"}`

## Sizing

Hugs its content: 282 × 40 pt in a parent of 760 pt; in a parent that stretches its children, as a form column, it takes the whole width.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover** on "Scan or type a code and press Enter": border {color:control-border,width:1.0} → {color:accent,width:1.5}.
- **Focus** on "Scan or type a code and press Enter": border {color:control-border,width:1.0} → {color:accent,width:1.5}; adds a rect (border 2.0 focus-ring).
- **Pressed** on "Scan or type a code and press Enter": border {color:control-border,width:1.0} → {color:accent,width:1.5}.

## Behavior

- **Keys** its first control handles once focused: `Space`.
- **Accessibility**: role `TextInput` "Scan or type a code and press Enter".
- **Focus order** (Tab): TextInput "Scan or type a code and press Enter".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | TextInput "Scan or type a code and press Enter" | 282.0 × 40.0 | radius-md | bg-300 | 1.0 control-border | Inter 400 15.0px ink-muted | — | space-3 |
| Disabled | TextInput "Scan or type a code and press Enter" | 282.0 × 40.0 | radius-md | bg-300@45 | 1.0 control-border@45 | Inter 400 15.0px ink-muted@45 | — | space-3 |

## API

Builder methods: `disabled`, `new`, `placeholder`.

## Example

```rust
fn scan_field() -> impl View {
    let last = State::new(String::new());
    let on_scan = last.clone();
    VStack::new((
        ScanField::new(move |scan: Scan| {
            let source = if scan.from_reader { "a scanner" } else { "the keyboard" };
            on_scan.set(format!("{} from {source}", scan.text));
        })
        .placeholder("Scan or type a code and press Enter"),
        Text::bound(&last, Clone::clone).foreground(Semantic::InkMuted),
    ))
    .spacing(8.0)
}
```

Every number above is in `ScanField.json` beside this card.
