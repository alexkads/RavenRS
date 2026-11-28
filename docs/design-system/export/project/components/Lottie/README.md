# Lottie

Plays a Lottie animation, as LottieFiles keeps them, drawn by RavenRS.

**Family:** Media · **Rust:** `ravenrs::prelude::Lottie`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `size` | number 24–320 by 4 | `96` |  |
| `looping` | on/off | `true` |  |
| `speed` | number 0.25–3 by 0.25 | `1` |  |
| `alt` | text | `A ring that draws itself` | What a screen reader says. |

## Stories

- **Loading ring**: 96 × 96 pt, settings `{"alt":"A ring that draws itself","looping":true,"size":96.0,"speed":1.0}`
- **Large**: 192 × 192 pt, settings `{"alt":"A ring that draws itself","looping":true,"size":192.0,"speed":1.0}`
- **Once**: 96 × 96 pt, settings `{"alt":"A ring that draws itself","looping":false,"size":96.0,"speed":1.0}`

## Sizing

Hugs its content: 96 × 96 pt in a parent of 760 pt.

## Behavior

- **Accessibility**: role `Image` "A ring that draws itself".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Loading ring | Image "A ring that draws itself" | 96.0 × 96.0 | — | — | — | — | — | — |
| Large | Image "A ring that draws itself" | 192.0 × 192.0 | — | — | — | — | — | — |

## API

Builder methods: `alt`, `looping`, `new`, `playing`, `size`, `speed`.

## Example

```rust
/// A Lottie file: a ring that draws itself, on and on. Any file from LottieFiles goes here.
const LOADING_RING: &str = r#"{"v":"5.7.0","fr":30,"ip":0,"op":60,"w":100,"h":100,"layers":[{"ty":4,"ks":{},
"ip":0,"op":60,"st":0,"shapes":[{"ty":"gr","it":[{"ty":"el","p":{"a":0,"k":[50,50]},"s":{"a":0,"k":[70,70]}},
{"ty":"tm","s":{"a":1,"k":[{"t":0,"s":[0],"o":{"x":[0.4],"y":[0]},"i":{"x":[0.2],"y":[1]}},{"t":30,"s":[0],
"o":{"x":[0.4],"y":[0]},"i":{"x":[0.2],"y":[1]}},{"t":60,"s":[100]}]},"e":{"a":1,"k":[{"t":0,"s":[0],
"o":{"x":[0.4],"y":[0]},"i":{"x":[0.2],"y":[1]}},{"t":30,"s":[100]}]},"o":{"a":0,"k":0},"m":1},
{"ty":"st","c":{"a":0,"k":[0.25,0.34,0.86,1]},"o":{"a":0,"k":100},"w":{"a":0,"k":9},"lc":2,"lj":2},
{"ty":"tr","a":{"a":0,"k":[0,0]},"p":{"a":0,"k":[0,0]},"s":{"a":0,"k":[100,100]},"r":{"a":0,"k":0},"o":{"a":0,"k":100}}]}]}]}"#;

fn lottie() -> impl View {
    match LottieAnimation::from_json(LOADING_RING.as_bytes()) {
        Ok(animation) => Some(Lottie::new(animation).size(96.0, 96.0).alt("A ring that draws itself")),
        Err(_) => None,
    }
}
```

Every number above is in `Lottie.json` beside this card.
