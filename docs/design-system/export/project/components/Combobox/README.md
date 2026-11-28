# Combobox

Select with a search box that filters the options.

**Family:** Forms · **Rust:** `ravenrs::prelude::Combobox`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `placeholder` | text | `Choose a country` |  |
| `label` | text | `Country` |  |
| `disabled` | on/off | `false` |  |
| `fill` | on/off | `false` |  |
| `read_only` | on/off | `false` |  |
| `width` | number 0–960 by 1 | `240` |  |

## Stories

- **Example**: 240 × 40 pt, settings `{"disabled":false,"fill":false,"label":"Country","placeholder":"Choose a country","read_only":false,"width":240.0}`
- **Disabled**: 240 × 40 pt, settings `{"disabled":true,"fill":false,"label":"Country","placeholder":"Choose a country","read_only":false,"width":240.0}`
- **Read only**: 240 × 40 pt, settings `{"disabled":false,"fill":false,"label":"Country","placeholder":"Choose a country","read_only":true,"width":240.0}`

## Sizing

Hugs its content: 240 × 40 pt in a parent of 760 pt.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover** on "Country": border {color:border,width:1.0} → {color:mix(ink, border, 25),width:1.0}.
- **Focus** on "Country": adds a rect (border 2.0 focus-ring).
- **Pressed** on "Country": border {color:border,width:1.0} → {color:mix(ink, border, 25),width:1.0}.

## Behavior

- **Keys** its first control handles once focused: `Enter`, `Space`, `Escape`.
- **Accessibility**: role `ComboBox` "Country".
- **Focus order** (Tab): ComboBox "Country".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | ComboBox "Country" | 240.0 × 40.0 | radius-md | bg-300 | 1.0 border | Inter 400 15.0px ink-muted | chevron-down 16.0px ink-muted | space-3 |
| Disabled | ComboBox "Country" | 240.0 × 40.0 | radius-md | bg-300@45 | 1.0 border@45 | Inter 400 15.0px ink-muted@45 | chevron-down 16.0px ink-muted@45 | space-3 |
| Read only | ComboBox "Country" | 240.0 × 40.0 | radius-md | bg-300@50 | 1.0 border | Inter 400 15.0px ink-muted | chevron-down 16.0px ink-muted | space-3 |

## API

Builder methods: `disabled`, `fill`, `label`, `new`, `placeholder`, `read_only`, `width`.

## Example

```rust
fn combobox() -> impl View {
    let country = State::new(None);
    Combobox::new(country.binding(), ["Argentina", "Brazil", "Chile", "Portugal", "Spain"])
        .placeholder("Choose a country")
        .label("Country")
}
```

Every number above is in `Combobox.json` beside this card.
