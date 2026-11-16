---
name: raven-test-studio
description: How to test a RavenRS screen from the outside with Raven Test Studio, the end-to-end test tool of RavenRS, and how to read its report - the scenarios, the run_scenario tool, the CLI, the JSON report with stable E2E_* codes, the cache and the self-test. Use whenever you change a RavenRS screen or component and need to prove it still works with real clicks and typing, when asked to run, read or fix an end-to-end test, a scenario, the self-test or "o autoteste do Studio", or when a report shows an E2E_ code, even if the request does not name the Studio.
---

# Raven Test Studio

The Studio runs a scenario, a flow of blocks (open an app, click, type, check, mention), against a RavenRS app with `ravenrs-e2e`, and says how each block went. Its report is made to be acted on: each failure has a stable code, what it means, the detail, how to fix it, the parts that came close and the screen of the app at that moment.

The whole list of what it offers is in `tools/raven-test-studio/tool.json`, the same one the RavenRS MCP server answers from (`ravenrs_tools` with name `raven-test-studio`).

## Running a scenario

Prefer the tool when it is there: `run_scenario` with `scenario` (`autoteste`, `lancamento` or `quebrado`) returns the JSON report and shows it in the pane beside the chat.

Without the pane, the CLI does the same:

```bash
raven-test-studio --run autoteste --format json
```

From a RavenRS checkout with no install:

```bash
cargo run -q -p raven-test-studio -- --run autoteste --format json
```

The exit code is 0 when it passed, 1 when it failed and 2 when there is no such scenario.

## Handing it to Codex

When the user asks to delegate the test, or to save your own context on a long diagnosis, hand the scenario to Codex: `run_scenario` with `delegate: "codex"`, or `raven-test-studio --delegate codex --run <scenario>`. Codex runs it with its cheapest model (read from the list the installed Codex keeps; `model` names another), reading only, and answers with the verdict, the cause and the fix with file and line. The Studio then runs the same scenario itself: `delegation.confirmed` says whether it agrees. Report Codex's diagnosis as Codex's, and trust it only when it is confirmed; when it is not, the Studio's own report is the truth. `delegate: "claude"` uses the Claude CLI with `haiku`.

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
