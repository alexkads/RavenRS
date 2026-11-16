# tools

The tools of RavenRS: apps that people install and use to build with RavenRS, made with RavenRS. They are not examples. An example shows a component or a pattern and can change any day; a tool is a product, with its own version, an installer, news, and tests that prove it works, and it is treated like Raven Finanças.

## What every tool has

| What | Where | Why |
|---|---|---|
| Its own version, not the workspace's | `version = "x.y.z"` in its `Cargo.toml`, with `publish = false` | raising it is what announces an update to the copies people installed |
| The news of the version and the history of every version | `novidades.json` and `historico-de-novidades.json`, with `contribuidores` and `issues` | the app shows them, and the test `the_three_release_files_agree` keeps them and the version together |
| Who it is to the installer and the updater | `src/versao.rs` with an `InstalledApp` of `ravenrs-app-update` | the same installer and updater as Raven Finanças and Raven Flows |
| The installer | `install.cmd`, made from the shared template (`RAVENRS_WRITE_INSTALLER=1 cargo test -p <tool> versao`) and checked by `the_installer_is_the_one_the_template_makes` | one file that installs by compiling on Windows, macOS and Linux |
| Its icons | `assets/icon.svg`, `icon.png`, `icon.ico`, `icon.icns` | the installer puts them on the app bundle, the Start menu and the desktop entry |
| `--versao` | in `src/main.rs`, before any window | the installer proves a new binary opens before it replaces the old one |
| `--self-test` | in `src/main.rs`, with no window, exit code 0 when it passed | a tool tests itself, before anyone trusts it; its window does the same when it opens |
| What it offers, for AI agents | `tool.json`: summary, when to use it, commands, limits and whatever else it has, kept true by its own tests | the RavenRS MCP server answers `ravenrs_tools` with it, and a skill of the tool's own (`claude-pane/skills/`) is listed by `skills` |
| Beside the clock when its window closes | `AppIdentity::close_to_tray(SystemTray…)` in `src/main.rs`, with the `tray` feature of `ravenrs` and the app's icon | a tool stays ready, as Raven Finanças does, and runs as one copy |
| The window named by its package, with the updater's notice | `AppIdentity::new(APP.package)` and a status bar with the notice | it says when there is a new version, as Raven Finanças does |
| Tests that prove it works, run with no window | `tests/suite/` | a tool is used by people and by AI agents; what it promises is tested |
| A README that explains its role, how it works and how to use it | `README.md` | how people and agents learn it |

`scripts/check-tools.py` checks the list for every folder here.

## The tools

| Tool | What it does |
|---|---|
| [Raven Test Studio](raven-test-studio) | End-to-end tests of RavenRS apps as flows of blocks, reported to a person or to the AI that asked; it tests itself |

## Install one

```bash
curl -fsSL https://raw.githubusercontent.com/alexkads/RavenRS/master/tools/raven-test-studio/install.cmd -o /tmp/install.cmd && sh /tmp/install.cmd
```

On Windows, in PowerShell:

```powershell
irm https://raw.githubusercontent.com/alexkads/RavenRS/master/tools/raven-test-studio/install.cmd -OutFile "$env:TEMP\install.cmd"; & "$env:TEMP\install.cmd"
```
