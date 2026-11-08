# ravenrs-lint

Lints for applications built with [RavenRS](https://github.com/alexkads/RavenRS). The compiler and Clippy know Rust; `ravenrs-lint` knows what an interface owes to stay right in every theme, for screen readers and across rebuilds. It reads the syntax tree of your sources and reports like the compiler:

```text
error[ravenrs::state_in_body]: `State::new` inside a rebuild
  --> src/ui.rs:24:21
   |
24 |         let query = State::new(String::new());
   |                     ^^^^^^^^^^
   |
   = help: `body` runs on every rebuild, so this state starts over each time and loses what the user did: keep the `State<T>` in a field of the component and clone it here, or use `cx.use_state` in a custom `View`
   = note: `ravenrs::state_in_body` (state lives in the component, not in `body`) is denied; fix the code rather than silencing it
```

It is strict on purpose. Every rule is an error, an exception needs a written reason, and lowering a rule in the configuration needs one too. Coding agents fix what a diagnostic names, so each `help` says exactly what to write instead.

## Install and set up

```bash
cargo install ravenrs-lint --locked
cargo ravenrs-lint --init --language pt-BR   # or en
cargo ravenrs-lint --gate
```

`--init` prepares the package:

- `[lints]` at their strictest: `clippy::all` and `clippy::pedantic` denied; no `unwrap`, `expect`, `panic!`, `todo!`, indexing that can go out of bounds, string slicing, `println!` or `dbg!`; `#[allow]` replaced by `#[expect(.., reason = "..")]`. See them with `--print-lints`.
- `clippy.toml` with limits on function length and complexity, and the exceptions tests need.
- `tests/ravenrs_lint.rs`, so `cargo test` fails while a rule is broken.
- The same instructions for every coding agent, in `AGENTS.md` (Codex and most others), `CLAUDE.md`, `GEMINI.md` and `.github/copilot-instructions.md`. They live between `ravenrs-lint` markers, and running `--init` again updates only that block.

`--gate` runs `cargo fmt --check`, `cargo clippy --all-targets -D warnings`, these rules and `cargo test`, stopping at the first that fails. It is the one command the instructions tell agents to run before saying they are done.

## Rules

| Rule | Catches |
|---|---|
| `raw_color` | `Color::rgb`, `Color::WHITE`, `Color { .. }` in the interface; use `Semantic` roles. Defining a `Theme` or `Palette` is left alone |
| `raw_font_size` | `.font_size(..)`; use `TextStyle` steps. A printed report's `Style` is left alone |
| `off_grid_spacing` | Literal `padding`, `spacing`, `gap` and `Insets` that are not multiples of 4 |
| `button_without_verb` | `Button` captions that do not start with a verb, in English or Brazilian Portuguese |
| `icon_button_without_label` | `IconButton` with no `.label(..)` and no `Tooltip` |
| `many_primary_buttons` | More than one primary `Button` in a function; `Button` is primary unless styled |
| `state_in_body` | `State::new` in `body` or `build`, which run on every rebuild |
| `blocking_in_body` | Files, processes, sleeping, blocking network calls or `block_on` in `body` or `build` |
| `oversized_file` | Source files over 600 lines |
| `allow_without_reason` | `ravenrs-lint: allow(..)` comments that do not say why |
| `unknown_lint` | `allow(..)` comments naming a rule that does not exist |

Test code (`#[test]`, `#[cfg(test)]`) and the `tests/` directory are not checked.

## Configuring

```toml
[package.metadata.ravenrs-lint]   # or [workspace.metadata.ravenrs-lint]; the package wins
language = "pt-BR"                 # "en" by default
extra-verbs = ["Ok"]               # first words accepted besides the built-in verbs
max-file-lines = 600

[package.metadata.ravenrs-lint.lints]
off_grid_spacing = { level = "warn", reason = "the old reports screen is being redrawn in #42" }
```

A level of `"deny"` needs nothing; `"warn"` and `"allow"` need a `reason`. One place is excused with a comment that says why, on the same line or on its own line right above:

```rust
// ravenrs-lint: allow(raw_color) the church's logo keeps its brand color in every theme
```

`allow-file(..)` excuses a whole file.

## From a test, or as JSON

```rust
#[test]
fn follows_the_ravenrs_rules() {
    ravenrs_lint::assert_clean();
}
```

`cargo ravenrs-lint --message-format json` writes one object per diagnostic, for editors and agents. `--list` lists the rules.

## License

MIT OR Apache-2.0.
