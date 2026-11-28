# Filmstrip

A strip of thumbnails of any length, whose cells come from the app.

**Family:** Media · **Rust:** `ravenrs::prelude::Filmstrip`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `mode` | None · Single · Multiple | `Multiple` |  |
| `cell_width` | number 0–480 by 1 | `120` |  |
| `height` | number 0–448 by 1 | `112` |  |
| `label` | text | `` | Empty leaves it as the component has it. |
| `spacing` | number 0–24 by 0.1 | `6` |  |

## Stories

- **Example**: 728 × 112 pt, settings `{"cell_width":120.0,"height":112.0,"label":"","mode":"Multiple","spacing":6.0}`
- **Mode: None**: 728 × 112 pt, settings `{"cell_width":120.0,"height":112.0,"label":"","mode":"None","spacing":6.0}`
- **Mode: Single**: 728 × 112 pt, settings `{"cell_width":120.0,"height":112.0,"label":"","mode":"Single","spacing":6.0}`

## Sizing

Fills the width of its parent: 288, 728, 1168 pt wide in parents of 320, 760, 1200 pt.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover** on "Thumbnails": border {color:border,width:1.0} → {color:ink@40,width:1.0}.
- **Focus** on "Thumbnails": fill warning → ink-muted@35; fill warning → ink-muted@35; fill warning → ink-muted@35; fill warning → ink-muted@35.
- **Pressed** on "Thumbnails": border {color:border,width:1.0} → {color:ink@40,width:1.0}.

## Behavior

- **Events**: `on_activate`.
- **Keys** its first control handles once focused: `ArrowLeft`, `ArrowRight`, `Space`, `Escape`, `Home`, `End`.
- **Accessibility**: role `ListBox` "Thumbnails".
- **Focus order** (Tab): ListBox "Thumbnails" → Button "Scroll back" → Button "Scroll forward".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | ListBox "Thumbnails" | 728.0 × 112.0 | — | — | — | — | chevron-left 20.0px ink@70 | 6.0 |
| Example | ListBoxOption "Photo 1" | 120.0 × 112.0 | — | — | — | — | — | — |
| Example | Image "Rating" | 55.0 × 11.0 | — | — | — | — | — | — |
| Example | ListBoxOption "Photo 2" | 120.0 × 112.0 | radius-sm | bg-100 | 1.0 border | — | — | — |
| Example | ListBoxOption "Photo 3" | 120.0 × 112.0 | radius-sm | bg-100 | 2.0 accent | — | chevron-left 20.0px ink@70 | 6.0 |
| Example | ListBoxOption "Photo 8" | 120.0 × 112.0 | radius-sm | bg-100 | 1.0 border | — | chevron-right 20.0px ink@70 | 72.0 |
| Example | Button "Scroll back" | 32.0 × 112.0 | 0.0 | {"end":[-15.0,-15.5],"from":"bg-200","start":[-16.0,-15.5],"to":"transparent"} | — | — | chevron-left 20.0px ink@70 | 6.0 |
| Example | Button "Scroll forward" | 32.0 × 112.0 | 0.0 | {"end":[-16.0,-15.5],"from":"bg-200","start":[-15.0,-15.5],"to":"transparent"} | — | — | chevron-right 20.0px ink@70 | 6.0 |

## API

Builder methods: `cell_name`, `cell_width`, `fade`, `height`, `key`, `label`, `mode`, `new`, `on_activate`, `selection`, `spacing`.

## Example

```rust
fn filmstrip() -> impl View {
    let selected = State::new(BTreeSet::from([2_usize]));
    Filmstrip::new(5_000, |cell: FilmstripCell| {
        // The cell belongs to the app: its picture, its stars, its badges.
        let shade = (cell.index % 10) as f32 / 10.0;
        let picture = ImageData::from_fn(64, 64, move |x, y| {
            Color::rgb(0.2 + shade * 0.5, 0.3 + y as f32 / 200.0, 0.7 - x as f32 / 200.0)
        });
        let picture: AnyView = match picture {
            Some(picture) => Box::new(Image::new(picture).height(72.0).stretch()),
            None => Box::new(Spacer::new()),
        };
        VStack::new((picture, Rating::read_only((cell.index % 6) as u32).size(11.0)))
            .spacing(4.0)
            .align(Align::Center)
    })
    .cell_name(|index| format!("Photo {}", index + 1))
    .selection(selected.binding())
    .mode(SelectionMode::Multiple)
    .cell_width(120.0)
    .height(112.0)
    .fade(Semantic::SurfaceElevated)
}
```

Every number above is in `Filmstrip.json` beside this card.
