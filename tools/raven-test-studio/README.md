# Raven Test Studio

The end-to-end test tool of RavenRS, as an app: a scenario is a flow of blocks (open an app, type, click, check), run with [`ravenrs-e2e`](../../crates/ravenrs-e2e) against an app, each block marked as it goes. It is made only with RavenRS components (`FlowLibrary`, `FlowToolbox`, `FlowCanvas`, `FlowInspector`), and **it tests itself**: the scenario it opens with, "O Studio testa o Studio", opens another Studio and checks that running, stepping, stopping and failing all show as they should.

This is the first step of [#627](https://github.com/alexkads/RavenRS/issues/627), [#634](https://github.com/alexkads/RavenRS/issues/634). The plan, the design and what comes next are there.

## Install it

It is a tool, not an example: it installs by compiling on your computer and updates itself, as Raven Finanças does (see [`tools/README.md`](../README.md)).

```bash
curl -fsSL https://raw.githubusercontent.com/alexkads/RavenRS/master/tools/raven-test-studio/install.cmd -o /tmp/install.cmd && sh /tmp/install.cmd
```

On Windows, in PowerShell:

```powershell
irm https://raw.githubusercontent.com/alexkads/RavenRS/master/tools/raven-test-studio/install.cmd -OutFile "$env:TEMP\install.cmd"; & "$env:TEMP\install.cmd"
```

The version is its own (`version` in `Cargo.toml`), with its news in `novidades.json` and every version in `historico-de-novidades.json`. An installed Studio looks for a new version when it opens and every hour after, and says so in its status bar. `raven-test-studio --versao` prints the version.

## How to use it

**The window**

```bash
cargo run -p raven-test-studio
```

"Rodar" runs the scenario from the start; "Próximo passo" runs one block at a time; "Parar" puts every block back; "Rodar sem cache" runs again even when nothing changed. Each block turns done, failed with the error, or "Não rodou" after a failure, and the log on the right says how each went.

**With no window, for a person or a tool**

```bash
cargo run -p raven-test-studio -- --self-test
cargo run -p raven-test-studio -- --run quebrado --format json
```

When its window opens, the Studio runs its self-test first (from the cache when nothing changed) and says how it went in the status bar.

`--run` takes `autoteste`, `lancamento` or `quebrado`. The text report is a few lines to read or quote; the JSON carries every block with its result, the stable code of a failure (`E2E_NOT_FOUND`…), what it means in Portuguese, the detail from `ravenrs-e2e`, how to fix it, what came close and the screen of the app under test at that moment. Every report ends with what was not verified. The exit code is 0 when the scenario passed and 1 when it did not.

**Handing a scenario to Codex**

```bash
raven-test-studio --delegate codex --run quebrado
```

The Studio asks the Codex CLI installed on the computer, with its cheapest model and reading only, to run the scenario through the Studio's command line, read the report and, when it failed, find the cause in the Rust sources with file and line. Then the Studio runs the same scenario itself and says whether it confirms what Codex said, so a delegated answer is never taken on trust. The cheapest model is read from the list of models the installed Codex keeps (`~/.codex/models_cache.json`), the first it calls affordable; `--model` names another, and `--delegate claude` uses the Claude CLI with `haiku`. With `--format json` the report carries Codex's answer under `delegation`. Codex is called through [`ravenrs-agent-cli`](../../crates/ravenrs-agent-cli), as Raven Flows calls it: it keeps its own login, and every limit is checked against what it says it can do.

**The cache**

A scenario that did not change, on a build that did not change, comes back from the Studio's cache at once and says so. The key joins the scenario's blocks (kind, target, text, in order) and the build (the size and time of the program), so editing a block or building again runs it again. `--no-cache` runs anyway and `--clear-cache` forgets every run. The cache lives in the user's cache folder (`~/Library/Caches/ravenrs/test-studio/` on macOS), outside every project.

**Inside Claude, beside the chat**

[`claude-pane/`](claude-pane) is a Claude Code mod: a pane "Raven Test Studio" beside the conversation, and a tool, `run_scenario`, that the AI calls to run a scenario and read its report. The pane shows the verdict, a bar with one ■ per block, the counts, the failure with its code, meaning, fix and screen, a button that asks the AI to fix it, and what was not verified. Its design is the page "Painel no Claude" of the [canvas](https://claude.ai/artifact/VkLqNQ7QAPULKXoAN1qSWm).

Load it in a session started in this checkout:

```bash
claude --plugin-dir tools/raven-test-studio/claude-pane
```

`/raven-test-studio` opens the pane; asking "roda o autoteste do Studio" makes the AI call the tool, and "delega o teste ao Codex" makes it call the tool with `delegate`. The pane runs the self-test by itself as the session starts. In this checkout it runs the code here (`cargo run`); anywhere else it runs the installed Studio. The plugin also brings the skill `raven-test-studio`, which teaches an agent to run scenarios and read their reports.

**For any AI, through the RavenRS MCP server**

`ravenrs_tools` with name `raven-test-studio` answers with everything the Studio offers, from [`tool.json`](tool.json), and `skills` lists the skill above with the rest of the repository's.

## How it works

- `src/runner.rs`: what each block means (`step_of`: the kind is the action, the subtitle is the target and the text, `#id` for a test id), running it (`perform`), and a run that marks the flow as it goes (`Run`).
- `src/sample.rs`: the apps an Open block can name and the scenarios that come with the Studio, the self-test among them.
- `src/report.rs`: the report for a person (`text`) and for a tool (`json`).
- `src/cache.rs`: the cache of runs.
- `src/delegate.rs`: handing a scenario to Codex or Claude and checking what it says.
- `tool.json`: what the Studio offers, for the MCP server; `tests/suite/manifest.rs` keeps it true.
- `src/lib.rs`: the window, and `open`, which opens the app a block names; a Studio opened there runs its own scenario the same way, which is how the Studio tests the Studio.
- `tests/suite/self_test.rs`: guards that the self-test keeps passing, through the engine and through the window, that a failure carries its code, fix and screen, and that the cache works and is keyed by the build.

## Limits for now

- The app under test runs in the Studio's process. Opening the person's own app in another process is phase F6 of #627.
- Scenarios come with the Studio; scenario files (`.raventest`) are phase F5.
- Linux and Windows have not been run.
- The comparison of each screen with its design board is phase F14.
