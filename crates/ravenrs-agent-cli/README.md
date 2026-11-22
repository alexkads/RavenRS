# ravenrs-agent-cli

Flow nodes that call the coding agents installed on the computer: the Codex CLI (`codex exec`) and the Claude CLI (`claude -p`). Each CLI keeps its own login; the app never holds it.

## How a run is kept safe

- **Reading only by default.** Codex runs with `--sandbox read-only`, Claude with `--permission-mode plan`. A node writes files only when `read_only` is turned off; `code_review` always reads only.
- **Limits checked before running.** What the installed CLI can keep is read from its `--help` (`Cli::probe`): a sandbox, a permission mode, lists of tools, a budget, turns. A limit it does not keep is refused (`AgentError::Refused`) and nothing runs, so a limit is never silently ignored. Codex keeps no list of tools nor budget, so asking for them is refused too.
- **Only what is needed reaches the CLI.** It runs through `ravenrs-process` with the basic environment only, the prompt on its input, a time limit, and a stop that reaches what it started.
- **Secrets stay out of the record.** The values of variables whose names say they are secrets (`TOKEN`, `KEY`, `SECRET`, `PASSWORD`…) are taken out of every event kept.
- **Cost unknown is unknown.** Claude says what a run cost (`cost_usd`); Codex does not, and its cost stays `None`, never zero.

## The nodes

`with_agents(catalog, codex, claude, review)` adds them:

| Node | What it does | Parameters |
|---|---|---|
| `codex` | Runs the Codex CLI with the prompt. | `prompt` (text with expressions), `dir`, `read_only`, `timeout_ms`, `session`, `model` |
| `claude` | Runs the Claude CLI with the prompt. | the same, and `allowed_tools`, `denied_tools`, `max_turns`, `budget_usd` |

`model` names the model; `cheapest` asks for the agent's cheapest, which `cheapest_model` finds: for Codex, the first model its installed CLI lists and calls affordable (from `models_cache.json` in `CODEX_HOME`, `~/.codex` by default), so a new generation needs no new build; for Claude, `haiku`. A CLI whose `--help` has no `--model` refuses a model before running.
| `code_review` | Asks the agent to review what the prompt gives, pointing each problem with file and line, changing nothing. | `prompt`, `dir` |

Each item that reaches a node is one run; the item that leaves carries the `final_message`, the `events` (started, messages, tools, errors, finished), the `session` to resume and the `cost_usd`. A CLI that is not installed, too old, or refused is a failure of the node that says why; a failure while reading only may be tried again, and one that may have changed files is reported as uncertain.

## Tested

`tests/agents.rs` runs `fake/main.rs`, a CLI of its own that answers `--version` and `--help` and writes recorded JSON lines as each agent does, spending no subscription.
