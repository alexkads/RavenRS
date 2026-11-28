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

From a checkout: `cargo run --manifest-path tools/raven-movie-editor/Cargo.toml`.

## Limits

- It edits RavenRS films, not video files: Raven Channel Video writes those.

## As a product

It follows [tools/README.md](../README.md): its own version (`Cargo.toml`), the news of each version (`novidades.json`, `historico-de-novidades.json`), the installer made from `crates/ravenrs-app-update/install.cmd.in` (`install.cmd`), `--versao` and `--self-test`, and `tests/suite/`, which runs both on the binary.

## End-to-end tests (Raven Test Studio)

The scenarios of this tool are in `tests/e2e/`, as `.raventest` files the Raven Test Studio reads: on a made-up film of one second, it puts a chapter and a title card at the needle and saves; the test reopens the saved film and checks the chapter. Saving writes into a folder of its own under the temporary folder, or the one `RAVEN_MOVIE_EDITOR_E2E_DIR` names. The tool brings them (`SCENARIOS`), and `tests/suite/scenarios.rs` plays them on every `cargo test`.

```bash
raven-test-studio --project tools/raven-movie-editor --run-all --no-cache --format json --out target/e2e/raven-movie-editor
```

The tool answers the Studio when opened with `--e2e-serve`, with no window; its name on the channel is "Raven Movie Editor" (`[package.metadata.ravenrs.e2e]` in `Cargo.toml`).

