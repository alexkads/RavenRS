# Contributing to RavenRS

Thank you for helping make RavenRS the most complete free component library for Rust. Bug reports, component requests, documentation fixes and code are all welcome.

## Ways to help

- **Report a bug** with the bug template: what you did, what you expected, what happened, platform and browser.
- **Request a component** with the component template. Check [docs/PARITY.md](docs/PARITY.md) first; it lists what exists and what is planned.
- **Improve docs and examples.** If something in the guide confused you, it will confuse others.
- **Start with an issue.** Every change, even a fix, begins as an issue that says what and why. It is where we agree on the approach before the code.
- **Send a pull request that closes it.** One branch per issue; the description follows the template (what changes and why, how it was tested, limitations) and says `Closes #N`. Explain every new or updated dependency. What is left undone becomes a new issue.
- **The release notes cite the issues.** Each Raven Finanças version lists the issues it closes and who contributed. The whole workflow is in [`.claude/skills/ravenrs-github/SKILL.md`](.claude/skills/ravenrs-github/SKILL.md).

## Building

You need Rust stable 1.88+. For the web build, also `rustup target add wasm32-unknown-unknown` and `cargo install wasm-bindgen-cli --version 0.2.130 --locked`.

```bash
cargo run -p component-gallery --release
```

Each worktree builds the ~780 external dependencies once. A compiler cache shares them between checkouts, so a new branch starts from the workspace crates only:

```bash
brew install sccache   # or: cargo install sccache --locked
```

then, in `~/.cargo/config.toml` (your machine, not the repository):

```toml
[build]
rustc-wrapper = "sccache"
```

The checks are tasks of `cargo xtask` (the `xtask/` crate), one per purpose, each taking no arguments. They are Rust, so they run the same on macOS, Linux and Windows. The project has no hosted CI.

```bash
cargo xtask dev
```

While you work, before a commit: rustfmt, the cheap checks of the whole workspace, then clippy with warnings as errors and the tests of the packages whose files changed against `origin/master`, and nothing else.

```bash
cargo xtask pr
```

The pull request's gate: clippy on every package your change reaches (those whose files changed, and every package that depends on them), the tests of the packages that changed and of those that depend on them directly, and the design system and the public API when the change reaches them. `Cargo.toml` and `Cargo.lock` are read entry by entry: a new crate, a version bump or an updated dependency reaches only the packages that use it, while profiles, lints, the edition, `.cargo/`, the toolchain and `clippy.toml` reach the whole workspace.

```bash
cargo xtask release
```

Before a release: clippy and every test on the whole workspace, wasm32, optional features, the minimum Rust version, dependency checks, API docs and the web builds. Renderer tests draw on your GPU and skip themselves when no adapter is available.

