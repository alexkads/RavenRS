# Raven Report Designer (RRD)

The report designer of RavenRS Reporting for the people who use the reports: bands, fields, expressions, preview and export to PDF and XLSX, on the desktop and in the browser.

## When to use it

- To design or change a report of RavenRS Reporting and see it with data.
- To try ravenrs-report-designer in a real app before putting it in your own.

## Commands

| Command | What it does |
|---|---|
| `raven-report-designer` | Opens the designer with a sample report. Closing the window keeps it beside the clock. |
| `raven-report-designer --self-test` | Builds the designer with no window; exit code 0 when it draws. |
| `raven-report-designer --versao` | Prints the version. |

## Install

Like the other RavenRS tools, it is installed by compiling the main branch on your machine, and it updates itself the same way. On macOS and Linux:

```bash
curl -fsSL https://raw.githubusercontent.com/alexkads/RavenRS/master/tools/raven-report-designer/install.cmd -o /tmp/install.cmd && sh /tmp/install.cmd
```

On Windows, in PowerShell:

```powershell
irm https://raw.githubusercontent.com/alexkads/RavenRS/master/tools/raven-report-designer/install.cmd -OutFile "$env:TEMP\install.cmd"; & "$env:TEMP\install.cmd"
```

From a checkout: `cargo run -p raven-report-designer`.

## Limits

- The web build is made with cargo xtask web raven-report-designer.

## As a product

It follows [tools/README.md](../README.md): its own version (`Cargo.toml`), the news of each version (`novidades.json`, `historico-de-novidades.json`), the installer made from `crates/ravenrs-app-update/install.cmd.in` (`install.cmd`), `--versao` and `--self-test`, and `tests/suite/`, which runs both on the binary.

## End-to-end tests (Raven Test Studio)

The scenarios of this tool are in `tests/e2e/`, as `.raventest` files the Raven Test Studio reads: the designer opens on the gallery of templates; the invoice opens in the preview, with its page, and the JSON opens from its tab. The tool brings them (`SCENARIOS`), and `tests/suite/scenarios.rs` plays them on every `cargo test`.

```bash
raven-test-studio --project tools/raven-report-designer --run-all --no-cache --format json --out target/e2e/raven-report-designer
```

The tool answers the Studio when opened with `--e2e-serve`, with no window; its name on the channel is "Raven Report Designer" (`[package.metadata.ravenrs.e2e]` in `Cargo.toml`).

