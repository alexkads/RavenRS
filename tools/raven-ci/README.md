# Raven CI (RCI)

A CI/CD server of its own for RavenRS and for small build servers, made to do what a small Jenkins
does with a fraction of its memory. Its core holds three things: the queue, the model of jobs and
builds, and the register of plugins. Everything else is a plugin, including the pipeline formats,
the steps, git, the providers, triggers, publishers and agents. A plugin is compiled in or is a
program of any language that speaks JSON-RPC.

Phase 1 of [#803](https://github.com/alexkads/RavenRS/issues/803) made the core and the API of
plugins. Phase 2 brings the plugins that do real work, starting with the life of a build around its
steps (#861), the format `.raven-ci.toml` (#862) and the step `sh` (#863). Git, GitHub, the other
plugins, the agents, the panel and the HTTP API come next.

## How it works

```
raven-ci (the binary: command line, self-test)
  └─ raven-ci-core      Ci: register of plugins, model, queue
       ├─ raven-ci-plugin-api   CiPlugin, the extension points, Host, permissions
       ├─ ravenrs-app           Module, ModuleManifest, plan_modules, EntityInfo
       ├─ ravenrs-flows         a pipeline is a flow; a step is a NodeKind
       ├─ ravenrs-flows-runtime the queue and the runs, in SQLite
       └─ ravenrs-process       external plugins, one process each
```

- **The register** puts the plugins in order with `ravenrs_app::plan_modules`. A plugin that does
  not fit (a version it does not accept, a dependency missing or switched off, a cycle) stays out
  with the reason in words, and the others start. Switching one off stops the plugins that need it
  first, and switching it back on brings them back. Nothing restarts.
- **A plugin** gets a `Host`: its configuration, checked against what it describes; its own folder
  and SQLite database, with its migrations applied once; secrets; and the log. Each service checks
  the permission the plugin declared, and every answer goes to the audit.
- **A pipeline** is read by a plugin's format into a flow of `ravenrs-flows`, where each step is a
  node whose kind a plugin brings and each node carries its stage. A build is a run of that flow.
- **A build** gets a folder of its own, a log kept as it comes with every secret hidden, and its
  variables, which each step finds through its `BuildScope`. Around its steps the core fetches the
  code, runs the `post` steps and the publishers, sends the status of the commit, tells the
  notifiers and removes the folder.
- **The queue** is `ravenrs-flows-runtime`, the same one the Raven Test Studio uses. It has
  priorities, requests that join an equal build still waiting, cancelling (even a build that is
  running), running a build again, and the retention of each job in builds and days.
- **External plugins** start at their first call and stop when idle. A plugin that takes too long
  or dies fails its call and nothing else.

Everything is in `raven-ci.sqlite` in the folder of data.

## How to use it

```bash
cargo run -p raven-ci -- --plugins tools/raven-ci/plugins plugins listar
```

| Command | What it does |
|---|---|
| `raven-ci plugins listar` | every plugin: version, origin, state (on, off, refused with why) and points |
| `raven-ci plugins info NOME` | one plugin: dependencies, permissions declared and granted, configuration, memory, audit |
| `raven-ci plugins ligar NOME` / `desligar NOME` | switches it, and says what else changed |
| `raven-ci <comando>` | a command a plugin brings, such as `raven-ci hello Ana` |
| `raven-ci --self-test` | a core in memory with a fake plugin for every point, and builds through them |
| `raven-ci --versao` | the version |

`--format json` gives the lists and the details as JSON, for agents. The folder of data is
`--dados PASTA`, `RAVEN_CI_HOME` or `~/.raven-ci`. External plugins are found in its `plugins/`
folder and in each `--plugins PASTA`.

## The plugins that come with it

| Plugin | Point | What it does |
|---|---|---|
| `format-toml` | pipeline format | reads `.raven-ci.toml`: stages, steps, parameters, `when`, `parallel`, `post`, `timeout`, `retry`, `lock`, `container`, `artifacts`, `junit`; every error with its line |
| `shell` | step | `sh`: runs a command in the folder of the build with each line in the log, `nice`, a time limit and cancelling |

```toml
[[parameters]]
name = "E2E"
type = "bool"

[[stages]]
name = "Teste"
steps = ["cargo test", { sh = "./e2e.sh", timeout = "5m" }]

[[stages]]
name = "Ponta a ponta"
when = { parameter = "E2E" }
steps = ["./e2e.sh"]

[post]
failure = ["echo falhou"]
```

The whole format is in the documentation of `crates/raven-ci-plugin-format-toml`.

## Writing a plugin

[docs/writing-a-plugin.md](docs/writing-a-plugin.md) explains both kinds, the manifest of an
external plugin, its protocol and how to test it. The example is [plugins/hello](plugins/hello),
in Python with only its standard library.

## Install it

```bash
curl -fsSL https://raw.githubusercontent.com/alexkads/RavenRS/master/tools/raven-ci/install.cmd -o /tmp/install.cmd && sh /tmp/install.cmd
```

## Tests

- `tools/raven-ci/tests/suite`: the binary as a person and an agent run it.
- `crates/raven-ci-core/tests/suite`: the register, external plugins with real Python, the model,
  the queue and the life of a build (`lifecycle.rs`).
- `crates/raven-ci-plugin-format-toml/tests`: what the format reads, its errors, and pipelines run
  for real with `sh`.
- `crates/raven-ci-plugin-shell/tests`: the step `sh`, with scripts for `sh` and for `cmd`.
- `crates/raven-ci-plugin-api/tests/suite`: each point through its fake plugin.

The tests of external plugins say they skip when there is no Python 3.
