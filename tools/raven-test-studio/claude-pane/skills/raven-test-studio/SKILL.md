---
name: raven-test-studio
description: How to test a RavenRS screen from the outside with Raven Test Studio (RTS), the end-to-end test tool of RavenRS, and how to read its report - the scenarios, the run_scenario tool, the CLI, the JSON report with stable E2E_* codes, the cache and the self-test. Use whenever you change a RavenRS screen or component and need to prove it still works with real clicks and typing, when asked to run, read or fix an end-to-end test, a scenario, the self-test or "o autoteste do Studio", whenever the user says RTS (Raven Test Studio), or asks to test Raven Finanças in the Studio, or when a report shows an E2E_ code, even if the request does not name the Studio.
---

# Raven Test Studio (RTS)

RTS is how people call it for short: "roda no RTS", "o teste do RTS" mean this Studio.

The Studio runs a scenario, a flow of blocks (open an app, click, type, check, mention), against a RavenRS app with `ravenrs-e2e`, and says how each block went. Its report is made to be acted on: each failure has a stable code, what it means, the detail, how to fix it, the parts that came close and the screen of the app at that moment.

The whole list of what it offers is in `tools/raven-test-studio/tool.json`, the same one the RavenRS MCP server answers from (`ravenrs_tools` with name `raven-test-studio` or `RTS`).

## Running a scenario

Prefer the tool when it is there: `run_scenario` with `scenario` (`autoteste`, `financas`, or the absolute path of a `.raventest` file or folder) returns the JSON report and shows it in the pane beside the chat.

Without the pane, the CLI does the same:

```bash
raven-test-studio --run autoteste --format json
```

From a RavenRS checkout with no install:

```bash
cargo run -q -p raven-test-studio -- --run autoteste --format json
```

The exit code is 0 when it passed, 1 when it failed and 2 when there is no such scenario or the `--format` is not one of `text`, `json`, `jsonl`, `junit`.

