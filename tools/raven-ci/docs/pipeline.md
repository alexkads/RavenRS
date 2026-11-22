# The pipeline: `.raven-ci.toml`

A pipeline says what a build does: its stages, the steps of each stage, the parameters a person
or a program gives it, the conditions of each stage, what runs at the end, and what the build
keeps. It lives in a file `.raven-ci.toml` in the repository, so it changes with the code it
builds; a job of many branches reads the file of each branch from that branch.

The format is read by the plugin `format-toml`. It is TOML with no programming: a pipeline is a
description, so the server can show its stages before running it, refuse what is wrong before
anything starts, and run it the same way on every machine. What needs a program goes in a
script that a step runs. A declarative `Jenkinsfile` is read by `format-jenkinsfile`, which
turns it into this format first: see [Moving from Jenkins](migrating-from-jenkins.md).

## A whole pipeline

```toml
# Keys at the top come before the first table, or TOML puts them inside it.
artifacts = ["target/release/site", "target/*.tar.gz"]
junit = ["**/junit.xml"]

[pipeline]
timeout = "30m"
keep = 10
agent = { label = "linux" }

[environment]
CARGO_TERM_COLOR = "always"

[[parameters]]
name = "E2E"
type = "bool"
default = false
description = "Roda os testes de ponta a ponta"

[[parameters]]
name = "ALVO"
type = "choice"
choices = ["web", "desktop"]

[[stages]]
name = "Build"
steps = [
  "cargo build --release",
  { cargo = "clippy -- -D warnings", name = "Clippy" },
]

[[stages]]
name = "Testes"
parallel = [
  { name = "unidade", steps = ["cargo test"] },
  { name = "web", steps = ["npm test"] },
]

[[stages]]
name = "Ponta a ponta"
when = { parameter = "E2E" }
timeout = "20m"
retry = 1
lock = "navegador"
container = { image = "mcr.microsoft.com/playwright:v1.50.0", env = { CI = "1" } }
steps = ["./e2e.sh ${ALVO}"]

[[stages]]
name = "Publica"
when = { branch = ["main", "release/*"] }
steps = [{ uses = "docker/docker-push", with = { image = "ghcr.io/dono/site:${BUILD_NUMBER}", credential = "ghcr" } }]

[post]
always = ["./limpa.sh"]
failure = ["./avisa.sh"]
```

## How a build runs it

1. **Before any step**, the core makes the build's folder, fetches the code (the `scm` of the job)
   and asks each hook (`workspace-cleanup` refuses a build with no room on the disk).
2. **The stages run in order**, each step in order. A step that fails stops the build, unless it
   is tried again (`retry`). A stage whose `when` does not hold has its steps switched off: they
   are shown as skipped and the build goes on.
3. **`parallel`** branches leave from the step before and meet after the stage. Today they run one
   after the other: the flow has the branches, and the executor runs one node at a time.
4. **At the end**, however it ended: the `post` steps that apply, the publishers (`artifacts`,
   `junit`, the diagnostics of `cargo`), the status of the commit, the notices, and the folder is
   removed.

## `[pipeline]`

| Field | Value | What it does |
|---|---|---|
| `timeout` | a period: `30s`, `10m`, `1h30m`, `500ms` | the whole build; when it passes, the build is cancelled, saying why |
| `keep` | a number | how many builds of the job are kept; the older ones go, with their logs and attachments |
| `agent` | `"any"`, `{ label = "linux" }`, `{ docker = { … } }`, `{ kubernetes = { … } }` | where the build runs. `any` (the default) is any executor of the server or agent. A label waits for an agent with it, and a template of agents brings one up when none is connected ([Build agents](agents.md)). `docker` and `kubernetes` bring an agent up in a container or a pod for this build |

## `[environment]`

Variables of every step, as text. They join the ones the core gives each build:

| Variable | Value |
|---|---|
| `JOB_NAME` | the job, by its path: `apps/site/main` |
| `BUILD_NUMBER` | the number of the build in the job |
| `BRANCH_NAME` | the last part of the job's name: `main`, `PR-12` |
| `WORKSPACE` | the build's folder |
| `GIT_COMMIT` | the commit built, when code was fetched |
| each parameter | its value, under its own name: `E2E`, `ALVO` |

A secret never goes here: it goes in the vault (`raven-ci credencial criar`), and the plugin that
needs it reads it by name. Every log hides each secret read.

## `[[parameters]]`

| Field | Value |
|---|---|
| `name` | the name, which is also the variable the steps read |
| `type` | `bool`, `text` (the default) or `choice` |
| `default` | the value when none is given |
| `choices` | for `choice`, the values a person picks from |
| `description` | what the panel says beside it |

A build asked through the API, the panel or `raven-ci rodar JOB --param NOME=VALOR` gets the
values; a value of the wrong kind is refused before the build starts.

## `[[stages]]`

