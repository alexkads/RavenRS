# LookupField

Picks a record from a list searched on demand.

**Family:** Actions and inputs · **Rust:** `ravenrs::prelude::LookupField`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `placeholder` | text | `Choose a city` |  |
| `width` | number 0–880 by 1 | `220` |  |
| `label` | text | `City` |  |
| `disabled` | on/off | `false` |  |
| `read_only` | on/off | `false` |  |

## Stories

- **Example**: 220 × 40 pt, settings `{"disabled":false,"label":"City","placeholder":"Choose a city","read_only":false,"width":220.0}`
- **Disabled**: 220 × 40 pt, settings `{"disabled":true,"label":"City","placeholder":"Choose a city","read_only":false,"width":220.0}`
- **Read only**: 220 × 40 pt, settings `{"disabled":false,"label":"City","placeholder":"Choose a city","read_only":true,"width":220.0}`

## Sizing

Hugs its content: 220 × 40 pt in a parent of 760 pt.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover** on "City": border {color:control-border,width:1.0} → {color:mix(ink, control-border, 25),width:1.0}.
- **Focus** on "City": adds a rect (border 2.0 focus-ring).
- **Pressed** on "City": border {color:control-border,width:1.0} → {color:mix(ink, control-border, 25),width:1.0}.

## Behavior

- **Keys** its first control handles once focused: `ArrowDown`, `ArrowUp`, `Enter`, `Space`, `Escape`.
- **Accessibility**: role `ComboBox` "City".
- **Focus order** (Tab): ComboBox "City".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | ComboBox "City" | 220.0 × 40.0 | radius-md | bg-300 | 1.0 control-border | Inter 400 15.0px ink-muted | chevron-down 16.0px ink-muted | space-3 |
| Disabled | ComboBox "City" | 220.0 × 40.0 | radius-md | bg-300@45 | 1.0 control-border@45 | Inter 400 15.0px ink-muted@45 | chevron-down 16.0px ink-muted@45 | space-3 |
| Read only | ComboBox "City" | 220.0 × 40.0 | radius-md | bg-300@50 | 1.0 control-border | Inter 400 15.0px ink-muted | chevron-down 16.0px ink-muted | space-3 |

## API

Builder methods: `disabled`, `label`, `limit`, `new`, `placeholder`, `read_only`, `remote`, `width`.

## Example

```rust
fn lookup_field() -> impl View {
    let city = State::new(None);
    LookupField::remote(city.binding(), |request, responder| {
        // Ask a database or a server here; a clone of the responder can answer later.
        let cities = ["Belo Horizonte", "Recife", "Salvador", "São Paulo"];
        let needle = request.query.to_lowercase();
        let found = cities.iter().filter(|c| c.to_lowercase().contains(&needle));
        responder.respond(found.map(|c| LookupItem::new(*c, *c)).collect());
    })
    .placeholder("Choose a city")
    .width(220.0)
    .label("City")
}
```

Every number above is in `LookupField.json` beside this card.
