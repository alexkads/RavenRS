# ScrollScene

A section that holds its content on screen for a length of scrolling, for steps that change as you scroll.

**Family:** Layout · **Rust:** `ravenrs::prelude::ScrollScene`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `length` | number 0–2400 by 1 | `600` |  |
| `top` | number 0–200 by 1 | `0` | Distance from the top of the scroll container while pinned, in points. |

## Stories

- **Example**: 370 × 280 pt, settings `{"length":600.0,"top":0.0}`

## Sizing

Hugs its content: 370 × 280 pt in a parent of 760 pt; in a parent of 320 pt it narrows to 266 pt; in a parent that stretches its children, as a form column, it takes the whole width.

## Behavior

- **Accessibility**: role `Label` "Scroll down: the scene holds while its steps change.".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Label "Scroll down: the scene holds while its steps change." | 370.0 × 22.0 | — | — | — | Inter 400 15.0px ink-muted | — | 0.0 |
| Example | Heading "1. State" | 77.0 × 28.0 | — | — | — | Inter 600 22.0px ink | — | 0.0 |
| Example | Label "Here the scene lets go." | 370.0 × 22.0 | — | — | — | — | — | — |

## API

Builder methods: `length`, `new`, `top`.

## Example

```rust
fn scroll_scene() -> impl View {
    // Each step fades in, holds for its share of the scene and fades out.
    let step = |title: &'static str, text: &'static str, from: f32, to: f32| {
        let away = Effect::hidden().then(Effect::moved(0.0, 16.0));
        let card = Card::new(
            VStack::new((
                Text::new(title).style(TextStyle::Title),
                Text::new(text).foreground(Semantic::InkMuted),
            ))
            .spacing(8.0),
        );
        let link = ScrollLink::keyframes([
            (from - 0.08, away),
            (from + 0.02, Effect::IDENTITY),
            (to - 0.02, Effect::IDENTITY),
            (to + 0.08, away),
        ]);
        ScrollLinked::new(card, link)
    };
    let steps = ZStack::new((
        step("1. State", "A click changes a value.", -1.0, 0.33),
        step("2. Layout", "Only what read it moves.", 0.33, 0.66),
        step("3. Paint", "The GPU draws the frame.", 0.66, 2.0),
    ));
    ScrollView::new((
        Text::new("Scroll down: the scene holds while its steps change.").foreground(Semantic::InkMuted),
        ScrollScene::new(steps).length(600.0),
        Text::new("Here the scene lets go.").foreground(Semantic::InkMuted),
        Canvas::new(|_, _| {}).size(10.0, 200.0),
    ))
    .spacing(12.0)
    .height(280.0)
}
```

Every number above is in `ScrollScene.json` beside this card.
