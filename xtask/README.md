# xtask

`cargo xtask <task>` runs the project's checks and builds, one task per purpose (#1034). It is
Rust with the standard library only, so it runs the same on macOS, Linux and Windows and compiles
in seconds. How to run and read the checks is in the skill `.claude/skills/ravenrs-check`.

| Task | When |
|---|---|
| `cargo xtask dev` | while working, before a commit: formatting, the cheap checks, clippy and the tests of the packages changed |
| `cargo xtask pr` | the pull request's gate, on the developer's machine |
| `cargo xtask release` | before a release: the whole workspace and everything else |
| `cargo xtask web <name>` | builds an example or a tool for the browser |

## The cheap checks

`cheap` in `src/checks.rs` runs first in every check and covers the whole workspace in seconds:
rustfmt, the shell check guard, module cycles, the widgets' layers, the test suites, the
playground's lists, the tools, the feature center, RCB, RTS, fixed texts and the apps' scenarios.

### The shell check does not come back (#1068)

`src/guard.rs` makes sure the old shell check does not come back. It fails when `scripts/check.sh` or `scripts/build-web.sh` exists again, when `scripts/` has a `*.sh` that is not in `KNOWN_SCRIPTS`, or when a file listed by `git ls-files` (tracked, or new and not ignored) cites either script. Each problem is printed as `file:line` with the task that replaced the script: `cargo xtask dev | pr | release` for the check, `cargo xtask web <name>` for the web build.

History passes: `CHANGELOG.md`, `novidades*.json` and `historico-de-novidades.json`, anywhere.
Other lines pass only when they are in `ALLOWED`, a short list in the code where each entry says
why. Add to it only a line that tells what happened to the scripts, never one that tells someone
to run them.
