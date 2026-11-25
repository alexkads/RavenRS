# Stat

Headline number with a label and a change indicator.

**Family:** Data · **Rust:** `ravenrs::prelude::Stat`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `label` | text | `Revenue` |  |
| `value` | text | `$128,450` |  |
| `delta` | text | `+12.4%` | Empty shows no change. |
| `delta_direction` | Up · Down · Up, bad · Down, good | `Up` |  |
| `caption` | text | `` | A muted line under the value; empty shows none. |
| `large` | on/off | `false` |  |
| `value_tone` | Neutral · Accent · Success · Warning · Danger | `Neutral` | The value in a tone, for a total whose color says what it is. |

## Stories

- **Example**: 134 × 86 pt, settings `{"caption":"","delta":"+12.4%","delta_direction":"Up","label":"Revenue","large":false,"value":"$128,450","value_tone":"Neutral"}`
- **Large**: 189 × 98 pt, settings `{"caption":"","delta":"+12.4%","delta_direction":"Up","label":"Revenue","large":true,"value":"$128,450","value_tone":"Neutral"}`
- **Label and value**: 89 × 58 pt, settings `{"caption":"","delta":"","delta_direction":"Up","label":"IN THE QUEUE","large":false,"value":"2","value_tone":"Neutral"}`
- **With a caption**: 133 × 82 pt, settings `{"caption":"The oldest waits 40 s","delta":"","delta_direction":"Up","label":"IN THE QUEUE","large":false,"value":"2","value_tone":"Neutral"}`
- **Delta: Down**: 134 × 86 pt, settings `{"caption":"","delta":"-3.1%","delta_direction":"Down","label":"Revenue","large":false,"value":"$128,450","value_tone":"Neutral"}`
- **Delta: Up, bad**: 76 × 86 pt, settings `{"caption":"","delta":"+0.4%","delta_direction":"Up, bad","label":"Churn","large":false,"value":"1.8%","value_tone":"Neutral"}`
- **Delta: Down, good**: 74 × 86 pt, settings `{"caption":"","delta":"-0.4%","delta_direction":"Down, good","label":"Churn","large":false,"value":"1.8%","value_tone":"Neutral"}`
- **Value tone: Accent**: 134 × 58 pt, settings `{"caption":"","delta":"","delta_direction":"Up","label":"Revenue","large":false,"value":"$128,450","value_tone":"Accent"}`
- **Value tone: Success**: 134 × 58 pt, settings `{"caption":"","delta":"","delta_direction":"Up","label":"Revenue","large":false,"value":"$128,450","value_tone":"Success"}`
- **Value tone: Warning**: 134 × 58 pt, settings `{"caption":"","delta":"","delta_direction":"Up","label":"Revenue","large":false,"value":"$128,450","value_tone":"Warning"}`
- **Value tone: Danger**: 134 × 58 pt, settings `{"caption":"","delta":"","delta_direction":"Up","label":"Revenue","large":false,"value":"$128,450","value_tone":"Danger"}`

## Sizing

Hugs its content: 134 × 86 pt in a parent of 760 pt.

## Behavior

- **Accessibility**: role `Label` "REVENUE".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Label "REVENUE" | 60.0 × 16.0 | — | — | — | Inter 600 12.0px ink-muted | — | 0.0 |
| Example | Label "$128,450" | 134.0 × 36.0 | — | — | — | Inter 600 30.0px ink | — | 0.0 |
| Example | Label "+12.4%" | 47.0 × 16.0 | — | — | — | Inter 600 12.0px mix(success, ink, 70) | — | 0.0 |
| Large | Label "$128,450" | 189.0 × 48.0 | — | — | — | Inter 600 42.0px ink | — | 0.0 |
| With a caption | Label "The oldest waits 40 s" | 132.0 × 18.0 | — | — | — | Inter 400 13.0px ink-muted | — | 0.0 |
| Delta: Down | Label "-3.1%" | 35.0 × 16.0 | — | — | — | Inter 600 12.0px mix(ink, danger, 30) | — | 0.0 |
| Value tone: Accent | Label "$128,450" | 134.0 × 36.0 | — | — | — | Inter 600 30.0px #a1b1f8 | — | 0.0 |
| Value tone: Success | Label "$128,450" | 134.0 × 36.0 | — | — | — | Inter 600 30.0px mix(success, ink, 70) | — | 0.0 |
| Value tone: Warning | Label "$128,450" | 134.0 × 36.0 | — | — | — | Inter 600 30.0px mix(ink, warning, 30) | — | 0.0 |
| Value tone: Danger | Label "$128,450" | 134.0 × 36.0 | — | — | — | Inter 600 30.0px mix(ink, danger, 30) | — | 0.0 |

## API

Builder methods: `caption`, `delta`, `delta_toned`, `large`, `new`, `value_tone`.

## Example

```rust
fn stat() -> impl View {
    HStack::new((
        Stat::new("Revenue", "R$ 128.450").delta("+12.4%", true),
        Stat::new("Churn", "1.8%").delta("-0.4%", false),
    ))
    .spacing(32.0)
}
```

Every number above is in `Stat.json` beside this card.
