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

The catalog has Gemma 3, Qwen 2.5, Qwen3, Llama 3.2, Phi-3 and Mistral, from 0.4 to 5 GB, and says how many requests each understood when that was measured. Measured with Qwen3 1.7B on an Apple M2 Pro: 1.1 GB to download, 4.3 GB of memory at the peak, about
1 second for a short answer, and about 3.3 MB more in the app. Asked to choose the right moment among
16 passages, it did worse than the text search, so it writes answers about what the search found. Asked to choose one of eight tools, it got most
requests right, so `tool_rules` and `read_tool_call` let it choose what the app should do. It only
chooses: the app reads the data and does the work, so a model never makes up a number, and whatever
it answers beyond the tools offered is no choice at all.
