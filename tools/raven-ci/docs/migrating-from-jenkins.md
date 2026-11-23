# Moving from Jenkins

Raven CI was made to take the place of a small Jenkins: a few jobs, a multibranch job over a
repository, a parameterized pipeline started from outside, the status on each pull request, test
reports and artifacts. It does that in a fraction of the memory (see "Feito com RavenRS" in the
[README](../README.md)). This page says how each piece of Jenkins maps to Raven CI, how to convert
a `Jenkinsfile`, what has no counterpart, and how to switch without a day with no CI.

## What maps to what

| Jenkins | Raven CI |
|---|---|
| `Jenkinsfile` (declarative) | `.raven-ci.toml` ([the pipeline](pipeline.md)); a declarative `Jenkinsfile` is also read as it is, by the plugin `format-jenkinsfile` |
| controller | `raven-ci servir`: the queue, the model, the API and the panel, in one program |
| agent, node, executor | `raven-ci-agent` ([Build agents](agents.md)), or the executors of the server itself (`--executores N`) |
| label of a node | `agent = { label = "linux" }` on the pipeline, `--rotulos linux` on the agent |
| cloud of agents (Docker, Kubernetes) | `agent = { docker = { … } }` and `agent = { kubernetes = { … } }`, one container or pod per build |
| multibranch pipeline | a job with `branches`: one job per branch (and per pull request), each with the pipeline of its own head. `branches = { id = "github" }` knows the pull requests and builds them merged onto their target; `branches = { id = "git" }` works on any git server ([Reading the pipeline from the repository](pipeline.md#reading-the-pipeline-from-the-repository)) |
| "Pipeline script from SCM" | `pipeline_from = "scm"` in the job's settings, with `scm.file = "Jenkinsfile"` (or `.raven-ci.toml`): the file is read from the tip of the branch at every build |
| GitHub Branch Source, status on the commit | the plugins `github` (branches and status) and `git` (fetching) |
| Azure DevOps | the plugin `azure-devops`: fetching with a PAT and the status on the pull request |
| `triggers { cron(…) }`, `pollSCM(…)` | in the job's `triggers`: `{"id": "cron", "schedule": "0 3 * * *", "zone": "America/Sao_Paulo"}` or `{"id": "cron", "every": "1h"}`; `{"id": "poll", "url": "…", "branch": "main", "every": "5m"}`; and, for a job of many branches, `{"id": "scan", "every": "5m"}`. The converter lists the `cron` and `pollSCM` of a `Jenkinsfile` with the line to put on the job |
| GitHub webhook | `{"id": "github-webhook", …}` on the job and the webhook pointed at `/hooks/github` |
| "Build with Parameters", remote API | `POST /api/ci/run`, the panel's Disparar, `raven-ci rodar JOB --param NOME=VALOR` |
| credentials (with their description) | the vault: the page Credenciais of the panel, `POST /api/ci/credentials`, or `raven-ci credencial criar NOME --tipo token\|user-password\|ssh-key\|arquivo [--servidor URL]` |
| `withCredentials([string(…), usernamePassword(…), file(…), sshUserPrivateKey(…)])` | `credentials = { VAR = "nome", USER = "nome:user", ARQUIVO = { file = "nome" } }` on the stage or the step; the converter writes it |
| `junit`, `archiveArtifacts` | `junit = [...]` and `artifacts = [...]` in the pipeline |
| `post { always / success / failure / cleanup }` | `[post]` with `always`, `success`, `failure`; `cleanup` joins `always`, after its steps |
| `lock(…)` (Lockable Resources) | `lock = "nome"` on a stage |
| `timeout`, `retry`, `when { branch }` | the same names on a stage |
| `buildDiscarder(logRotator(numToKeepStr, daysToKeepStr))` | `keep = N` in the pipeline; the days go on the job, `keep_days = N` (the converter says the line) |
| `cleanWs()` | nothing to write: the build's folder is removed at its end |
| e-mail notices, a notice to another service | the notifiers `email` and `webhook-out`; `webhook-out` posts the JSON of Raven CI, signed, so a chat that expects its own format needs a small receiver in between |
| JCasC, Job DSL | `raven-ci.toml`, the configuration as code (`raven-ci aplicar`) |
| `/prometheus` plugin | the plugin `metrics`: `/api/ci/metrics` |
| users and matrix of permissions | three roles: Administradores, Executores (ask for builds), Leitores (read) |

## Converting a `Jenkinsfile`

```bash
raven-ci converter Jenkinsfile                          # the TOML on the screen
raven-ci converter Jenkinsfile --saida .raven-ci.toml   # to a file; --substituir to overwrite
```

Every part of the file ends in one of four ways, written as comments at the top of the TOML, each
with its line in the `Jenkinsfile`:

| Mark | Means | Example |
|---|---|---|
| (converted) | the same thing in the TOML | `stage`, `sh`, `when { branch }`, `parameters`, `post` |
| `aviso` | converted, with a difference worth knowing | the `timeout` of a stage becomes the limit of each of its steps |
| `ignorado` | left out because Raven CI does it alone, or it belongs elsewhere | `checkout scm`, `cleanWs()`, `timestamps()`, `triggers` (they go on the job) |
| `não convertido` | no counterpart: the pipeline would not do the same without it | `script { }`, `bat`, `input`, `${currentBuild.number}` |

The command exits with 0 when everything was converted and 1 when something was not, so a script
can tell. Nothing is guessed: a part not converted is listed, never rewritten into something that
might do something else.

A job may also point at the `Jenkinsfile` of its repository with the format `jenkinsfile`. Then the
same reading happens at every build, and a part not converted refuses the job with the error on
its line: a pipeline never runs with a part missing.

## What has no counterpart, and what to do

| Jenkins | Why there is none | Instead |
|---|---|---|
| `script { }`, scripted pipelines (`node { }`), shared libraries, a `def` function of the file | a pipeline is a description, not a program | move the logic into a script of the repository that a `sh` runs; the converter names each `def` at its line |
| `bat`, `powershell` | there is one kind of command | `sh` runs in `cmd` on Windows |
| `input` | a build never stops waiting for someone | split the pipeline in two jobs, the second started by a person |
| `parallel` running at once | the branches run one after the other today | split the work in two jobs, or keep it in one stage |
| `disableConcurrentBuilds()` | builds of a job are not limited | `lock` on the stages that must not run together |
| interpolation with Groovy expressions | there is no Groovy | the variables of the build: `${BUILD_NUMBER}`, `${BRANCH_NAME}`, each parameter by its name |
| thousands of plugins | the plugins are Raven CI's own | a plugin is short to write, in Rust or in any language ([Writing a plugin](writing-a-plugin.md)) |

## Switching without a day with no CI

1. **Install Raven CI beside Jenkins**, on another port: the RPM listens on `127.0.0.1:8090`, and
   `tools/raven-ci/pacote/docker` runs the same RPM in a container to try it on a machine that
   already has a Jenkins.
2. **Convert each `Jenkinsfile`** and look at every `não convertido` before going on.
3. **Make the credentials** in the vault, with the names the jobs will use.
4. **Write the jobs in a `raven-ci.toml`** and apply it (`raven-ci aplicar`), with a context of
   status different from Jenkins' (`"context": "raven-ci"`), so both statuses show on the pull
   request side by side.
5. **Run both for a week.** For each pull request, compare the result, the test counts and the
   artifacts. A difference is a bug of the conversion or of Raven CI: fix it before going on.
6. **Make the status of Raven CI the required one** in the protection of the branch, then stop the
   jobs of Jenkins, and only then Jenkins.

Keep the Jenkins data until the week after the switch: it is the way back.
