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
| `raw_color` | `Color::rgb`, `Color::WHITE`, `Color { .. }` in the interface; use `Semantic` roles. Defining a `Theme` or `Palette`, and the pixels of an `ImageData::from_fn`, are left alone |
| `raw_font_size` | `.font_size(..)`; use `TextStyle` steps. A printed report's `Style` is left alone |
| `off_grid_spacing` | Literal `padding`, `spacing`, `gap` and `Insets` that are not multiples of 4 |
| `button_without_verb` | `Button` captions that do not start with a verb, in English or Brazilian Portuguese |
| `icon_button_without_label` | `IconButton` with no `.label(..)` and no `Tooltip` |
| `many_primary_buttons` | More than one primary `Button` in a function; `Button` is primary unless styled |
| `state_in_body` | `State::new` in `body` or `build`, which run on every rebuild |
| `blocking_in_body` | Files, processes, sleeping, blocking network calls or `block_on` in `body` or `build` |
| `oversized_file` | Source files over 600 lines, not counting their `#[cfg(test)]` modules |
| `module_cycle` | Top-level modules that use each other in a loop of more than two; see below |
| `allow_without_reason` | `ravenrs-lint: allow(..)` comments that do not say why |
| `unknown_lint` | `allow(..)` comments naming a rule that does not exist |

Test code (`#[test]`, `#[cfg(test)]`) and the `tests/` directory are not checked.

## Configuring

```toml
[package.metadata.ravenrs-lint]   # or [workspace.metadata.ravenrs-lint]; the package wins
language = "pt-BR"                 # "en" by default
extra-verbs = ["Ok"]               # first words accepted besides the built-in verbs
max-file-lines = 600
max-module-cycle = 2               # the most modules one loop may join; see "Modules that use each other"

[package.metadata.ravenrs-lint.lints]
off_grid_spacing = { level = "warn", reason = "the old reports screen is being redrawn in #42" }
```

A level of `"deny"` needs nothing; `"warn"` and `"allow"` need a `reason`. One place is excused with a comment that says why, on the same line or on its own line right above:

```rust
// ravenrs-lint: allow(raw_color) the church's logo keeps its brand color in every theme
```

`allow-file(..)` excuses a whole file.

## Modules that use each other

`module_cycle` reads the whole crate instead of one file. Each top-level module (`mod grid;` in `lib.rs` or `main.rs`, with the files under it) uses another when its code names it: `use crate::grid::…`, `crate::grid::X` in an expression or a type, `crate::X` re-exported from `grid` at the root, or `super::grid` in a file right under the root. Test code does not count.

Modules in a loop (`a` uses `b`, `b` uses `c`, `c` uses `a`) can only be read, tested and moved to a crate of their own together, so a loop grows into a block nobody can take apart. A pair is common (a screen and its helper) and allowed; a longer loop is an error, reported at the `use` of the module that names the most others in it:

```text
error[ravenrs::module_cycle]: 3 modules use each other in a loop, more than the 2 allowed: a → b → c → a (all of them: a, b, c)
 --> src/a.rs:1:1
```

To untangle one, `cargo ravenrs-lint -p <package> --modules` prints each module's size, how many it uses and how many use it, and every loop. Move what two modules both need into a module below them, or let the lower one take a trait or a closure instead of naming the upper one. An `allow(module_cycle)` comment with a reason on a `use` takes that one edge out.

A crate that already has a long loop holds today's size with `max-module-cycle`, so it cannot grow; once it shrinks, the rule asks for the lower number, so the limit only goes down. `--only module_cycle` checks this rule alone, and `--workspace` checks every package, not only those that depend on RavenRS.

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
