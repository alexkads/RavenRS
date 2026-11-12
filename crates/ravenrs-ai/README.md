# ravenrs-ai

Local AI models for a RavenRS app. Nothing here talks to the network except the download someone asks
for, and what is asked of a model stays on the machine.

- `Machine` says how much memory and disk there are, and `LocalModel::fits` says whether a model
  fits. Models that do not fit are not offered.
- `ModelFolder` downloads a model's files with progress and cancelling, checks each hash before
  installing, and removes them. A broken download never looks installed.
- `ModelManager` is the screen for all of it: it offers only the models of a list that fit this
  machine, and each can be downloaded, cancelled, tried for speed, put to use and removed.
- `LocalEngine` runs a model on the CPU with [candle](https://github.com/huggingface/candle), which
  is plain Rust, so building an app needs nothing but Cargo. `LocalAssistant` keeps one on its own
  thread and lets go of its memory after two minutes without a question.

The catalog has LFM3, Gemma 3, Qwen 2.5, Qwen3, Llama 3.2, Phi-3 and Mistral, from 0.4 to 5 GB, and says how many requests each understood when that was measured. Measured with Qwen3 1.7B on an Apple M2 Pro: 1.1 GB to download, 4.3 GB of memory at the peak, about
1 second for a short answer, and about 3.3 MB more in the app. Asked to choose the right moment among
16 passages, it did worse than the text search, so it writes answers about what the search found. Asked to choose one of eight tools, it got most
requests right, so `tool_rules` and `read_tool_call` let it choose what the app should do. It only
chooses: the app reads the data and does the work, so a model never makes up a number, and whatever
it answers beyond the tools offered is no choice at all.

LFM3 is not in candle yet, so this crate reads it itself. LFM3 1.7B needs 4 GB of memory at the peak and chose the right tool in 27 of 32 requests. Its license, LFM Open 1.0, allows commercial use only below 10 million dollars of annual revenue.

`VOICE_CATALOG` has Kokoro 82M first, then LFM3 Audio 2B. Kokoro (Apache 2.0, 329 MB) speaks Brazilian Portuguese with three voices, `pf_dora`, `pm_alex` and `pm_santa` (`KOKORO_SPEAKERS`, chosen with `SpeechSettings::speaker`), at about two and a half times the speed of speech on an Apple M2 Pro and 1.7 GB at its peak. The model runs in Rust with `candle`, checked stage by stage against its published PyTorch code, and the phonemes come from this crate's own rules for Brazilian Portuguese, with no espeak-ng: on the films of Raven Finanças they match what espeak-ng writes in 99.8% of the characters, and in 99.4% on sentences the rules were not written from. Numbers are not read yet: write them in words. LFM3 Audio speaks more languages, with the prompt `Perform TTS in <language> using a <female|male> voice.` and its card's sampling values as `SpeechSettings::default`, at about the speed of speech. `SpeechEngine::pieces` says how to split a text: by sentences for LFM3 Audio, joining short ones for Kokoro, which says them weakly. Speech recognition and voice chat are not here yet.
