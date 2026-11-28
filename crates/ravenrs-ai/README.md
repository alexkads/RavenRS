# ravenrs-ai

Local AI models for a RavenRS app: models that write text, one that hears and two that speak. Nothing
here talks to the network except the download someone asks for, and what is asked of a model stays
on the machine. Everything runs on the CPU in plain Rust with
[candle](https://github.com/huggingface/candle), so building an app needs nothing but Cargo: no
Python, no ONNX runtime, no espeak-ng.

## What is here

| Type | What it is for | Needs the UI thread? |
|---|---|---|
| `Machine`, `LocalModel::fits` | How much memory and disk there are, and whether a model fits. Models that do not fit are not offered. | No |
| `ModelFolder` | Downloads a model's files with progress and cancelling, checks each SHA-256 before installing, and removes them. A broken download never looks installed. | No (it blocks) |
| `CATALOG`, `VOICE_CATALOG`, `HEARING_CATALOG`, `model_by_id` | The models this crate knows, each pinned to a commit and a hash. | No |
| `ModelManager` | The screen for all of it: lists what fits, downloads, cancels, tries for speed, puts to use and removes. | Yes, it is a `View` |
| `LocalEngine` | A text model in memory: `generate` blocks until the answer is written. | No (it blocks) |
| `LocalAssistant` | A `LocalEngine` on its own thread, which lets go of its memory after two minutes without a question. | Built from the UI |
| `tool_rules`, `read_tool_call`, `Tool`, `ToolCall` | Lets a text model choose one of the app's tools; the app does the work. | No |
| `SpeechEngine` | A voice in memory: `speak` turns text into 24 kHz mono samples. | No (it blocks) |
| `LocalVoice` | A `SpeechEngine` on its own thread, which hands back ready clips one sentence at a time. | Built from the UI |
| `VoiceManager` | The screen for the voice: which model, which speaker, read answers aloud or not. | Yes, it is a `View` |
| `Transcriber` | Whisper, which turns 16 kHz samples into text. | No (it blocks) |

Everything that blocks (`download`, `LocalEngine::load`, `SpeechEngine::load`, `generate`, `speak`)
belongs on a thread of its own. The `Local…` types already do that for you.

## Text models

The catalog has LFM3, Gemma 3, Qwen 2.5, Qwen3, Llama 3.2, Phi-3 and Mistral, from 0.4 to 5 GB, and
says how many requests each understood when that was measured. Measured with Qwen3 1.7B on an
Apple M2 Pro: 1.1 GB to download, 4.3 GB of memory at the peak, about 1 second for a short answer,
and about 3.3 MB more in the app.

Asked to choose the right moment among 16 passages, it did worse than the text search, so it writes
answers about what the search found. Asked to choose one of eight tools, it got most requests right,
so `tool_rules` and `read_tool_call` let it choose what the app should do. It only chooses: the app
reads the data and does the work, so a model never makes up a number, and whatever it answers beyond
the tools offered is no choice at all.

LFM3 is not in candle yet, so this crate reads it itself (`src/lfm3.rs`). LFM3 1.7B needs 4 GB of
memory at the peak and chose the right tool in 27 of 32 requests. Its license, LFM Open 1.0, allows
commercial use only below 10 million dollars of annual revenue.

## Voice

`VOICE_CATALOG` has two models. Both answer through the same `SpeechEngine`, which picks the code by
`LocalModel::family`.

| | Kokoro 82M (`kokoro-82m`) | LFM3 Audio 2B (`lfm3-audio-2b-q4`) |
|---|---|---|
| License | Apache 2.0 | LFM Open 1.0 |
| Download | 329 MB | 1.3 GB, quantized GGUF |
| Memory at the peak | 1.7 GB | 4.5 GB |
| Speed on an Apple M2 Pro | about 2.5 times the speed of speech | about the speed of speech |
| Languages | Brazilian Portuguese only | the 19 of `SPEECH_LANGUAGES` |
| Voices | `pf_dora`, `pm_alex`, `pm_santa` (`KOKORO_SPEAKERS`) | female or male (`SpeechVoice`) |
| What of `SpeechSettings` it reads | `language` (must be `"Portuguese"`), `speaker` or else `voice`, `seed` | all of it: the prompt, sampling, repetition, `seed` |

Kokoro is the voice RavenRS uses: `pf_dora` was chosen for the channel videos and `pm_santa` for the
training films of Raven Finanças. Without a `speaker`, Kokoro speaks as `pf_dora` for
`SpeechVoice::Female` and `pm_alex` for `SpeechVoice::Male`.

### Three ways in

Pick the lowest level that does the job.

**1. `SpeechEngine`: text in, samples out.** No UI, no threads, no RavenRS app needed. This is the
one to use from a command line, a test, a build step or an app with another UI toolkit.

```rust,no_run
use ravenrs_ai::{ModelFolder, SPEECH_RATE, SpeechEngine, SpeechSettings, model_by_id};
use std::sync::atomic::AtomicBool;

let folder = ModelFolder::new("/path/to/models");
let model = model_by_id("kokoro-82m").expect("in the catalog");
// Once: about 329 MB, each file checked by its hash. Blocks until the end.
folder.download(&model, &|received, total| eprintln!("{received} of {total}"), &AtomicBool::new(false))?;

// Takes a few seconds; keep the engine and reuse it.
let mut engine = SpeechEngine::load(&folder, &model)?;
let settings = SpeechSettings { speaker: Some("pm_santa".into()), ..SpeechSettings::default() };

let text = "Olá! Este é o seu resumo do mês.";
let mut sound: Vec<f32> = Vec::new();
for piece in engine.pieces(text) {
    // `go_on` hears how many pieces are done; returning false stops with what is ready.
    sound.extend(engine.speak(&piece, &settings, &mut |_| true)?);
}
// Mono samples between -1 and 1, at SPEECH_RATE (24 000) per second.
eprintln!("{:.1} s of speech", sound.len() as f32 / SPEECH_RATE as f32);
# Ok::<(), Box<dyn std::error::Error>>(())
```

`speak` can take a whole paragraph, but `pieces` is better: Kokoro says short sentences weakly, so
`pieces` joins those under 60 letters with the next, and each piece comes back sooner than the whole.

**2. `LocalVoice`: the same on its own thread, inside a RavenRS app.** It loads the model on the
first `say`, speaks a piece at a time, and lets go of the memory after two minutes of silence.
`say` replaces anything still being said; `stop` silences it. When a piece is ready it calls the
`UiWaker`, and the next build plays it:

```rust,no_run
use ravenrs_ai::{LocalVoice, ModelFolder, SpeechSettings, model_by_id};
use ravenrs_widgets::{Sound, UiWaker};

let voice = LocalVoice::new(ModelFolder::new("/path/to/models"), model_by_id("kokoro-82m").unwrap());
voice.say("Você tem três contas para pagar esta semana.".into(), SpeechSettings::default(), UiWaker::default());

// In the `build` of a view that stays on screen while it speaks, with `cx.sound()`:
fn build_part(voice: &LocalVoice, sound: &Sound) {
    voice.play_ready(sound);
}
```

**3. `VoiceManager`: the screen and the settings.** It is a `View` with the model list, the speaker,
"read answers aloud" and, for LFM3 Audio, the sampling sliders. It keeps them in
`voice-settings.json` in the models' folder and holds a `LocalVoice` for the model in use. The
assistant of Raven Finanças (`examples/family-finance/src/ui/assistant.rs`) uses it as is:
`voices.say(spoken(text), waker)` when an answer arrives, and `voices.play_ready(cx.sound())` while
building.

### Live, or recorded ahead

A voice can speak **live**, as the assistant does, or be **recorded ahead** of time, as the training
films of Raven Finanças are. Recorded ahead, whoever watches needs no model, no download and no
memory for it: the app carries the sound.

How the training narration is made (`record_narration` in
`examples/family-finance/tests/suite/training.rs`):

1. It gathers every text the films say: each hotspot's title and text, and the opening and closing
   cards.
2. Each text goes through `family_finance::ui::spoken` first, which writes money, dates and numbers
   in words (`R$ 1.234,56` → "mil duzentos e trinta e quatro reais e cinquenta e seis centavos"),
   since Kokoro leaves digits out.
3. `SpeechEngine` with `kokoro-82m` and `speaker: Some("pm_santa")` says each piece of
   `engine.pieces(text)`, with a quarter of a second of silence after each.
4. `ravenrs_movie::Narration::pack(&recordings, SPEECH_RATE)` packs every `(text, samples)` into one
   file, `assets/treinamentos/narracao.voz` (about 5 MB for every film: 16 kHz, ADPCM, one level of
   loudness for all).
5. The app embeds it with `include_bytes!` and gives `Narration::new(bytes)` to the films. A text is
   found by its exact words; a text that changed and was not recorded again is said by the system's
   voice until it is.

```sh
# Everything again, about 25 minutes:
FF_AI_MODELS=/path/to/models cargo test --release --manifest-path examples/family-finance/Cargo.toml --test suite training::record_narration -- --ignored --nocapture
# Only what narracao.voz does not say yet, added to it:
FF_NARRATION_ADD=1 FF_AI_MODELS=/path/to/models cargo test --release --manifest-path examples/family-finance/Cargo.toml --test suite training::record_narration -- --ignored --nocapture
# Six takes of texts 12 and 40, as WAV, to choose by ear; then pack the folder:
FF_NARRATION_TAKES=12,40 FF_NARRATION_WAVS=/path/to/wavs FF_AI_MODELS=/path/to/models cargo test --release --manifest-path examples/family-finance/Cargo.toml --test suite training::record_narration -- --ignored
FF_NARRATION_WAVS=/path/to/wavs cargo test --release --manifest-path examples/family-finance/Cargo.toml --test suite training::pack_narration -- --ignored
```

`FF_NARRATION_SPEAKER` picks another Kokoro voice, `FF_NARRATION_MODEL` another model, and
`FF_NARRATION_MISSING=1` records only what a WAV folder does not have yet. The videos of the channel
are made the same way, in `tools/raven-channel-video/src/main.rs`. Check new speech with an independent
recognizer (Whisper, `Transcriber`) rather than only reading the text: a missing phoneme rule is heard,
not seen.

## How Kokoro speaks

Kokoro is a StyleTTS 2 model with an iSTFTNet decoder. `src/kokoro.rs` follows its published PyTorch
code (`kokoro/model.py`, `modules.py`, `istftnet.py`) stage by stage, and the test
`kokoro_matches_the_published_code` compares each stage with numbers that code wrote. One call to
`SpeechEngine::speak` goes:

1. **Text to phonemes** (`src/phonemes.rs`, `phonemes_pt_br`). This crate's own rules for Brazilian
   Portuguese, written from the spelling: accents mark the stress, the ending decides it when there
   is none, a short list keeps the open vowels spelling does not show, and words change before the
   next one (`as contas do banco` → `as kˈoŋtæz dʊ bˈɐ̃ŋkʊ`). It replaces espeak-ng, which is GPL. On
   the films of Raven Finanças the result matches espeak-ng in 99.8% of the characters, and in 99.4%
   on sentences the rules were not written from. Digits are left out.
2. **Phonemes to pieces** (`phoneme_pieces` in `src/speech.rs`): at most 400 phonemes, cut where a
   sentence or clause ends, and never more than the 510 the model takes, since it rushes at the end
   of its longest.
3. **Phonemes to numbers** (`Kokoro::ids`): each phoneme is looked up in the `vocab` of
   `config.json`; what it does not know is dropped, and a 0 opens and closes the piece.
4. **The voice's style** (`Voice::style`). A voice file (`pf_dora.pt`…) is a PyTorch zip of
   `(510, 1, 256)` floats: one style for each length of text. The row for the number of phonemes is
   taken; its first 128 numbers are the timbre, the last 128 the prosody.
5. **Reading** (`Albert`): a small ALBERT, one layer used twelve times, reads the ids; a linear layer
   brings it to 512 channels.
6. **Durations** (`Predictor::encode`, `durations`): LSTMs over the reading and the prosody say how
   many frames each phoneme lasts, rounded as PyTorch rounds. `speed` divides them; it is fixed at 1.
7. **Pitch and loudness** (`pitch_and_loudness`): each phoneme's features are repeated for its
   frames, and two stacks of style-conditioned blocks (AdaIN) write the pitch (F0) and loudness of
   every half frame.
8. **Sound** (`Decoder`). A text encoder reads the ids again for the sound itself. The decoder joins
   that with pitch, loudness and the timbre. A source of sines at the pitch and its eight harmonics,
   with noise where the voice is unvoiced, is shaped by two transposed convolutions (×10, then ×6)
   and snake residual blocks into a spectrum of 11 magnitudes and 11 phases, and an inverse STFT
   (`src/spectrum.rs`, 20-point Hann window, hop 5) turns it into samples. Each frame of step 6
   becomes 600 samples: 25 ms at 24 kHz.

The noise of step 8 comes from `SpeechSettings::seed`, with its own generator (`Noise`), so the same
text, voice and seed sound the same on every machine. It is not bit for bit the published code: the
near-silent bins change phase with the smallest difference, so two runs of the PyTorch code with
different seeds differ by about as much as this one differs from it.

## Using the voice outside a RavenRS app

`SpeechEngine` needs none of the UI: give it a `ModelFolder` and a `LocalModel`, and it gives back
`Vec<f32>`. An app with another toolkit (GPUI, egui, a server) uses only that, and plays the samples
with whatever it already has. With [rodio](https://crates.io/crates/rodio):

```rust,ignore
use ravenrs_ai::{ModelFolder, SPEECH_RATE, SpeechEngine, SpeechSettings, model_by_id};
use std::sync::mpsc;

// A thread of its own: loading takes seconds and each piece blocks.
let (say, heard) = mpsc::channel::<String>();
std::thread::spawn(move || {
    let model = model_by_id("kokoro-82m").unwrap();
    let mut engine = SpeechEngine::load(&ModelFolder::new(models_dir), &model).unwrap();
    let settings = SpeechSettings { speaker: Some("pf_dora".into()), ..SpeechSettings::default() };
    let stream = rodio::OutputStreamBuilder::open_default_stream().unwrap();
    let sink = rodio::Sink::connect_new(stream.mixer());
    for text in heard {
        sink.stop();
        for piece in engine.pieces(&text) {
            let sound = engine.speak(&piece, &settings, &mut |_| true).unwrap();
            // Each piece queues behind the last, so speech starts after the first.
            sink.append(rodio::buffer::SamplesBuffer::new(1, SPEECH_RATE, sound));
        }
    }
});
say.send("Bem-vindo de volta.".into()).unwrap();
```

What to know before bringing it in:

- **Dependencies.** The crate depends on `ravenrs-widgets` for `ModelManager`, `VoiceManager` and
  `LocalVoice`, so it compiles them even when only `SpeechEngine` is used. They do not draw anything
  until asked, and nothing of them conflicts with another toolkit, but the build is longer.
- **Model files.** Download them with `ModelFolder::download` (or point a `ModelFolder` at a folder
  that already has `kokoro-82m/` from another app); see the layout below. The URLs are pinned to a
  commit of `hexgrad/Kokoro-82M` on Hugging Face and each file is checked by its SHA-256.
- **Text.** Write numbers, money, dates and acronyms in words before `speak`
  (`examples/family-finance/src/ui/spoken.rs` is one way). Only Portuguese.
- **Recorded ahead.** For fixed texts (a tour, a tutorial), record once and ship the sound, as
  described above, so the people who use the app download nothing.

## Files, memory and folder

`ModelFolder::new(root)` keeps each model in `root/<id>/`, under the names of its `ModelFile`s:

```text
root/
  kokoro-82m/
    model.pth      327 MB  the weights (kokoro-v1_0.pth)
    config.json      2 KB  the phoneme vocabulary
    pf_dora.pt     523 KB  one voice each
    pm_alex.pt
    pm_santa.pt
  voice-settings.json      what VoiceManager remembers
```

A model is installed when every file is there with the right size; the hash is checked when it
arrives, and a download in progress has its own name, so it never looks installed. `LocalModel::fits`
asks for the model's peak memory (1.7 GB for Kokoro) plus room for the system, and disk for what is
not there yet.

## Limits

- Kokoro leaves digits out: write numbers in words (alexkads/RavenRS#96).
- Kokoro speaks only Brazilian Portuguese, with the three voices above; LFM3 Audio speaks the others.
- CPU only. Measured on macOS; Linux and Windows are not checked yet (alexkads/RavenRS#98).
- Speech recognition (`Transcriber`, Whisper) exists, but capturing the microphone does not, so there
  is no voice chat yet.

## Checking by ear

These need the real model and run only when asked (`--ignored`), in release:

```sh
# Three sentences, each with another Kokoro voice, into RAVENRS_AI_MODELS/fala-N.wav, with timings:
RAVENRS_AI_MODELS=/path/to/models cargo test --release --manifest-path camadas/automacao/Cargo.toml -p ravenrs-ai --test manager real_speech -- --ignored --nocapture
# One sentence, one voice, one file. This one reads a clone of hexgrad/Kokoro-82M as Hugging Face
# names it (kokoro-v1_0.pth, config.json, voices/pm_alex.pt), not a ModelFolder; RAVENRS_SAY_PHONEMES
# gives the phonemes as they are, to hear the model apart from the rules:
RAVENRS_KOKORO=/path/to/Kokoro-82M RAVENRS_SAY="Olá, tudo bem?" RAVENRS_VOICE=pm_alex RAVENRS_SAY_OUT=ola.wav cargo test --release --manifest-path camadas/automacao/Cargo.toml -p ravenrs-ai kokoro_says -- --ignored
# Every stage against references written by the published PyTorch code:
RAVENRS_KOKORO=/path/to/Kokoro-82M RAVENRS_KOKORO_REF=/path/to/references cargo test --release --manifest-path camadas/automacao/Cargo.toml -p ravenrs-ai kokoro_matches -- --ignored --nocapture
```

When something sounds wrong, compare the phonemes of the same sentence first
(the test `phonemes_close_to_espeak` in `src/phonemes.rs` does it against what espeak-ng writes): so far a wrong word has been a missing phoneme
rule, not the model.
