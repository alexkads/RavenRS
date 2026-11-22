# Raven Code (RCD)

A small editor for Rust projects made only of RavenRS components, on the DockArea: it opens a project from disk, edits and saves its files, builds and runs it with cargo and answers through rust-analyzer. The seed of the RavenRS IDE.

## When to use it

- To open and edit a Rust project, build it and run it, in an editor made with RavenRS.
- To try the DockArea, the code editor and the language server together, in a real app.

## Commands

| Command | What it does |
|---|---|
| `raven-code [<folder>]` | Opens the project in the folder, or the sample project with no folder. Closing the window keeps it beside the clock. |
| `raven-code --self-test` | Builds the editor on the sample project with no window; exit code 0 when it draws. |
| `raven-code --versao` | Prints the version. |

## Install

Like the other RavenRS tools, it is installed by compiling the main branch on your machine, and it updates itself the same way. On macOS and Linux:

```bash
curl -fsSL https://raw.githubusercontent.com/alexkads/RavenRS/master/tools/raven-code/install.cmd -o /tmp/install.cmd && sh /tmp/install.cmd
```

On Windows, in PowerShell:

```powershell
irm https://raw.githubusercontent.com/alexkads/RavenRS/master/tools/raven-code/install.cmd -OutFile "$env:TEMP\install.cmd"; & "$env:TEMP\install.cmd"
```

From a checkout: `cargo run -p raven-code`.

## Limits

- rust-analyzer has to be installed for the language features.
- It is the seed of the IDE (#495): debugging and refactoring are not there yet.

## As a product

It follows [tools/README.md](../README.md): its own version (`Cargo.toml`), the news of each version (`novidades.json`, `historico-de-novidades.json`), the installer made from `crates/ravenrs-app-update/install.cmd.in` (`install.cmd`), `--versao` and `--self-test`, and `tests/suite/`, which runs both on the binary.

## End-to-end tests (Raven Test Studio)

The scenarios of this tool are in `tests/e2e/`, as `.raventest` files the Raven Test Studio reads: on the sample project, written into a folder of its own under the temporary folder (`e2e_session`), it replaces the text of `src/main.rs` and saves all; the test then reads the file on disk. The editor of each file is `editor:<path>`. The tool brings them (`SCENARIOS`), and `tests/suite/scenarios.rs` plays them on every `cargo test`.

```bash
raven-test-studio --project tools/raven-code --run-all --no-cache --format json --out target/e2e/raven-code
```

The tool answers the Studio when opened with `--e2e-serve`, with no window; its name on the channel is "Raven Code" (`[package.metadata.ravenrs.e2e]` in `Cargo.toml`).

