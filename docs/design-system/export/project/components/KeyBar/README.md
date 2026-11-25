# KeyBar

The function keys of a counter, each with what it does, the main one stressed.

**Family:** Display and feedback · **Rust:** `ravenrs::prelude::KeyBar`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `primary_label` | text | `Pay` | The words of the key that closes the sale. |
| `primary` | on/off | `true` | Stresses the key, as the main action of the screen. |
| `disabled` | on/off | `true` | Greys out Remove item, as when the sale is empty. |

## Stories

- **Example**: 390 × 48 pt, settings `{"disabled":true,"primary":true,"primary_label":"Pay"}`
- **All keys available**: 390 × 48 pt, settings `{"disabled":false,"primary":true,"primary_label":"Pay"}`
- **No main key**: 390 × 48 pt, settings `{"disabled":true,"primary":false,"primary_label":"Pay"}`

## Sizing

Hugs its content: 390 × 48 pt in a parent of 760 pt; in a parent that stretches its children, as a form column, it takes the whole width.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover** on "F1 Help": fill bg-300 → mix(ink, bg-300, 6).
- **Focus** on "F1 Help": adds a rect (border 2.0 focus-ring).
- **Pressed** on "F1 Help": border {color:border,width:1.0} → {color:border,width:0.97}, fill bg-300 → mix(ink, bg-300, 12), rect [0.0,0.0,105.0,48.0] → [1.57,0.72,101.85,46.56].

## Behavior

- **Accessibility**: role `Button` "F1 Help".
- **Focus order** (Tab): Button "F1 Help" → Button "F9 Pay".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Button "F1 Help" | 105.0 × 48.0 | radius-md | bg-300 | 1.0 border | Inter 600 15.0px ink | — | space-6 |
| Example | Button "F6 Remove item" | 168.0 × 48.0 | radius-md | bg-300@45 | 1.0 border | Inter 600 15.0px ink@45 | — | space-6 |
| Example | Button "F9 Pay" | 101.0 × 48.0 | radius-md | accent | — | Inter 600 15.0px on-accent | — | space-6 |

## API

Builder methods: `item`, `new`.

## Example

```rust
fn key_bar() -> impl View {
    use ravenrs::core::Key;

    let paid = State::new(0);
    let on_key = paid.clone();
    VStack::new((
        KeyBar::new()
            .item(KeyBarItem::new(Hotkey::key(Key::Function(1)), "Help", || {}))
            .item(KeyBarItem::new(Hotkey::key(Key::Function(6)), "Remove item", || {}).disabled(true))
            .item(
                KeyBarItem::new(Hotkey::key(Key::Function(9)), "Pay", move || on_key.update(|n| *n += 1))
                    .primary(),
            ),
        Text::bound(&paid, |n| format!("Paid {n} times")),
    ))
    .spacing(8.0)
}
```

Every number above is in `KeyBar.json` beside this card.
