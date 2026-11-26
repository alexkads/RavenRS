# Select

Picks one option from a list that opens under the field, with search when it is long.

**Family:** Forms · **Rust:** `ravenrs::prelude::Select`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `label` | text | `Date range` |  |
| `options` | text | `Last 7 days, Last 30 days, This year` | The options, separated by commas. |
| `selected` | number 0–10 by 1 | `0` |  |
| `size` | Mini · Small · Medium · Large | `Medium` |  |
| `searchable` | on/off | `false` | A search box at the top of the list. |
| `fill` | on/off | `false` | Fills the width of its column, as the fields around it. |
| `icon` | none · map-pin · calendar · user · globe | `none` | An icon before the value, as a pin before a place. |
| `disabled` | on/off | `false` |  |
| `read_only` | on/off | `false` |  |

## Stories

- **Default**: 220 × 40 pt, settings `{"disabled":false,"fill":false,"icon":"none","label":"Date range","options":"Last 7 days, Last 30 days, This year","read_only":false,"searchable":false,"selected":0.0,"size":"Medium"}`
- **Small and searchable**: 220 × 32 pt, settings `{"disabled":false,"fill":false,"icon":"none","label":"Currency","options":"Real, Euro, Dollar","read_only":false,"searchable":true,"selected":0.0,"size":"Small"}`
- **Disabled**: 220 × 40 pt, settings `{"disabled":true,"fill":false,"icon":"none","label":"Date range","options":"Last 7 days, Last 30 days, This year","read_only":false,"searchable":false,"selected":0.0,"size":"Medium"}`
- **Read only**: 220 × 40 pt, settings `{"disabled":false,"fill":false,"icon":"none","label":"Date range","options":"Last 7 days, Last 30 days, This year","read_only":true,"searchable":false,"selected":0.0,"size":"Medium"}`
- **Mini**: 220 × 22 pt, settings `{"disabled":false,"fill":false,"icon":"none","label":"Date range","options":"Last 7 days, Last 30 days, This year","read_only":false,"searchable":false,"selected":0.0,"size":"Mini"}`
- **Large**: 220 × 48 pt, settings `{"disabled":false,"fill":false,"icon":"none","label":"Date range","options":"Last 7 days, Last 30 days, This year","read_only":false,"searchable":false,"selected":0.0,"size":"Large"}`
- **Fill**: 728 × 40 pt, settings `{"disabled":false,"fill":true,"icon":"none","label":"Date range","options":"Last 7 days, Last 30 days, This year","read_only":false,"searchable":false,"selected":0.0,"size":"Medium"}`
- **With icon**: 220 × 40 pt, settings `{"disabled":false,"fill":false,"icon":"map-pin","label":"Studio","options":"All studios, Studio, Outside","read_only":false,"searchable":false,"selected":0.0,"size":"Medium"}`

## Sizing

Hugs its content: 220 × 40 pt in a parent of 760 pt.

- **Fill**: Fills the width of its parent: 288, 728, 1168 pt wide in parents of 320, 760, 1200 pt.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Default:

- **Hover** on "Date range": border {color:control-border,width:1.0} → {color:mix(ink, control-border, 25),width:1.0}.
- **Focus** on "Date range": adds a rect (border 2.0 focus-ring).
- **Pressed** on "Date range": border {color:control-border,width:1.0} → {color:mix(ink, control-border, 25),width:1.0}.

## Behavior

- **Keys** its first control handles once focused: `ArrowDown`, `ArrowUp`, `Enter`, `Space`, `Escape`.
- **Accessibility**: role `ComboBox` "Date range".
- **Focus order** (Tab): ComboBox "Date range".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Default | ComboBox "Date range" | 220.0 × 40.0 | radius-md | bg-300 | 1.0 control-border | Inter 400 15.0px ink | chevron-down 16.0px ink-muted | space-3 |
| Small and searchable | ComboBox "Currency" | 220.0 × 32.0 | radius-md | bg-300 | 1.0 control-border | Inter 400 13.0px ink | chevron-down 16.0px ink-muted | space-2 |
| Disabled | ComboBox "Date range" | 220.0 × 40.0 | radius-md | bg-300@45 | 1.0 control-border@45 | Inter 400 15.0px ink@45 | chevron-down 16.0px ink-muted@45 | space-3 |
| Read only | ComboBox "Date range" | 220.0 × 40.0 | radius-md | bg-300@50 | 1.0 control-border | Inter 400 15.0px ink | chevron-down 16.0px ink-muted | space-3 |
| Mini | ComboBox "Date range" | 220.0 × 22.0 | radius-md | bg-300 | 1.0 control-border | Inter 600 12.0px ink | chevron-down 12.0px ink-muted | space-2 |
| Large | ComboBox "Date range" | 220.0 × 48.0 | radius-md | bg-300 | 1.0 control-border | Inter 400 15.0px ink | chevron-down 16.0px ink-muted | space-3 |
| With icon | ComboBox "Studio" | 220.0 × 40.0 | radius-md | bg-300 | 1.0 control-border | Inter 400 15.0px ink | map-pin 16.0px ink-muted | space-3 |

## API

Builder methods: `disabled`, `fill`, `icon`, `label`, `new`, `of`, `read_only`, `searchable`, `size`, `width`.

`SelectionMode`: `Multiple`, `None`, `Single`.

## Example

```rust
fn select() -> impl View {
    let range = State::new(0);
    // `Select::of` holds the value itself; `.searchable()` adds a search box.
    let currency = State::new("BRL");
    VStack::new((
        Select::new(range.binding(), ["Last 7 days", "Last 30 days", "This year"]).label("Date range"),
        Select::of(currency.binding(), [("BRL", "Real"), ("EUR", "Euro"), ("USD", "Dollar")])
            .label("Currency")
            .size(ButtonSize::Small)
            .searchable(),
    ))
    .spacing(8.0)
}
```

Every number above is in `Select.json` beside this card.
