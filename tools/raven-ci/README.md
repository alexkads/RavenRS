# Raven CI (RCI)

A CI/CD server of its own for RavenRS and for small build servers, made to do what a small Jenkins
does with a fraction of its memory. Its core holds three things: the queue, the model of jobs and
builds, and the register of plugins. Everything else is a plugin, including the pipeline formats,
the steps, git, the providers, triggers, publishers and agents. A plugin is compiled in or is a
program of any language that speaks JSON-RPC.

Phase 1 of [#803](https://github.com/alexkads/RavenRS/issues/803) made the core and the API of
plugins. Phase 2 brings the plugins that do real work, starting with the life of a build around its
steps (#861), the format `.raven-ci.toml` (#862), the step `sh` (#863), git (#870), locks (#871)
and room on the disk (#872). GitHub, Azure DevOps, triggers, the HTTP API, the other plugins, the
agents and the panel come next.

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
| `raven-ci servir [--endereco HOST:PORTA] [--executores N]` | the server: HTTP with sign-in (`/health` open; `/api/ci/jobs`, `build`, `log?after=`, `queue` and the routes of the plugins behind a token), the executors and the triggers of the jobs; it stops on Ctrl+C or `SIGTERM` |
| `echo SENHA \| raven-ci usuario criar NOME [--papel P]` | someone who may sign in; the first is an administrator, then `Executores` (asks for builds) or `Leitores` (reads) |
| `raven-ci --self-test` | a core in memory with a fake plugin for every point, and builds through them |
| `raven-ci --versao` | the version |

`--format json` gives the lists and the details as JSON, for agents. The folder of data is
`--dados PASTA`, `RAVEN_CI_HOME` or `~/.raven-ci`. External plugins are found in its `plugins/`
folder and in each `--plugins PASTA`.

## A job of many branches on GitHub

The model job holds the settings; the core keeps one job per head under it (`repo/main`,
`repo/PR-12`), each with the `.raven-ci.toml` of its own head, and asks for a build when a head
moves:

```json
{
  "branches": { "id": "github", "repository": "dono/repo", "credential": "github", "branches": ["main"] },
  "scm": { "id": "git", "url": "https://github.com/dono/repo.git", "credential": "github" },
  "status": [{ "id": "github", "repository": "dono/repo", "credential": "github", "context": "raven-ci" }],
  "triggers": [{ "id": "scan", "every": "5m" }]
}
```

- Pull requests of forks are left out (`"forks": true` takes them), and a pull request is built
  merged onto its target; its status goes to its own commit.
- The job of a head that is gone is removed (after `branches.orphan_days` days, 0 unless said).
- The token is a credential: `echo '{"token": "ghp_…"}' | raven-ci credencial criar github --tipo token`.

## The plugins that come with it

| Plugin | Point | What it does |
|---|---|---|
| `format-toml` | pipeline format | reads `.raven-ci.toml`: stages, steps, parameters, `when`, `parallel`, `post`, `timeout`, `retry`, `lock`, `container`, `artifacts`, `junit`; every error with its line |
| `shell` | step | `sh`: runs a command in the folder of the build with each line in the log, `nice`, a time limit and cancelling |
| `git` | scm | fetches a branch, a commit, or a pull request merged onto its target into the folder of the build; the credential goes as a header, never as an argument |
| `lock` | step | named locks (`lock = "maquina"` in a stage) on the same files `flock` uses, let go when the build ends however it ends, never handed to the programs a step starts |
| `workspace-cleanup` | build hook | refuses a build when the disk of its folder has less free space than `workspace.min_free_gb` (5 GB unless said) |
| `api-trigger` | trigger, route | `POST /api/ci/run` with `{"job", "parameters"}` asks for a build of a job whose `triggers` say `{"id": "api"}`, for someone with the role `Executores` or an administrator |
| `cron` | trigger | `cron` (a schedule in a time zone, or `every = "1h"`) and `poll` (`git ls-remote` of a branch every `5m`, a build when it moved) |
| `credentials` | credential kinds | `token`, `user-password` and `ssh-key`, sealed by name in the core's vault (`raven-ci credencial criar NOME --tipo token`, the fields in JSON on the input); a plugin reads one with `Host::secret`, and every log hides it |
| `github` | branch source, status | a job of many branches over a GitHub repository: the branches chosen and the open pull requests (forks left out), each with the pipeline of its own head, built merged onto its target, with the status sent to its commit |
| `docker` | steps, credential kind | `container` (a stage with `container = { image }`): the script in the image with the build's folder mounted and the agent's user, no file of root left; `docker-build`/`docker-push` with a `registry` credential; `compose` up with the wait for health; every container, network, volume and compose project of the build removed at its end, however it ends. Docker or Podman, through their command |
| `kubernetes` | step, credential kind | `kubectl` with `namespace` and `context` declared: `apply` and `rollout status`, the `kubeconfig` of the vault in a file of a moment; `ImagePullBackOff`, `OOMKilled`, `Pending` with no resource and a 403 told in clear words |

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
  the queue and the life of a build (`lifecycle.rs`), the API (`api.rs`).
- `crates/raven-ci-plugin-format-toml/tests`: what the format reads, its errors, and pipelines run
  for real with `sh`.
- `crates/raven-ci-plugin-shell/tests`: the step `sh`, with scripts for `sh` and for `cmd`.
- `crates/raven-ci-plugin-git/tests`: a real repository made by the test, cloned by `file://`.
- `crates/raven-ci-plugin-lock/tests`: two executors, a holder outside, a program left running.
- `crates/raven-ci-plugin-workspace/tests`: a build refused for room on the disk.
- `crates/raven-ci-plugin-github/tests`: a GitHub of the test's own on a free port and a real repository: the scan, the builds merged, the statuses.
- `crates/raven-ci-plugin-docker/tests`: the machine's real Docker or Podman (`rust:1`, an image that does not exist, a cancelled build, compose with health); skipped, saying so, with no engine or with an engine of another machine.
- `crates/raven-ci-plugin-kubernetes/tests`: `kubectl-falso`, a kubectl in Rust that answers what each test says, for the arguments, the kubeconfig and each message of a stuck rollout.
- `crates/raven-ci-plugin-api/tests/suite`: each point through its fake plugin.

The tests of external plugins say they skip when there is no Python 3.
