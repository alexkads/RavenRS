# TitleBar

Title, actions and window buttons drawn by the app; does not move the window.

**Family:** Actions and inputs · **Rust:** `ravenrs::prelude::TitleBar`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `active` | on/off | `true` |  |
| `maximized` | on/off | `false` |  |
| `maximized_state` | on/off | `false` | How the example starts. |
| `mobile` | on/off | `false` | The bar on top of a phone's screen. |

## Stories

- **Example**: 728 × 38 pt, settings `{"active":true,"maximized":false,"maximized_state":false,"mobile":false}`
- **Maximized state**: 728 × 38 pt, settings `{"active":true,"maximized":false,"maximized_state":true,"mobile":false}`
- **Mobile**: 728 × 56 pt, settings `{"active":true,"maximized":false,"maximized_state":false,"mobile":true}`

## Sizing

Fills the width of its parent: 288, 728, 1168 pt wide in parents of 320, 760, 1200 pt.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover** on "Search": fill transparent → ink@7.
- **Focus** on "Search": adds a rect (border 2.0 focus-ring).
- **Pressed** on "Search": fill transparent → ink@12, rect [604.0,8.0,22.0,22.0] → [604.33,8.33,21.34,21.34].

## Behavior

- **Events**: `on_close`, `on_context_menu`, `on_maximize`, `on_minimize`.
- **Accessibility**: role `Group` "Report.pdf".
- **Focus order** (Tab): Button "Search" → Button "Minimize" → Button "Maximize" → Button "Close".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Label "Report.pdf" | 560.0 × 18.0 | — | — | — | Inter 400 13.0px ink | — | 0.0 |
| Example | Button "Search" | 22.0 × 22.0 | radius-md | transparent | — | — | search 12.0px ink | 5.0 |
| Example | Button "Minimize" | 22.0 × 22.0 | radius-md | transparent | — | — | minus 12.0px ink | 5.0 |
| Example | Button "Maximize" | 22.0 × 22.0 | radius-md | transparent | — | — | square 12.0px ink | 5.0 |
| Example | Button "Close" | 22.0 × 22.0 | radius-md | transparent | — | — | x 12.0px ink | 5.0 |
| Mobile | Button "Open the menu" | 40.0 × 40.0 | radius-md | transparent | — | — | menu 16.0px ink | space-3 |
| Mobile | Label "Raven CI" | 608.0 × 24.0 | — | — | — | Inter 600 17.0px ink | — | 0.0 |
| Mobile | Button "Search" | 40.0 × 40.0 | radius-md | transparent | — | — | search 16.0px ink | space-3 |

## API

Builder methods: `action`, `active`, `leading`, `maximized`, `mobile`, `new`, `on_close`, `on_context_menu`, `on_maximize`, `on_minimize`.

## Example

```rust
fn title_bar() -> impl View {
    let maximized = State::new(false);
    view_fn(move || {
        let toggle = maximized.clone();
        TitleBar::new("Report.pdf")
            .leading(Icon::new(IconName::File).size(16.0))
            .action(IconButton::new(IconName::Search).label("Search").size(ButtonSize::Mini))
            .maximized(maximized.get())
            .on_minimize(|| {})
            .on_maximize(move || toggle.update(|on| *on = !*on))
            .on_close(|| {})
    })
}
```

Every number above is in `TitleBar.json` beside this card.