| Field | Value | What it does |
|---|---|---|
| `name` | text, one per stage | what the panel and the log show |
| `steps` | a list of steps | what the stage does, in order |
| `parallel` | a list of `{ name, steps }` | branches instead of `steps` (one or the other) |
| `when` | `{ branch = …, pull_request = …, parameter = … }` | when it runs; every condition given must hold |
| `timeout` | a period | the limit of **each** step of the stage |
| `retry` | a number | how many more times each step is tried when it fails |
| `lock` | a name | one build at a time holds it, across jobs and with scripts outside Raven CI (plugin `lock`) |
| `nice` | a number | each `sh` runs under `nice -n N` (Linux and macOS) |
| `container` | `{ image, args, env }` | each `sh` of the stage runs in the image, with the build's folder mounted (plugin `docker`) |

### `when`

| Condition | Holds when |
|---|---|
| `branch = "main"` or `["main", "release/*"]` | the head of the job is one of them; a pattern ending in `*` takes what starts with it |
| `pull_request = true` | the job is of a pull request (`PR-12`); `false` holds for a branch |
| `parameter = "E2E"` | the parameter is true, decided as the build runs |

`branch` and `pull_request` are decided when the pipeline is read, by the name of the job.
`parameter` is decided as the build runs, so a stage skipped by it is shown as skipped.

## Steps

| Form | What it is | Plugin |
|---|---|---|
| `"cargo test"` | a command, run by `sh -e -c` (by `cmd /C` on Windows) | `shell` |
| `{ sh = "./e2e.sh", name = "Ponta a ponta", timeout = "5m", retry = 2, nice = 10 }` | the same, with a name, a limit, tries again and a priority | `shell` |
| `{ cargo = "clippy -p site -- -D warnings" }` | `cargo` with each diagnostic in the log and in an attachment | `cargo` |
| `{ uses = "plugin/passo", with = { … } }` | a step of any plugin, by its name and kind | the plugin named |

A command runs in the build's folder, with the build's variables and the few basic ones of the
machine (`PATH`, `HOME`), nothing else of the server's environment. Each line it writes goes to
the log as it comes. It ends well with code 0.

The steps of the plugins that come with Raven CI:

| `uses` | `with` | What it does |
|---|---|---|
| `docker/docker-build` | `tag` (required, one or a list), `context`, `file`, `build_args`, `pull`, `push`, `credential` | builds an image, logged in with a credential `registry` that never touches the machine's configuration; `push = true` sends it too |
| `docker/docker-push` | `image` (required, one or a list), `credential` | sends images already built |
| `docker/compose` | `file`, `services`, `action` (`up` unless said), `wait` (`true` unless said), `pull` | brings the services of the tests up and waits for each to be healthy; the `container` steps after it reach them by name (`db:5432`) |
| `kubernetes/kubectl` | `namespace`, `context` (both required), `apply`, `rollout`, `args`, `credential` | applies manifests and waits for the rollout, saying why a stuck one is stuck |

Whatever a plugin of Docker or Kubernetes starts in a build is removed at its end, however it
ends.

## `[post]`

| Key | Runs when |
|---|---|
| `always` | at the end of every build |
| `success` | when every step passed |
| `failure` | when a step failed |

They are lists of steps, as in a stage. A post step that fails is said in the log and does not
change how the build ended.

## `artifacts` and `junit`

They go at the top of the file, before `[pipeline]`.

- **`artifacts`**: the files the patterns find (`*`, `**`, `?`), kept with the build and given back
  by `GET /api/ci/artifacts?job=&number=` and by `raven-ci evidencia`. A pattern that finds
  nothing is said in the log; nothing outside the build's folder is taken.
- **`junit`**: the JUnit XML files of the tests (Maven Surefire, pytest, nextest, the end-to-end
  runner of RavenRS). The build keeps the counts and the names of what failed; the job keeps the
  trend.

A pipeline with a step `cargo` keeps the diagnostics of the compiler too, as the attachment
`cargo-diagnostics.json`: nothing to say for that.

## When the file is wrong

The whole file is read before anything runs, and every error is said at once, each with its line:

```text
linha 12: um estágio não tem o campo step; os campos são: name, steps, parallel, when, timeout, retry, lock, nice, container
linha 18: timeout: não entendi a duração "dez minutos"; use 30s, 10m, 1h30m ou 500ms
linha 25: já há um estágio chamado Build
```

A build of a pipeline that names a step whose plugin is switched off is refused with
`falta o plugin X`, before any step runs. The editor of the pipeline in the panel shows the same
errors on their lines as you type.

## Where it comes from

- The reader: `crates/raven-ci-plugin-format-toml/src/read.rs`.
- The steps: the crates `raven-ci-plugin-shell`, `-cargo`, `-docker`, `-kubernetes` and `-lock`.
- The examples run for real: `crates/raven-ci-plugin-format-toml/tests/toml.rs`.
