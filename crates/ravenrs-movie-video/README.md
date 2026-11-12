# ravenrs-movie-video

Turns a film of `ravenrs-movie` into an ordinary video file, to publish where the app is not: a
video site, a social network, a presentation.

Each picture is drawn by the RavenRS renderer with no window, from the same views the player uses,
so the video shows what the app shows: the edited film, the mark of what is being explained, its
box, the pointer and the zooms. The sound is the film's clicks and keys and the narration recorded
for each box.

```rust,ignore
use ravenrs_movie_video::{Video, VideoFormat};

let video = Video::new(movie.clone(), VideoFormat::WIDE).narration(narration.clone());
let report = video.write("demo.mov", |done, all| eprintln!("{done}/{all}"))?;
std::fs::write("cover.png", video.thumbnail(12_000)?)?;
std::fs::write("demo.srt", ravenrs_movie_video::subtitles(&[movie.clone()]))?;
std::fs::write("chapters.txt", ravenrs_movie_video::chapters(&[movie]))?;
```

| Format | Size | Layout |
|---|---|---|
| `VideoFormat::WIDE` | 1920 × 1080 | The film fills the picture, boxes over it |
| `VideoFormat::VERTICAL` | 1080 × 1920 | Headline, film and words under it, up to 3 minutes |
| `VideoFormat::PORTRAIT` | 1080 × 1350 | The same, for a post in a feed |
| `VideoFormat::SQUARE` | 1080 × 1080 | The same, square |

On a tall screen the film is small; `Movie::following` brings the camera closer to each spot it
explains, so it stays readable on a phone.

The file is QuickTime (`.mov`) with H.264 pictures and plain 16-bit sound at 48 kHz, which video
sites, players and editors read. The pictures are coded by OpenH264, built from its source, so it
needs no program installed and works the same on macOS, Linux and Windows. It needs a GPU, as the
app does.

`subtitles` writes what the boxes say as `.srt`, and `chapters` writes the chapters as the lines a
video description turns into chapters.

It needs Rust 1.89, one version above the rest of RavenRS, because the SIMD crate the H.264 coder
uses asks for it.
