# Raven Movie Editor (RME)

Opens a RavenRS film (.ravenmovie) or a bug report (.ravenbug) and edits it on a timeline: cuts, boxes, narration and chapters. The films come from the Raven Test Studio, the trainings and the bug reports of the apps.

## When to use it

- To look at the film of a test that failed in the Raven Test Studio.
- To edit a training film or the film of a bug report before sharing it.

## Commands

| Command | What it does |
|---|---|
| `raven-movie-editor [<file.ravenmovie | file.ravenbug>]` | Opens the film, or asks for one; a bug report prints what it says in the terminal. Saving keeps the film beside it. Closing the window keeps it beside the clock. |
| `raven-movie-editor --self-test` | Opens the editor on a made-up film with no window; exit code 0 when it draws. |
| `raven-movie-editor --versao` | Prints the version. |

## Install

Like the other RavenRS tools, it is installed by compiling the main branch on your machine, and it updates itself the same way. On macOS and Linux:

```bash
curl -fsSL https://raw.githubusercontent.com/alexkads/RavenRS/master/tools/raven-movie-editor/install.cmd -o /tmp/install.cmd && sh /tmp/install.cmd
```

On Windows, in PowerShell:

```powershell
irm https://raw.githubusercontent.com/alexkads/RavenRS/master/tools/raven-movie-editor/install.cmd -OutFile "$env:TEMP\install.cmd"; & "$env:TEMP\install.cmd"
```

From a checkout: `cargo run -p raven-movie-editor`.

## Limits

- It edits RavenRS films, not video files: Raven Channel Video writes those.

## As a product

It follows [tools/README.md](../README.md): its own version (`Cargo.toml`), the news of each version (`novidades.json`, `historico-de-novidades.json`), the installer made from `crates/ravenrs-app-update/install.cmd.in` (`install.cmd`), `--versao` and `--self-test`, and `tests/suite/`, which runs both on the binary.
