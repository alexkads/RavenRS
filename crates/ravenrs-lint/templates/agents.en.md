<!-- ravenrs-lint:start -->
## Code quality: non-negotiable

This project uses RavenRS and is checked by `ravenrs-lint`, rustc and Clippy at their strictest. Work is done only when this command passes, with no errors and no warnings:

```sh
cargo ravenrs-lint --gate
```

It runs `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, the RavenRS rules and `cargo test`. Run it before saying a task is finished. If `cargo ravenrs-lint` is missing, install it with `cargo install ravenrs-lint --locked`.

### Rules

- When a check fails, fix the code as the diagnostic's `help` says. Do not silence it.
- Never add `#[allow(...)]`, lower a level in `Cargo.toml` or `clippy.toml`, or write a `ravenrs-lint: allow(...)` comment to make a check pass. When an exception is truly right, use `#[expect(lint, reason = "...")]` or `// ravenrs-lint: allow(rule) reason`, and say so in your answer.
- Never use `unwrap`, `expect`, `panic!`, `todo!`, `unimplemented!` or indexing that can go out of bounds in the application. Return a `Result`, and show errors to the person in the interface.
- No `println!`, `eprintln!` or `dbg!` left in the code.
- Colors only from `Semantic` roles, text only from `TextStyle` steps. Never a literal `Color` or `font_size`.
- Padding, spacing and gaps are multiples of 4: screens 32, cards 24, stacks 16.
- One primary button per view; the others get `.style(ButtonStyle::Secondary)` or `Ghost`. Captions start with a verb.
- Every `IconButton` has a `.label(...)`.
- `State` lives in a field of the component, never created in `body`. `body` and `build` never read files, sleep or wait on the network.
- Small files and functions: split a file before it reaches 600 lines and a function before 80.
- Before using a RavenRS API, confirm it in the documentation or the sources; do not guess method names. If the framework lacks something, say so instead of building a lookalike.
- Every change to behavior comes with a test that drives it, through `Ui` for the interface.
<!-- ravenrs-lint:end -->
