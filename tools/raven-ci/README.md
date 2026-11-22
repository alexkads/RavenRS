# Raven CI (RCI)

A CI/CD server of its own for RavenRS and for small build servers, made to do what a small Jenkins
does with a fraction of its memory. Its core holds three things: the queue, the model of jobs and
builds, and the register of plugins. Everything else is a plugin, including the pipeline formats,
the steps, git, the providers, triggers, publishers and agents. A plugin is compiled in or is a
program of any language that speaks JSON-RPC.

This is phase 1 of [#803](https://github.com/alexkads/RavenRS/issues/803): the core and the API
of plugins. The plugins that do real work (shell, git, GitHub, the `.raven-ci.toml` format), the
agents, the panel and the HTTP API come in the next phases.

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
- `crates/raven-ci-core/tests/suite`: the register, external plugins with real Python, the model
  and the queue.
- `crates/raven-ci-plugin-api/tests/suite`: each point through its fake plugin.

The tests of external plugins say they skip when there is no Python 3.
