# SplashScreen

The screen an app shows while it opens: its name, an icon and how far it got.

**Family:** Overlays · **Rust:** `ravenrs::prelude::SplashScreen`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `name` | text | `Raven Finance` |  |
| `status` | text | `Opening the database` | What it is doing now. |
| `progress` | on/off | `false` | A bar in place of the spinner, once there is something to measure. |
| `value` | number 0–1 by 0.05 | `0.6` |  |

## Stories

- **Spinner**: 442 × 625 pt, settings `{"name":"Raven Finance","progress":false,"status":"Opening the database","value":0.6}`
- **Progress**: 524 × 616 pt, settings `{"name":"Raven Finance","progress":true,"status":"Loading your accounts","value":0.6}`

## Sizing

Hugs its content: 760 × 1100 pt in a parent of 760 pt; in a parent of 320 pt it narrows to 320 pt.

## Behavior

- **Accessibility**: role `Dialog` "Raven Finance".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Spinner | Dialog "Raven Finance" | 760.0 × 1100.0 | 0.0 | bg-000 | — | Inter 600 22.0px ink | wallet 64.0px accent | 302.0 |
| Spinner | Label "Raven Finance" | 156.0 × 28.0 | — | — | — | Inter 600 22.0px ink | — | 0.0 |
| Spinner | Status | 136.0 × 18.0 | — | — | — | Inter 400 13.0px ink-muted | — | 0.0 |
| Spinner | Label "Opening the database" | 136.0 × 18.0 | — | — | — | Inter 400 13.0px ink-muted | — | 0.0 |
| Spinner | ProgressIndicator "Loading" | 24.0 × 24.0 | — | — | — | — | — | — |
| Progress | Dialog "Raven Finance" | 760.0 × 1100.0 | 0.0 | bg-000 | — | Inter 600 22.0px ink | wallet 64.0px accent | 302.0 |
| Progress | ProgressIndicator "Loading" | 320.0 × 6.0 | radius-full | bg-300 | — | — | — | — |

## API

Builder methods: `icon`, `logo`, `new`, `progress`, `reveal_after`, `status`.

## Example

```rust
fn splash_screen() -> impl View {
    /// Pretends to load for three seconds. A real app sets the same values as its work advances.
    struct Loading;

    impl View for Loading {
        fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
            const SECONDS: f64 = 3.0;
            let visible = cx.use_state(|| false);
            let started = cx.use_state(|| None::<f64>);
            let now = cx.now();
            let elapsed = match (visible.get(), started.get()) {
                (true, Some(start)) => now - start,
                (true, None) => {
                    started.set(Some(now));
                    0.0
                }
                (false, _) => {
                    started.set_if_changed(None);
                    0.0
                }
            };
            if elapsed >= SECONDS {
                visible.set(false);
            } else if visible.get() {
                cx.wake_at(now + 0.25);
            }
            let status = match elapsed {
                e if e < 1.0 => "Opening the database",
                e if e < 2.0 => "Loading your accounts",
                _ => "Almost there",
            };
            let show = visible.clone();
            let splash =
                SplashScreen::new(visible.binding(), "Raven Finance").icon(IconName::Wallet).status(status);
            // A spinner until there is something to measure, then a bar.
            let splash = if elapsed < 1.0 { splash } else { splash.progress((elapsed / SECONDS) as f32) };
            let page = VStack::new((
                Button::new("Show the splash screen")
                    .style(ButtonStyle::Secondary)
                    .on_click(move || show.set(true)),
                splash,
            ));
            cx.child(&page)
        }
    }

    Loading
}
```

Every number above is in `SplashScreen.json` beside this card.
