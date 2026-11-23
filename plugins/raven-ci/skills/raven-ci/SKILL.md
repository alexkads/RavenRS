---
name: raven-ci
description: Runs builds on Raven CI and finds out why they failed without reading whole logs, writes and checks `.raven-ci.toml` pipelines, and takes the evidence of a build to a pull request. Use whenever the user mentions Raven CI, `raven-ci`, a `.raven-ci.toml`, a build or a job of their CI server, or asks to run the tests "no CI", "no Raven CI" or "na macpro", or to attach the evidence of a build.
---

# Raven CI for coding agents

Raven CI is a CI/CD server: jobs, builds and their logs, agents, a vault of credentials. This
skill keeps a session cheap: it reads the **summary** of a build (a few hundred tokens), never the
whole log (thousands), unless the summary is not enough.

## The tools

With the MCP server `raven-ci` connected (`raven-ci ia instalar`), use its tools. Without it, the
command line does the same, with `--format json`.

| To | MCP tool | Command line |
|---|---|---|
| see every job and the queue | `resumo_geral` | `raven-ci resumo` |
| run a job and wait for it | `rodar` with `esperar: true` | `raven-ci rodar JOB --esperar > /dev/null; raven-ci resumo JOB --format json` |
| know why a build failed | `build` | `raven-ci resumo JOB [NUMERO] --format json` |
| read one step only | `log` with `passo` (the `failed.step` of the summary) | `raven-ci log JOB NUMERO` |
| check a pipeline before applying it | `validar_pipeline`, `esquema_pipeline` | `raven-ci validar .raven-ci.toml --format json`, `raven-ci esquema` |
| convert a Jenkinsfile | `converter_jenkinsfile` | `raven-ci converter Jenkinsfile` |
| take the evidence to a pull request | `evidencia` | `raven-ci evidencia JOB NUMERO PASTA` |
| cancel, run again | `cancelar`, `repetir` | the panel |

## How to work

1. **Run and wait in one call**: `rodar` with `esperar: true` answers the summary when the build
   ends. On the command line, run `rodar --esperar` in the background, send its output to
   `/dev/null`, and read the summary when it ends.
2. **Read the summary first.** It says the stage and the step that failed with their last 30
   lines (progress and tests that passed are left out), the tests that failed with their
   message, the diagnostics of `cargo` with file and line, and what the build kept. That is
   almost always enough to fix the cause.
3. **Open the log of one step** only when the summary is not enough: `log` with `passo` and a
   `limite`. Never ask for the whole log of a long build.
4. **Before applying a pipeline**, check it with `validar_pipeline`: each error comes with its
   line. `esquema_pipeline` says every field.
5. **Evidence**: `evidencia` downloads the artifacts, `log.txt` and `resumo.md` to a folder; attach
   `resumo.md` and what matters to the pull request.

## The exit codes of the command line

`0` done (the build passed), `1` the build failed, `2` wrong words, `3` no server, `4` no
permission (no key, or a key of a reader for an action), `5` the server refused (a job that is not
there), `6` an error on this machine. With `--format json` an error is
`{"codigo", "saida", "mensagem"}`.

## Access

The server is `RAVEN_CI_SERVIDOR`; the key is `RAVEN_CI_TOKEN`, or the one saved by
`echo CHAVE | raven-ci ia instalar --chave -` (readable only by its owner). A key of the role
Leitores reads everything and runs nothing: the tools that act are not listed. Never print a key,
and never put one in a file of the repository.

## Do not

- Do not read whole logs to find an error: read the summary.
- Do not wait in a loop of `status`: use `rodar` with `esperar`.
- Do not apply a pipeline without `validar_pipeline`.
- Do not look for secrets in a log: the server hides them as `****`.

More: `tools/raven-ci/docs/ai.md` in the repository.
