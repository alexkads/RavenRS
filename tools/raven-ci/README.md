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
| `raven-ci plugins compilar PASTA` | builds a plugin written in Rust with the standard traits (`cargo build --release`, a target of its own removed at the end) and installs it in `plugins/<nome>/`; refuses one that asks for another `raven-ci-plugin-api` |
| `raven-ci converter Jenkinsfile [--saida ARQ] [--substituir]` | the `.raven-ci.toml` of a declarative `Jenkinsfile`, with what was not converted, ignored or converted with a difference in comments at its start, each with its line; exits with 1 when a part was not converted (plugin `format-jenkinsfile`) |
| `raven-ci <comando>` | a command a plugin brings, such as `raven-ci hello Ana` |
| `raven-ci servir [--endereco HOST:PORTA] [--executores N]` | the server: HTTP with sign-in (`/health` open; `/api/ci/jobs`, `build`, `log?after=`, `queue` and the routes of the plugins behind a token), the executors and the triggers of the jobs; it stops on Ctrl+C or `SIGTERM` |
| `echo SENHA \| raven-ci usuario criar NOME [--papel P]` | someone who may sign in; the first is an administrator, then `Executores` (asks for builds) or `Leitores` (reads) |
| `raven-ci --self-test` | a core in memory with a fake plugin for every point, and builds through them |
| `raven-ci rodar JOB [--param NOME=VALOR]... [--esperar]` | asks a server for a build (`POST /api/ci/run`); `--esperar` follows its log and ends with 0 only when it passed |
| `raven-ci status JOB [NUMERO]` / `log JOB NUMERO [--seguir]` / `resumo` | the build (the last one with no number), its log (followed to its end) and every job with its last build and the queue |
| `raven-ci evidencia JOB NUMERO PASTA` | downloads the artifacts of the build and its `log.txt` to `PASTA` |
| `raven-ci aplicar ARQUIVO [--remover] [--servidor URL]` | the configuration as code: jobs, folders and plugins of a `raven-ci.toml`, applied to the folder of data or to the server running (`servir --config ARQUIVO` applies it when it starts) |
| `raven-ci servico instalar\|remover\|estado [--memoria 1G] [--endereco …]` | the server as a service of the system: systemd with `MemoryMax`, launchd, or a task of Windows |
| `raven-ci --versao` | the version |

`--format json` gives the lists and the details as JSON, for agents. The folder of data is
`--dados PASTA`, `RAVEN_CI_HOME` or `~/.raven-ci`. External plugins are found in its `plugins/`
folder and in each `--plugins PASTA`.

### From the terminal, against a server

`rodar`, `status`, `log`, `resumo` and `evidencia` speak to a running server, not to the disk:
`--servidor URL` (or `RAVEN_CI_SERVIDOR`, `http://127.0.0.1:8080` unless said) with a key in
`RAVEN_CI_TOKEN`. A key for a program is made once, signed in:

```bash
TOKEN=$(curl -s -X POST http://127.0.0.1:8080/api/auth/token -H 'Content-Type: application/json' -d '{"name":"ana","password":"…"}' | jq -r .token)
curl -s -X POST http://127.0.0.1:8080/api/auth/keys -H "Authorization: Bearer $TOKEN" -H 'Content-Type: application/json' -d '{"name":"terminal"}'
```

```bash
export RAVEN_CI_TOKEN=…
raven-ci rodar apps/site --param E2E=true --esperar
raven-ci evidencia apps/site 12 ./evidencia --format json
```

Every one takes `--format json`; `log --seguir --format json` gives one line of JSON per line of
the log. A server that does not answer is named, and a missing key says how to put it.

### As a service

`raven-ci servico instalar` writes the file of the system's manager of services and starts the
server with it, with the data of `--dados` and the binary the installer put in place (or this one):

| System | File | What it gives |
|---|---|---|
| Linux | `~/.config/systemd/user/raven-ci.service` (`/etc/systemd/system` with `--sistema`) | `Restart=on-failure`, `MemoryMax=` from `--memoria`, `SIGTERM` to stop; a service of a user runs with the machine after `loginctl enable-linger` |
| macOS | `~/Library/LaunchAgents/dev.ravenrs.raven-ci.plist` (`/Library/LaunchDaemons` with `--sistema`) | starts at login, starts again when it falls (launchd waits 10 s), output in `<dados>/logs`; macOS enforces no memory limit |
| Windows | a task of the Task Scheduler | starts at logon (at boot as SYSTEM with `--sistema`), again a minute after it falls; no memory limit |

`RCI_SERVICO=1 sh install.cmd` (or `set RCI_SERVICO=1` before the `.cmd` on Windows) installs or
restarts the service at the end of the installation, so an update runs the new binary.
`raven-ci servico estado` ends with 0 only while it runs.

