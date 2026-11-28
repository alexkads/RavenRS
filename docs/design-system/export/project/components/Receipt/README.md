# Receipt

The receipt of a sale on the screen: its lines, the subtotal and the total in a band.

**Family:** Data · **Rust:** `ravenrs::prelude::Receipt`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `title` | text | `Cupom` |  |
| `subtitle` | text | `Caixa 1 · venda 128` |  |
| `lines` | number 0–3 by 1 | `3` | How many lines the sale has. |
| `discount` | on/off | `false` | A discount row under the subtotal. |

## Stories

- **With lines**: 324 × 325 pt, settings `{"discount":false,"lines":3.0,"subtitle":"Caixa 1 · venda 128","title":"Cupom"}`
- **Empty**: 324 × 259 pt, settings `{"discount":false,"lines":0.0,"subtitle":"Caixa 1 · venda 128","title":"Cupom"}`
- **With a discount**: 324 × 353 pt, settings `{"discount":true,"lines":3.0,"subtitle":"Caixa 1 · venda 128","title":"Cupom"}`

## Sizing

Hugs its content: 324 × 325 pt in a parent of 760 pt; in a parent that stretches its children, as a form column, it takes the whole width.

## Behavior

- **Accessibility**: role `Heading` "Cupom".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| With lines | Heading "Cupom" | 60.0 × 24.0 | — | — | — | Inter 600 17.0px ink | — | 0.0 |
| With lines | Label "Caixa 1 · venda 128" | 117.0 × 18.0 | — | — | — | Inter 400 13.0px ink-muted | — | 0.0 |
| With lines | Label "SUBTOTAL (3 ITENS)" | 266.0 × 16.0 | — | — | — | Fira 500 12.0px ink-muted | — | 0.0 |
| With lines | Label "569,90" | 46.0 × 16.0 | — | — | — | Fira 500 12.0px ink | — | 0.0 |
| With lines | Status "TOTAL A RECEBER" | 324.0 × 52.0 | radius-md | ink@8 | — | Fira 500 13.0px ink | — | space-4 |
| With lines | Label "TOTAL A RECEBER" | 161.0 × 18.0 | — | — | — | Fira 500 13.0px ink | — | 0.0 |
| With lines | Heading "R$ 569,90" | 119.0 × 28.0 | — | — | — | Fira 500 22.0px ink | — | 0.0 |
| Empty | Label "The receipt shows here." | 276.0 × 22.0 | — | — | — | Inter 400 15.0px ink-muted | — | 53.0 |
| Empty | Status "TOTAL A RECEBER" | 324.0 × 52.0 | radius-md | ink@8 | — | Fira 500 13.0px ink | — | space-4 |
| Empty | Label "TOTAL A RECEBER" | 187.0 × 18.0 | — | — | — | Fira 500 13.0px ink | — | 0.0 |

## API

Builder methods: `empty_text`, `line`, `new`, `subtitle`, `summary`, `title`, `total`.

## Example

```rust
fn receipt() -> impl View {
    Receipt::new()
        .title("Receipt")
        .line(ReceiptLine::new("PHOTO 4X6 GLOSSY", "30.00").detail("2 x 15.00"))
        .line(ReceiptLine::new("ALBUM 8X12 HARDCOVER", "189.90").detail("1 x 189.90"))
        .summary("SUBTOTAL (2 ITEMS)", "219.90")
        .total("TOTAL DUE", "$ 219.90")
}
```

Every number above is in `Receipt.json` beside this card.
