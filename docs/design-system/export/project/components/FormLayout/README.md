# FormLayout

Groups of fields in columns, tabs and a list of errors.

**Family:** Actions and inputs · **Rust:** `ravenrs::prelude::FormLayout`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `columns` | number 0–10 by 1 | `2` |  |

## Stories

- **Example**: 728 × 192 pt, settings `{"columns":2.0}`
- **Groups, with and without a title**: 728 × 184 pt, settings `{"columns":2.0}`
- **Group with its own columns**: 728 × 312 pt, settings `{"columns":2.0}`
- **With errors**: 728 × 208 pt, settings `{"columns":2.0}`
- **With tabs**: 728 × 204 pt, settings `{"columns":2.0}`

## Sizing

Fills the width of its parent: 288, 728, 1168 pt wide in parents of 320, 760, 1200 pt; its height follows the width, as its content wraps.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover** on "Name": border {color:control-border,width:1.0} → {color:mix(ink, control-border, 25),width:1.0}.
- **Focus** on "Name": border {color:control-border,width:1.0} → {color:accent,width:1.5}; adds a rect (accent); adds a rect (border 2.0 focus-ring).
- **Pressed** on "Name": border {color:control-border,width:1.0} → {color:accent,width:1.5}; adds a rect (accent).

## Behavior

- **Keys** its first control handles once focused: `Space`, `Escape`.
- **Accessibility**: role `Label` "Contact".
- **Focus order** (Tab): TextInput "Name" → TextInput "City" → TextInput "Notes".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Label "Contact" | 728.0 × 24.0 | — | — | — | Inter 600 17.0px ink | — | 0.0 |
| Example | Label "Name" | 37.0 × 18.0 | — | — | — | Inter 600 13.0px ink | — | 0.0 |
| Example | Label "*" | 8.0 × 18.0 | — | — | — | Inter 600 13.0px danger | — | 0.0 |
| Example | TextInput "Name" | 356.0 × 40.0 | radius-md | bg-300 | 1.0 control-border | — | — | — |
| Example | TextInput "Notes" | 728.0 × 56.0 | radius-md | bg-300 | 1.0 control-border | — | — | — |
| With errors | Alert | 728.0 × 88.0 | radius-lg | danger@10 | 1.0 danger@30 | Inter 600 15.0px ink | circle-alert 18.0px danger | space-4 |
| With errors | Label "Please review the form" | 166.0 × 22.0 | — | — | — | Inter 600 15.0px ink | — | 0.0 |
| With errors | Label "Name is required. City must have at least 2 letters." | 197.0 × 36.0 | — | — | — | Inter 400 13.0px ink-muted | — | 0.0 |
| With tabs | Tab "Orders" | 54.0 × 42.0 | — | — | — | Inter 600 15.0px ink | — | 2.0 |
| With tabs | Tab "Notes" | 47.0 × 42.0 | — | — | — | Inter 600 15.0px ink-muted | — | 2.0 |
| With tabs | Label "The customer's orders." | 728.0 × 22.0 | — | — | — | Inter 400 15.0px ink-muted | — | 0.0 |

## API

Builder methods: `columns`, `errors`, `group`, `new`, `tab`.

## Example

```rust
fn form_layout() -> impl View {
    let (name, city, notes) =
        (State::new(String::new()), State::new(String::new()), State::new(String::new()));
    FormLayout::new().columns(2).group(
        FormGroup::titled("Contact")
            .field(FormField::new("Name", TextField::new(name.binding())).required())
            .field(FormField::new("City", TextField::new(city.binding())))
            .wide(FormField::new("Notes", TextArea::new(notes.binding()).rows(2))),
    )
}
```

Every number above is in `FormLayout.json` beside this card.
