# AudioPlayerButton

Plays and stops a recording, with progress and a transcript to open.

**Family:** Forms · **Rust:** `ravenrs::prelude::AudioPlayerButton`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `transcript` | text | `What was said goes here, to open when wanted.` |  |
| `loading` | on/off | `false` |  |

## Stories

- **Example**: 322 × 66 pt, settings `{"loading":false,"transcript":"What was said goes here, to open when wanted."}`
- **Loading**: 320 × 66 pt, settings `{"loading":true,"transcript":"What was said goes here, to open when wanted."}`

## Sizing

Hugs its content: 322 × 66 pt in a parent of 760 pt.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover** on "Play audio": fill bg-300 → mix(ink, bg-300, 6).
- **Focus** on "Play audio": adds a rect (border 2.0 focus-ring).
- **Pressed** on "Play audio": border {color:border,width:1.0} → {color:border,width:0.97}, fill bg-300 → mix(ink, bg-300, 12), rect [0.0,0.0,40.0,40.0] → [0.6,0.6,38.8,38.8].

## Behavior

- **Events**: `on_request`, `on_transcribe`.
- **Keys** its first control handles once focused: `Enter`, `Space`.
- **Accessibility**: role `Button` "Play audio".
- **Focus order** (Tab): Button "Play audio" → Button "Transcript".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Button "Play audio" | 40.0 × 40.0 | radius-md | bg-300 | 1.0 border | — | play 16.0px ink | space-3 |
| Example | ProgressIndicator "Audio progress" | 237.0 × 4.0 | radius-full | border | — | — | — | — |
| Example | Label "0:01" | 27.0 × 16.0 | — | — | — | Inter 600 12.0px ink-muted | — | 0.0 |
| Example | Button "Transcript" | 96.0 × 22.0 | radius-md | transparent | — | Inter 600 12.0px ink | chevron-right 12.0px ink | space-2 |
| Loading | Button "Play audio" | 40.0 × 40.0 | radius-md | bg-300@45 | 1.0 border | — | play 16.0px ink@45 | space-3 |

## API

Builder methods: `clip`, `loading`, `new`, `on_request`, `on_transcribe`, `seconds`, `transcribing`, `transcript`, `transcript_open`.

## Example

```rust
fn audio_player_button() -> impl View {
    // A second of a low tone, computed here. An app delivers its own recording.
    let tone: Vec<f32> =
        (0..44_100).map(|i| (i as f32 / 44_100.0 * 220.0 * std::f32::consts::TAU).sin() * 0.2).collect();
    AudioPlayerButton::new()
        .clip(SoundClip::new(tone, 44_100))
        .transcript("What was said goes here, to open when wanted.")
        .width(320.0)
}
```

Every number above is in `AudioPlayerButton.json` beside this card.