### The configuration as code

`raven-ci.toml`, kept in a repository, says what the server has:

```toml
url = "https://ci.empresa.dev"

[plugins.cron]
settings = { tick_ms = 1000 }

[folders]
apps = "Aplicativos"

[[jobs]]
name = "apps/site"
file = "pipelines/site.raven-ci.toml"    # or pipeline = """…"""; format = "toml" unless said
keep_builds = 30
settings = { triggers = [{ id = "cron", every = "1h" }, { id = "api" }], scm = { id = "git", url = "…" } }
```

- `raven-ci aplicar raven-ci.toml --servidor URL` (an administrator's key in `RAVEN_CI_TOKEN`)
  applies it to the server running, which watches the triggers of what changed at once: no
  restart. With no server named, it applies to the folder of data.
- A job that did not change is left as it is; one no more in the file stays, and is said, until
  `--remover`. Credentials and builds are never touched. A field the file does not know is
  refused, naming the job and the field.
- `POST /api/ci/jobs` saves one job (JSON, with its pipeline) and `POST /api/ci/config[?remover=1]`
  a whole configuration: only administrators.

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

## A repository on Azure DevOps

The parameterized pipeline of an Azure DevOps repository is a job that fetches with the plugin
`azure-devops`, takes requests from the API and tells the pull request:

```json
{
  "scm": { "id": "azure-devops", "url": "https://dev.azure.com/org/projeto/_git/repo", "credential": "azure",
           "reference": { "pull_request": { "number": 7, "target": "main" } }, "merge_onto_target": true },
  "status": [{ "id": "azure-devops", "repository": "https://dev.azure.com/org/projeto/_git/repo", "credential": "azure" }],
  "triggers": [{ "id": "api" }]
}
```

- The PAT is a credential: `echo '{"token": "…"}' | raven-ci credencial criar azure --tipo token`
  (scope Code, read and status).
- Azure DevOps has no `refs/pull/N/head`: the plugin asks its API where the pull request comes
  from and fetches that branch, with the checkout of the plugin `git`.
- The status goes to the pull request built (`pullRequests/N/statuses`), else to the commit, with
  `context.genre` `raven-ci` (or `genre` in the entry): what a status policy of a branch asks for.
- Azure DevOps Server, or a test, takes the address of its collection in the configuration of the
  plugin: `{"api": "https://ado.empresa/colecao"}`.

## Build agents

The server can leave its builds to agents: `raven-ci-agent`, a program of about 12 MB at rest that
connects out to the server (HTTP and SSE, nothing open where it runs), takes builds by a lease of
the queue and runs them with only the plugins each one needs. A job asks for one with
`agent = { label = "linux" }`; with no agent connected that has the label, a template brings one up
(`agent-local` on this machine, as another user; `agent-remote` on another, through SSH). An agent
that falls comes back and resumes its build; one that does not leaves it interrupted after the
grace time. Each agent may have limits of cores, memory (a cgroup of its own on Linux, and a watch
of its processes everywhere) and disk: `--nucleos 4 --memoria 8G --disco 50G`. See
[Build agents](docs/agents.md).

```text
raven-ci agentes token linux-1 --rotulos linux
RAVEN_CI_AGENT_TOKEN=rva_… raven-ci-agent --servidor http://ci.local:8080 --nome linux-1 --rotulos linux
```

## The plugins that come with it

| Plugin | Point | What it does |
|---|---|---|
| `format-toml` | pipeline format | reads `.raven-ci.toml`: stages, steps, parameters, `when`, `parallel`, `post`, `timeout`, `retry`, `lock`, `container`, `artifacts`, `junit`; every error with its line |
| `format-jenkinsfile` | pipeline format, command | reads a declarative `Jenkinsfile` straight, or converts it with `raven-ci converter Jenkinsfile [--saida .raven-ci.toml]`; what it cannot convert (Groovy code, `withCredentials`, steps with no counterpart) is listed with its line and refuses the job, and what Raven CI does alone (`checkout scm`, `cleanWs()`) is ignored, saying why. The flow is made by `format-toml` |
| `shell` | step | `sh`: runs a command in the folder of the build with each line in the log, `nice`, a time limit and cancelling |
| `git` | scm | fetches a branch, a commit, or a pull request merged onto its target into the folder of the build; the credential goes as a header, never as an argument |
| `lock` | step | named locks (`lock = "maquina"` in a stage) on the same files `flock` uses, let go when the build ends however it ends, never handed to the programs a step starts |
| `workspace-cleanup` | build hook | refuses a build when the disk of its folder has less free space than `workspace.min_free_gb` (5 GB unless said) |
| `api-trigger` | trigger, route | `POST /api/ci/run` with `{"job", "parameters"}` asks for a build of a job whose `triggers` say `{"id": "api"}`, for someone with the role `Executores` or an administrator |
| `cron` | trigger | `cron` (a schedule in a time zone, or `every = "1h"`) and `poll` (`git ls-remote` of a branch every `5m`, a build when it moved) |
| `webhook-github` | trigger, route | `POST /hooks/github`, public, for the webhook of a repository: a job with `{"id": "github-webhook", "repository": "dono/repo", "secret": "NOME"}` takes only what the secret of the credential `NOME` signed (`X-Hub-Signature-256`, checked before the body is read). With `branch`, a push to it asks for a build; without it, a push or a pull request (`opened`, `synchronize`, `reopened`, `closed`) asks for the scan of a job of many branches. A delivery sent again does nothing; a wrong signature is 401 with a line in the log |
| `credentials` | credential kinds | `token`, `user-password` and `ssh-key`, sealed by name in the core's vault (`raven-ci credencial criar NOME --tipo token`, the fields in JSON on the input); a plugin reads one with `Host::secret`, and every log hides it |
| `github` | branch source, status | a job of many branches over a GitHub repository: the branches chosen and the open pull requests (forks left out), each with the pipeline of its own head, built merged onto its target, with the status sent to its commit |
| `docker` | steps, credential kind | `container` (a stage with `container = { image }`): the script in the image with the build's folder mounted and the agent's user, no file of root left; `docker-build`/`docker-push` with a `registry` credential; `compose` up with the wait for health; every container, network, volume and compose project of the build removed at its end, however it ends. Docker or Podman, through their command |
| `kubernetes` | step, credential kind | `kubectl` with `namespace` and `context` declared: `apply` and `rollout status`, the `kubeconfig` of the vault in a file of a moment; `ImagePullBackOff`, `OOMKilled`, `Pending` with no resource and a 403 told in clear words |
| `azure-devops` | scm, status | fetches a repository of Azure DevOps with the PAT of a credential, a pull request from its source branch merged onto its target, and sends the status to the pull request or the commit |
| `artifacts` | publisher, routes | keeps the files the patterns of `artifacts = [...]` find (`*`, `**`, `?`, nothing out of the folder), says which pattern found nothing, and gives them back: `GET /api/ci/artifacts?job=&number=` and `GET /api/ci/artifact?id=` |
| `junit` | publisher, route | reads the JUnit XML of `junit = [...]` with `ravenrs-junit`, keeps the count and the names of what failed with the build, and gives the trend of a job at `GET /api/ci/tests?job=` |
| `cargo` | step, publisher | `{ cargo = "clippy -p site -- -D warnings" }`: cargo with `--message-format=json-diagnostic-rendered-ansi` where it takes it, each diagnostic rendered in the log and kept with its file and line in the attachment `cargo-diagnostics.json` (kind `cargo`, the counts in its summary); one `CARGO_TARGET_DIR` per job kept between builds, held by a lock on a file; `ravenrs-xcache` as the `RUSTC_WRAPPER` with `xcache = true`. The format adds the publisher by itself |
| `email` | notifier | `{"notify": [{"id": "email", "to": ["ana@example.com"], "when": "falha"}]}` on a job: a message through `ravenrs-mail` with the job, the number, how it ended, how long it took, why, the commit and the panel; `when` is `falha` (the default), `mudanca` or `sempre`. The SMTP server and the credential `user-password` are in the plugin's settings |
| `webhook-out` | notifier | `{"id": "webhook", "url": "https://…", "secret": "NOME"}`: the end of a build posted as JSON, signed with `X-Raven-CI-Signature: sha256=…` when there is a secret, tried again after 1 s, 4 s and 16 s on a 5xx, a 429 or no answer, on a thread of its own |
| `metrics` | routes | `GET /api/ci/metrics` for someone signed in, and `GET /hooks/metrics` with no sign-in when its settings say `open = true`: the queue, the builds kept by status, the last build of each job, the agents, the plugins, the memory of each external plugin and of the server, in the Prometheus format, from `Host::snapshot` |
| `agent-local` | agent launcher | brings `raven-ci-agent` up on the server's machine for a template of agents, as another user when it says one (`sudo -n`, or switched by root), so the server runs with `--executores 0` |
| `agent-remote` | agent launcher | brings `raven-ci-agent` up on another machine through `ssh` (the token on its input, a key of the vault when the template names one); the agent connects back |

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

## The panel

`crates/raven-ci-painel` is the panel, the same code on the desktop and in the browser, made only
of RavenRS components. It runs no code of the server: it reads the API with `RemoteClient` and hears
the topic `ci` of the live channel (`/api/live/ci`: `build`, `line` and `plugins`), so a build's log
and status arrive with no reload. Its pages: Jobs (folders, last build, trend, Disparar), a build
(stages on a `FlowCanvas`, the log in a `LogView`, tests, artifacts, parameters, Repetir and
Cancelar), the queue and the executors on a `Scheduler`, the pipeline with its errors on their
lines, plugins, credentials, people and the audit. A plugin puts a page in the menu or a tab on the
page of a build through its model (`raven_ci_core::panel` says how), and leaves when switched off.

```sh
cargo run -p raven-ci-painel -- --servidor http://127.0.0.1:8080   # on the desktop
scripts/build-web.sh raven-ci                                        # for the browser
raven-ci servir --painel tools/raven-ci/web                          # then open /painel/
```

Pictures of each page are in `docs/painel/`.

## Writing a plugin

[docs/writing-a-plugin.md](docs/writing-a-plugin.md) explains both kinds, the manifest of an
external plugin, its protocol and how to test it. The example is [plugins/hello](plugins/hello),
in Python with only its standard library.

A plugin in Rust with the standard traits can also be compiled and installed by the system, with
no new `raven-ci`: `raven-ci plugins compilar PASTA`. The section "Um plugin Rust compilado pelo
sistema" of that page explains it, and [plugins/exemplo-rust](plugins/exemplo-rust) is the example.

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
- `crates/raven-ci-plugin-format-jenkinsfile/tests`: Jenkinsfiles converted and read by
  `format-toml`, each part not converted on its line, the command, and a converted pipeline run
  for real with the same log as the TOML written by hand.
- `crates/raven-ci-plugin-webhook-github/tests`: signed and unsigned deliveries, a secret of one
  repository naming another, a delivery sent twice, pull requests that move and ones that do not;
  `tests/suite/webhook.rs` does it on `raven-ci servir` with a credential of the vault.
- `crates/raven-ci-plugin-cargo/tests`: a crate the test makes, compiled for real: an error and a warning in the log and in the attachment, the target of a job kept between builds and apart from another job's, a missing cargo and a missing `ravenrs-xcache`.
- `crates/raven-ci-plugin-email/tests` and `crates/raven-ci-plugin-webhook-out/tests`: builds run in the core telling a mailer of the test and an HTTP server of the test on a free port; `falha`, `mudanca` and `sempre`, the signature, the tries again and what a job says wrong.
- `crates/raven-ci-plugin-metrics/tests` and `tests/suite/metrics.rs`: the series of a picture of the server, of a core with builds, and the two routes on `raven-ci servir`.
- `crates/raven-ci-plugin-shell/tests`: the step `sh`, with scripts for `sh` and for `cmd`.
- `crates/raven-ci-plugin-git/tests`: a real repository made by the test, cloned by `file://`.
- `crates/raven-ci-plugin-lock/tests`: two executors, a holder outside, a program left running.
- `crates/raven-ci-plugin-workspace/tests`: a build refused for room on the disk.
- `crates/raven-ci-plugin-github/tests`: a GitHub of the test's own on a free port and a real repository: the scan, the builds merged, the statuses.
- `crates/raven-ci-plugin-docker/tests`: the machine's real Docker or Podman (`rust:1`, an image that does not exist, a cancelled build, compose with health); skipped, saying so, with no engine or with an engine of another machine.
- `crates/raven-ci-plugin-kubernetes/tests`: `kubectl-falso`, a kubectl in Rust that answers what each test says, for the arguments, the kubeconfig and each message of a stuck rollout.
- `crates/raven-ci-plugin-azure-devops/tests`: an Azure DevOps of the test's own and a real repository: the pull request from its source branch, merged, the statuses with the PAT, a PAT refused.
- `crates/raven-ci-plugin-artifacts/tests`: the patterns, a link out of the folder, a build that keeps its files and the routes.
- `crates/raven-ci-plugin-junit/tests`: two builds with reports copied by `sh` or `cmd`, one broken, and the trend.
- `tests/suite/remote.rs`: the commands of the terminal against `raven-ci servir` on a free port.
- `tests/suite/config.rs`: a file applied to a server running, its `cron` job built by the schedule, the file changed and applied again with no restart, `--remover`, and the routes refused to who is not an administrator.
- The service is tested by its files on every system and was run for real on macOS (launchd): it
  came up, answered `/health`, came back after a `kill -9` and left nothing.
- `crates/raven-ci-plugin-api/tests/suite`: each point through its fake plugin.

The tests of external plugins say they skip when there is no Python 3.
