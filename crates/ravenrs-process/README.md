# ravenrs-process

Runs a child program for RavenRS apps and watches it: the one executor the IDE (Raven Code), the payment helper (TEF) and the agent nodes of Raven Flows share.

## What it does

| Piece | What it does |
|---|---|
| `Spec` | The program and its arguments, passed as they are (no shell reads them), the folder, the environment, input and a time limit. |
| Environment | Only `BASIC_ENV` (`PATH`, `HOME`, the language, the temporary folder…) and what `allow_env` lets through; `env` sets one; `inherit_env` gives everything, as a terminal would. |
| `Running::events` | Each line of its output (`Event::Out`) and of its errors (`Event::Err`) as it comes, in order on each stream, then `Event::Ended`. A full stream never blocks the other; a line longer than `line_limit` is cut. |
| `Control` | From any thread: `send` writes a line to its input, `close` closes it, `stop` stops it. |
| Stopping | The child leads a process group of its own on Linux and macOS: stopping it sends `TERM` to the whole group, and `KILL` after `GRACE`. On Windows it runs with no console window and `taskkill /T` stops the tree. A time limit stops it the same way. |
| `End` | How it ended: its code, a signal, the time limit or a stop. |
| `resolve`, `version` | Where a program is in `PATH` (with `PATHEXT` on Windows), and the first line it says with `--version`, within a time limit. |

## Example

```rust,no_run
use std::time::Duration;
use ravenrs_process::{Event, Spec};

let running = Spec::new("codex").args(["exec", "--json", "-"]).stdin().timeout(Duration::from_secs(600)).spawn()?;
running.control().send("Summarize the changes");
running.control().close();
for event in running.events() {
    if let Event::Out(line) = event {
        println!("{line}");
    }
}
# Ok::<(), ravenrs_process::ProcessError>(())
```

## Tested

`tests/process.rs` runs `fake/main.rs`, a program of its own built as `ravenrs-process-fake`, the same on every system: forty thousand lines on two streams, a grandchild stopped with its parent at the time limit, a variable left out, input lines, a stop, a long line cut, an exit code and a version within its time. Windows runs the same Rust; its `taskkill` is only tested by hand.