`--format junit` gives JUnit XML for a build server, and `--format jsonl` one event per line with the time of each step. `--out <folder>` keeps the run in `<folder>/<scenario>/` (`relatorio.json`, `trace.jsonl` and, on a failure, `arvore.txt` with the app's screen); with `--project` it is the project's `target/e2e`. To read a failure again, open that `arvore.txt` instead of running the scenario again.

## Testing a real app: Raven Finanças

A Studio installed from scratch brings **no test scenarios**. Scenarios are `.raventest` files, and the QA team chooses where they live: in the app's `tests/e2e/` (the app embeds them and hands them to the Studio) or in a folder of their own, run by path. To write one, put it in the app's `tests/e2e/` and add an `include_str!` to the app's list (Raven Finanças: `SCENARIOS` in `src/e2e.rs`; its test fails if a file is not embedded), or keep it in the QA folder.

`financas` opens Raven Finanças **apart**, in a child process with no window, drives it over the test channel (`ravenrs-e2e-wire`) and runs the scenarios it brings in `examples/family-finance/tests/e2e/` (today, `lancar-despesa.raventest`: an expense checked in the month, 9 blocks). The app runs on its sample family in memory on a fixed day, so the database of the person using it is never opened. It never comes from the cache, because the Studio's build says nothing about the app's.

The Studio finds the app in this order: the path in `RAVEN_FINANCAS`; the installed copy, only if `--versao` says 0.1.39 or later (an older one would read `--e2e-serve` as a database path); inside the RavenRS checkout, `cargo run -p family-finance --bin family-finance`. When none fits, the Open block fails saying which to do. A failure to start is `E2E_APP_START` with what the app wrote on leaving; an app that stops answering is ended with `E2E_TIMEOUT`.

In the app, generated forms tag each editor `field:<field>` and their buttons `form:save` and `form:cancel`, so a block writes `#field:description = Mercado`.

## Writing blocks

Blocks are lines of a `.raventest` file: `open`, `click`, `type`, `fill` (replaces what a field has; moves a slider to a number), `select` (picks an option of a combo box, list or tabs), `expand`, `collapse`, `scroll`, `drag` (`drag #from = #to`), `wait` (up to 5 s, `E2E_TIMEOUT` otherwise), `check` and `mention`. They also read in Portuguese: `preencher`, `escolher`, `expandir`, `recolher`, `rolar`, `arrastar`, `esperar`.

Ids that components give a test: `row:<key>` (VirtualList with `key`, DataGrid with `key_column`), `item:<key>` (TreeView), `cell:<key>:<column>`, `column:<column>` and `filter:<column>` (DataGrid; a column is its field, or its title), `node:<id>` and `port:<node>:<port>` (FlowCanvas), `event:<id>` and `resource:<id>` (Scheduler), `panel:<id>`, `floating:<id>` and `floating-move:<id>` (DockArea). Rust tests get drivers for the four with the feature `testing` of `ravenrs-widgets` (`ravenrs_widgets::testing`). `fill #cell:42:Name = Ana` edits a grid cell; `drag #port:a:out = #port:b:in` joins two nodes.

- Every step waits up to 2 s for its target before failing with its own code, so do not add a `wait` before each step. Use it only for what takes longer, such as an answer from a server.
- An action scrolls to a target out of sight by itself.
- To reach a row whatever its position, target it by its key: `#row:<key>` in a `VirtualList` with `key` or a `DataGrid` with `key_column`, `#item:<key>` in a `TreeView`, `#node:<id>` in a `FlowCanvas`.

## Handing it to Codex

When the user asks to delegate the test, or to save your own context on a long diagnosis, hand the scenario to Codex: `run_scenario` with `delegate: "codex"`, or `raven-test-studio --delegate codex --run <scenario>`. Codex runs it with its cheapest model (read from the list the installed Codex keeps; `model` names another), reading only, and answers with the verdict, the cause and the fix with file and line. The Studio then runs the same scenario itself: `delegation.confirmed` says whether it agrees. Report Codex's diagnosis as Codex's, and trust it only when it is confirmed; when it is not, the Studio's own report is the truth. `delegate: "claude"` uses the Claude CLI with `haiku`. In the Studio's window, "Delegar ao Codex" does the same without freezing the screen, and "Parar" ends it.

## A newer Studio

`raven-test-studio --atualizacao --format json` says whether there is one (`newer`, `latest`, `news`), and whether this copy installs itself (`updates_itself`) or comes from a checkout (`git pull`). The pane shows it and offers "Atualizar agora"; tell the user when `run_scenario`'s answer says a new version is out, and install only when they ask (`--atualizar`).

## Reading the report

- `verdict` is `passed` or `failed`; `summary` says it in a line ("18 de 18 passos passaram").
- `blocks` has the blocks that ran, in order. The failed one has `code`, `meaning`, `detail`, `fix`, `close` (what came close) and `screen` (the accessibility tree of the app under test, one `[ref=eN]` per node).
- `total - ran` blocks did not run, because a scenario stops at its first failure.
- `from_cache: true` means nothing changed since the last run: the scenario and the build are the same. Pass `no_cache` (or `--no-cache`) when you changed something the build key cannot see.
- `not_verified` is what this run does not prove. Say it when you report the result; never claim more.

## Fixing a failure

1. Read `code` and `screen` before touching code. `E2E_NOT_FOUND` with the part on the screen under another name means the target is wrong, not the app.
2. A part with no stable name gets `ViewExt::test_id("...")`, as the **last** call of its builder chain (it wraps the view, so methods after it do not exist).
3. `E2E_COVERED` with a dialog open: close it first or scope the target inside the dialog.
4. `E2E_AMBIGUOUS`: a `test_id`, `within`, or `nth`.
5. Run again and report the new verdict with its `not_verified`.

## The self-test

`autoteste` is the Studio testing another Studio: running, stepping, stopping and failing must all show as they should. It is the first thing to run after changing the Studio, `ravenrs-e2e` or the flow components (`FlowCanvas`, `FlowInspector`, `FlowToolbox`, `FlowLibrary`). The window runs it by itself when it opens and shows the result in its status bar, and `scripts/check-tools.py` runs it too.

## Where things are

- `tools/raven-test-studio/src/runner.rs`: what each block means and how it runs.
- `tools/raven-test-studio/src/sample.rs`: the apps and scenarios that come with the Studio.
- `tools/raven-test-studio/src/report.rs`: the text and JSON reports.
- `crates/ravenrs-e2e`: the engine, with `E2e`, `Query`, `Locator` and `Expect`.
- Plan and phases: issue #627 on GitHub.

## Pointing the RTS at a project

When the user points the RTS at a Rust project ("aponta o RTS para a pasta …"), use the `scan_project` tool (or `raven-test-studio --project <folder>`). It reads the project without building it and says, per package made with RavenRS: app or library, its binaries, whether it opens on the test channel, its `.raventest` scenarios and the views it builds with file, line and test ids, plus the next step when something is missing (the channel, or scenarios).

- `action: "run_all"` (`--run-all`) runs the scenarios of every app, opening each from the project's code.
- `action: "suggest"` (`--suggest`): when an app has no scenario, or to find others, an AI reads the app's first screen, its views and test ids and the scenarios it has, and writes new ones. **The AI is local by default** (the largest text model RavenRS apps already downloaded); `delegate: "codex"` or `"claude"` only when the user asks. The Studio runs each suggestion on the app and says which passed; `write` keeps them in `tests/e2e/sugestoes/` for a person to review. Never move a suggestion into `tests/e2e/` without the user's ok.

An app declares itself for the RTS in its `Cargo.toml`, `[package.metadata.ravenrs.e2e]`: `app` (the name its scenarios open), `binary`, `since` (first version with the channel), `variable`, `installed`. The RTS keeps no list of apps of its own.

## The Studio's own tests

The RTS is tested by the RTS: `tools/raven-test-studio/tests/e2e/*.raventest` test its window (health `#health`, `#run`, `#next`, `#stop`, `#run-fresh`, `#summary`, `#log-N`, `#cache-note`, `#delegate`, `#delegation`, `#delegate-hint`, the blocks `#node:<id>`) on another Studio opened apart with `--e2e-serve`. Run them with `raven-test-studio --project tools/raven-test-studio --run-all` (or `scan_project` with `action: "run_all"`). When the Studio's window changes, change these files in the same PR and run them.
