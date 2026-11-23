# Raven CI for sessions of AI

A session of AI (Claude Code, Codex) uses Raven CI the way a person does: it asks for a build,
waits, finds out why it failed and fixes the cause. What makes it cheap and right is reading
little: the **summary** of a build, which says what failed in a few hundred tokens, instead of the
whole log, which can be thousands. This guide says what Raven CI gives a session and how a
session should use it (#941).

## What there is

| Piece | What it does | Where |
|---|---|---|
| The summary of a build | status, cause, commit, duration; the stage and the step that failed with their last 30 lines, with no noise (`Compiling …`, `test … ok`, codes of color); the tests that failed with their message; the diagnostics of `cargo` with file and line, each once; what the build kept. Never over 8 KB | `GET /api/ci/build/summary?job=&number=[&format=md]`, `raven-ci resumo JOB [NUMERO] [--format json]` |
| The MCP server | 12 tools over standard input: read, act, check pipelines, take evidence | `raven-ci mcp` |
| The schema of the pipeline | the JSON Schema of `.raven-ci.toml`, made from the same lists of fields the reader takes | `raven-ci esquema`, the tool `esquema_pipeline` |
| Checking a file | the errors of a `.raven-ci.toml` or a `Jenkinsfile` with their lines, and the steps no plugin of Raven CI offers, with no server | `raven-ci validar ARQUIVO [--format json]`, the tool `validar_pipeline` |
| Evidence | the artifacts, `log.txt` and `resumo.md` of a build, in a folder, for a pull request | `raven-ci evidencia JOB NUMERO PASTA`, the tool `evidencia` |
| Exit codes and errors in JSON | a script tells a failed build from a server that did not answer | every remote command |
| The skill | how a session should work, in the plugin `raven-ci` of the repository's marketplace | `plugins/raven-ci` |

## Installing

```bash
echo "$CHAVE" | raven-ci ia instalar --servidor http://macpro-i7-alex:8090 --chave -
claude plugin marketplace add alexkads/RavenRS
claude plugin install raven-ci@ravenrs
```

- `ia instalar` saves the key in `~/.raven-ci/chave-do-servidor` (only its owner reads it) and
  registers `raven-ci mcp` in Claude Code with `RAVEN_CI_SERVIDOR`; the key never goes to the
  configuration of Claude Code. With `--so-mostrar` it only prints the command and the JSON for
  another client.
- The key is one of a program: `POST /api/auth/keys`. A key of the role **Leitores** reads
  everything and acts on nothing: `rodar`, `cancelar` and `repetir` are not listed for it, and
  the server refuses them anyway. A key of **Executores** runs builds.
- `raven-ci ia` prints the whole of this in one page, for a session that has only the command line.

## The tools of `raven-ci mcp`

| Tool | Takes | Gives |
|---|---|---|
| `resumo_geral` | | every job with its last build, and the queue |
| `build` | `job`, `numero` (the last when not said) | the summary |
| `log` | `job`, `numero`, `passo`, `depois`, `limite` (100, at most 400) | the last lines of one step, or those after a line, and `next_after` |
| `fila_e_agentes` | | the queue with why each build waits, and the agents |
| `historico` | `limite` | the last builds of every job |
| `rodar` | `job`, `parametros`, `esperar`, `prazo_s` | the number, or, with `esperar`, the summary when the build ends |
| `cancelar`, `repetir` | `job`, `numero` | what the server did |
| `validar_pipeline` | `texto`, `arquivo` | `{formato, valido, erros: [{linha, mensagem}]}` |
| `converter_jenkinsfile` | `texto` | the TOML and the notes of what did not convert, with the line |
| `esquema_pipeline` | | the JSON Schema |
| `evidencia` | `job`, `numero`, `pasta` | the paths of what it wrote |

A tool that fails answers `{codigo, saida, mensagem}`, with the codes below.

## How a session should work

1. **Run and wait in one call**: `rodar` with `esperar: true`. On the command line:
   `raven-ci rodar JOB --esperar > /dev/null` in the background, then `raven-ci resumo JOB --format json`.
2. **Read the summary.** It almost always says enough to fix the cause.
3. **Open one step** only when it does not: `log` with `passo` (the `failed.step` of the summary)
   and a small `limite`.
4. **Check a pipeline** with `validar_pipeline` before applying it.
5. **Take the evidence** with `evidencia`, and attach `resumo.md` to the pull request.

## What it saves, measured

A build of a crate with three dependencies, 300 tests that pass and 1 that fails, and a warning of
the compiler (`cargo test`, on Raven CI 0.1.6, 23/11/2026):

| Read | Bytes | Lines | Tokens (about bytes ÷ 4) |
|---|---:|---:|---:|
| the whole log (`raven-ci log`) | 14,206 | 366 | ~3,550 |
| the summary in JSON (`resumo --format json`, the tool `build`) | 2,793 | 78 | ~700 |
| the summary in Markdown (`resumo`) | 2,150 | 59 | ~540 |

The tail of the summary left out 332 lines of progress and tests that passed, and kept the test
that broke, its `panicked at src/lib.rs:305:32`, the message and the `left`/`right`. The log grows
with the build (a workspace of hundreds of crates writes hundreds of kilobytes); the summary
stays under 8 KB, about 2,000 tokens at most.

The same build, asked of Claude Code (2.1.330) in a session with only the MCP server `raven-ci`
(`claude -p … --strict-mcp-config`, the tools `rodar`, `build` and `log` allowed): "rode o job
medida, espere e diga por que falhou". It ran the build with `rodar` and `esperar`, read the
summary and answered right in 6 turns: the test `testes::soma_errada`, at `src/lib.rs:305`,
expects 5 where `soma(2, 2)` gives 4, and the unused `sobra` makes the 2 warnings. Most of the
tokens of such a session are its own context (the instructions and skills it loads), which the
cache serves; what the build adds to it is the summary.

## Explaining a failure with local AI

Whoever has no session of AI open (a person in the panel, a script) gets the cause in two or three
sentences from the plugin `explain` (#942), written by a model of `ravenrs-ai` that runs on the
machine. Nothing leaves it: the model reads the summary, whose log is already hidden.

```bash
raven-ci explicar JOB [NUMERO]          # the tool explicar of the MCP does the same
```

- **The model never runs in the server's process.** It takes gigabytes, and the RPM holds the
  server to 256 MB. The plugin hands the summary to the program `raven-ci-explicador`:
  - in the RPM, through `raven-ci-explicador.socket`: each request starts an instance of its own
    (`raven-ci-explicador@.service`), with `MemoryMax=6G`, no network (`PrivateNetwork=yes`) and at
    most 5 minutes, which ends with the answer and gives its memory back;
  - elsewhere (a desktop, a test), as a child of the server, the program `programa`.
- **Once is enough.** The explanation is kept with the build (`explicacao.md`, of kind
  `explicacao`); asking again gives it back without running the model. `GET
  /api/ci/build/explain?job=&number=` reads it; `POST` makes it (Executores and Administradores).
- **Off from the start.** It needs the subpackage and a model:

  ```bash
  sudo dnf install raven-ci-explicador-<versão>.rpm
  sudo -u raven-ci raven-ci-explicador baixar --pasta /var/lib/raven-ci-explicador/modelos
  raven-ci plugins ligar explain
  ```

  Off, its route and its tool say how to switch it on.
- **Settings**: `modelo` (`qwen3-1.7b-q4`; any text model of `ravenrs-ai`), `socket`, `programa`,
  `pasta`, `prazo_s`.

Measured on an Apple M2 Pro with `lfm3-1.7b-q4` (23/11/2026), on the summary of a `cargo test` with
one test broken: 11.2 s, 1.4 GB at the peak, and the answer named the test, `src/lib.rs` at line
305, the values 4 and 5, and what to change. The program has 9.5 MB.

## Exit codes

| Code | Name in JSON | Means |
|---:|---|---|
| 0 | `ok` | done; the build passed |
| 1 | `build_falhou` | the build ended and did not pass (`rodar --esperar`, `resumo JOB`); a file with errors (`validar`) |
| 2 | `uso` | the words of the command are wrong |
| 3 | `sem_servidor` | the server did not answer |
| 4 | `sem_permissao` | no key, a key the server refused, or one without the role |
| 5 | `recusado` | the server refused: a job that is not there, a wrong value |
| 6 | `local` | a file or the input, on this machine |

With `--format json`, a refusal is `{"codigo": "sem_servidor", "saida": 3, "mensagem": "…"}` on the
standard output.

## Where it comes from

- The summary: `crates/raven-ci-core/src/summary.rs`; its route in `crates/raven-ci-core/src/api.rs`.
- The MCP server: `src/mcp.rs`, made as `crates/ravenrs-mcp` is (`rmcp`, standard input).
- The schema: `crates/raven-ci-plugin-format-toml/src/schema.rs`, checked against the examples of
  the guides in `crates/raven-ci-plugin-format-toml/tests/schema.rs`.
- `esquema`, `validar` and `ia`: `src/ia.rs`. The exit codes: `Exit` in `src/saida.rs`.
- The explanation: `crates/raven-ci-plugin-explain` (the plugin, tested with a fake model in
  `tests/explain.rs`) and `crates/raven-ci-explicador` (the program and its prompt); the units of
  systemd in `pacote/`.
- Tested on the binary against `raven-ci servir`: `tests/suite/ia.rs` and `tests/suite/remote.rs`.