The tests run on [cargo-nextest](https://nexte.st), which runs every test binary at once; install it once with `cargo install cargo-nextest --locked`, or the check stops and says so. A compiler cache, the repository's own `ravenrs-xcache`, stands in front of rustc, so a new worktree restores what another one compiled instead of building it again; `RAVENRS_XCACHE=0` turns it off. Each worktree needs its own build directory: worktrees that share a `CARGO_TARGET_DIR` reuse each other's builds of the local crates, and the check warns when it sees that.

At the end, passing or failing, each check prints how long each step took, slowest first, and whether the build directory started empty; the test step is split into compiling and running, followed by the ten slowest tests. The table is saved to `<target>/check-timings/last.txt`, so two runs compare with `diff`. To see which crates take the compile time, run it with `RAVENRS_CHECK_TIMINGS=1`: Cargo then writes its per-crate report, and the check prints where.

## Adding a component

A component is done when it:

1. Uses the public API only (`View`/`Component` and `BuildCx`), like every built-in component.
2. Takes colors, radii and spacing from the theme (`Semantic` roles and theme scales), never hard-coded values, and looks right in Raven Dark, Light and Glass.
3. Works with the keyboard: focusable when interactive, Enter/Space to activate, arrows where it makes sense, Escape to close overlays.
4. Has behavior tests in `crates/ravenrs-widgets/tests/suite/`, declared with `mod` in its `main.rs`, that drive it through `Ui` (click, type, check state). Each package's integration tests are one binary, so they link once; `scripts/check-test-suites.py` fails if a file there is left undeclared.
5. Appears in the gallery (`examples/component-gallery`), is listed in the README and in `site/components/index.html`, has an example in one of the files of `examples/component-gallery/src/samples/`, and its row in `docs/PARITY.md` is updated.
6. Has rustdoc on the public type and builder methods.
7. Is testable by the Raven Test Studio, as the next section says.
8. Runs in the playground: `scripts/gen-live.py` exposes it to the interpreter (the gate fails until it is run), and its example runs from its code, which `coverage_of_the_interpreter` in `examples/component-gallery` checks. The check runs the example interpreted and compiled, and the two have to build as many elements, with no problem later in a paint or a handler. An example the interpreter cannot read yet goes in `examples/component-gallery/live-pending.txt`, with why; today the list is empty.
9. Is current in the design system that is kept in git: `scripts/gen-design-system.py` rewrites `docs/design-system/export/` from RavenComponentBook (its card, preview and measured anatomy), and the gate fails until it is run (#1000).

## Testability (RTS)

Every public type of the feature center has an entry in `tools/raven-test-studio/contrato.json`, which `scripts/rts-coverage.py` checks in `cargo xtask pr` (#734, #736). A component with a screen keeps this contract:

- [ ] every part a test acts on has a name a screen reader reads, or a test id (`BuildCx::test_id`, `ViewExt::test_id`);
- [ ] the items of a collection have test ids made from their key, `part:<key>` (`row:<key>`, `cell:<key>:<column>`), never from their position, so they stay with the data when the list sorts, filters or scrolls;
- [ ] the actions beyond a click it takes are declared with `BuildCx::on_access` (`Expand`, `Collapse`, `Select`, `SetValue`, `ShowMenu`, `Dismiss`), and listed in its entry;
- [ ] while it loads or waits for an answer it says so with `BuildCx::busy` (or `busy_while` and a `BusySignal` for work away from the `Ui`), so tests wait for it with no `sleep`;
- [ ] a component with a gesture of its own (dragging, joining, resizing) has a driver in the `testing` module of its crate, and, when it helps, a `.raventest` under a `tests/e2e` folder.

A complete entry, for a tree:

```json
{"crate": "ravenrs-widgets-lists", "tipo": "TreeView",
 "rts": {"ids": ["item:<chave>"], "acoes": ["Click", "Expand", "Collapse"], "busy": false,
         "driver": "TreeDriver", "cenario": "tools/raven-feature-center/tests/e2e/arvore.raventest"}}
```

with the code that keeps it:

```rust
cx.test_id(row, &format!("item:{}", node.key));
cx.on_access(row, &[AccessAction::Expand, AccessAction::Collapse], move |action, _| {
    open.set(action == AccessAction::Expand);
    true
});
```

A type with nothing on the screen is `{"sem_tela": true}`; one that does not keep the contract yet is `{"pendente": true, "issue": N}`, with the open issue of its family (#737 to #741).

## End-to-end scenarios of the apps

Every app with a window in `examples/` and `tools/` (an independent workspace too) is tested by the Raven Test Studio as a person uses it, and `scripts/e2e-scenarios.py` checks in `cargo xtask pr` that the Studio finds and runs it (#702). A new app comes with the same five pieces, in the same pull request:

- [ ] `Cargo.toml` says what the Studio opens, and brings the test channel outside the web build:

  ```toml
  [package.metadata.ravenrs.e2e]
  app = "Hello World"        # the name its scenarios open; no other app may use it
  binary = "hello-world"

  [target.'cfg(not(target_arch = "wasm32"))'.dependencies]
  ravenrs-e2e.workspace = true
  ravenrs-e2e-wire = { workspace = true, features = ["serve"] }
  ```

- [ ] `tests/e2e/` keeps `.raventest` scenarios that open the app and check what it did (`check`, `mention`, `expect` for a state), not only click. Find parts by test id or by the text a screen reader reads, never by position; a text that shows twice needs a test id (`display`, `item:<key>`).
- [ ] the app brings them, so a Studio installed anywhere runs them: `pub const SCENARIOS: [&str; 1] = [include_str!("../tests/e2e/abrir.raventest")];` in `lib.rs`.
- [ ] `main.rs` answers the channel before any window, with data in memory or in a folder of its own under the temporary folder, never the person's:

  ```rust
  if std::env::args().skip(1).any(|arg| arg == ravenrs_e2e_wire::SERVE_FLAG) {
      let session = ravenrs_e2e::E2e::new(hello_world::app(), Theme::raven_dark())
          .viewport(Size::new(760.0, 560.0), 1.0);
      let hello = ravenrs_e2e_wire::Hello::of("Hello World").scenarios(hello_world::SCENARIOS);
      ravenrs_e2e_wire::serve_filmed(hello, session, ravenrs::movie::ChannelFilm::default())?;
      return Ok(());
  }
  ```

- [ ] a test plays them on every `cargo test` (`tests/scenarios.rs`, or `tests/suite/scenarios.rs` when the app has a suite): `check_folder` refuses a file that is not embedded, and `Scenario::play` runs each on the same session the channel serves. An app that keeps what it saves on disk plays them in a child process with `Client::spawn` and `play_served`, and the test reads back what was saved (`tools/raven-movie-editor`).

Before the pull request, run the Studio on it with no cache, and say in the pull request what it answered:

```bash
raven-test-studio --project examples/hello-world --run-all --no-cache --format json --out target/e2e/hello-world
```

An app with no window of its own (it writes to the terminal or drives other apps) goes in `NO_SCREEN` of `scripts/e2e-scenarios.py`, with the issue that says what it would need; the script refuses it there once it keeps scenarios.

## Code style

- `cargo fmt` formatting (see `rustfmt.toml`) and no clippy warnings.
- No `unsafe` (the workspace forbids it; the one exception is the single call in `ravenrs-wayland-gestures`) and no `unwrap()`/`expect()` outside tests.
- Comments explain why, not what. Keep functions small and names plain.
- Public API: builder methods that take `self` and return `Self`, `Binding<T>` for values a control edits.

## Licensing

By contributing you agree that your contribution is dual-licensed under MIT and Apache-2.0, like the rest of the project, without additional terms.
