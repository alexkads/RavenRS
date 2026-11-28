# Raven Channel Video (RCV)

Records the videos of the RavenRS channel from the real apps: it drives them as a person would, narrates the boxes with the local Kokoro voice and writes the wide and vertical videos, subtitles, chapters and cover.

## When to use it

- To record a new video of the channel, or record one again after the apps changed.
- To look at single moments of a film as pictures before exporting.

## Commands

| Command | What it does |
|---|---|
| `raven-channel-video record OUT` | Writes the films into OUT. |
| `raven-channel-video narrate OUT --voz pm_santa` | Records what the boxes say with the local voice (pf_dora, pm_alex or pm_santa). |
| `raven-channel-video export OUT` | Writes the wide and the vertical video, the subtitles, the chapters and a cover. |
| `raven-channel-video frames OUT 0:6000` | Draws single moments (film:ms) as pictures. |
| `raven-channel-video --self-test` | Builds the screens it records with no window; exit code 0 when they draw. |
| `raven-channel-video --versao` | Prints the version. |

## Install

Like the other RavenRS tools, it is installed by compiling the main branch on your machine, and it updates itself the same way. On macOS and Linux:

```bash
curl -fsSL https://raw.githubusercontent.com/alexkads/RavenRS/master/tools/raven-channel-video/install.cmd -o /tmp/install.cmd && sh /tmp/install.cmd
```

On Windows, in PowerShell:

```powershell
irm https://raw.githubusercontent.com/alexkads/RavenRS/master/tools/raven-channel-video/install.cmd -OutFile "$env:TEMP\install.cmd"; & "$env:TEMP\install.cmd"
```

From a checkout: `cargo run --manifest-path tools/raven-channel-video/Cargo.toml`.

## Limits

- It has no window: it is run from the terminal.
- The voice needs the local models (FF_AI_MODELS).

## As a product

It follows [tools/README.md](../README.md): its own version (`Cargo.toml`), the news of each version (`novidades.json`, `historico-de-novidades.json`), the installer made from `crates/ravenrs-app-update/install.cmd.in` (`install.cmd`), `--versao` and `--self-test`, and `tests/suite/`, which runs both on the binary.
