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
