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

Before opening a pull request, run the checks locally (the project has no hosted CI):

```bash
./scripts/check.sh
```

It runs rustfmt, then clippy with warnings as errors on every package your change reaches (those whose files changed against `origin/master`, and every package that depends on them), and the tests of the packages that changed and of those that depend on them directly. `Cargo.toml` and `Cargo.lock` are read entry by entry: a new crate, a version bump or an updated dependency reaches only the packages that use it, while profiles, lints, the edition, `.cargo/`, the toolchain and `clippy.toml` reach the whole workspace.

The tests run on [cargo-nextest](https://nexte.st), which runs every test binary at once; install it once with `cargo install cargo-nextest --locked`, or the script stops and says so. Each worktree needs its own build directory: worktrees that share a `CARGO_TARGET_DIR` reuse each other's builds of the local crates, and the script warns when it sees that.

At the end, passing or failing, the script prints how long each step took, slowest first, and whether the build directory started empty; the test step is split into compiling and running, followed by the ten slowest tests. The table is saved to `<target>/check-timings/last.txt`, so two runs compare with `diff`. To see which crates take the compile time, run it with `RAVENRS_CHECK_TIMINGS=1`: Cargo then writes its per-crate report, and the script prints where.

Before a release, run everything:

```bash
./scripts/check.sh --full
```

That runs clippy and every test on the whole workspace, and adds wasm32, optional features, the minimum Rust version, dependency checks, API docs and the web builds. Renderer tests draw on your GPU and skip themselves when no adapter is available.

## Adding a component

A component is done when it:

1. Uses the public API only (`View`/`Component` and `BuildCx`), like every built-in component.
2. Takes colors, radii and spacing from the theme (`Semantic` roles and theme scales), never hard-coded values, and looks right in Raven Dark, Light and Glass.
3. Works with the keyboard: focusable when interactive, Enter/Space to activate, arrows where it makes sense, Escape to close overlays.
4. Has behavior tests in `crates/ravenrs-widgets/tests/suite/`, declared with `mod` in its `main.rs`, that drive it through `Ui` (click, type, check state). Each package's integration tests are one binary, so they link once; `scripts/check-test-suites.py` fails if a file there is left undeclared.
5. Appears in the gallery (`examples/component-gallery`), is listed in the README and in `site/components/index.html`, has an example in one of the files of `examples/component-gallery/src/samples/`, and its row in `docs/PARITY.md` is updated.
6. Has rustdoc on the public type and builder methods.
7. Runs in the playground: `scripts/gen-live.py` exposes it to the interpreter (the gate fails until it is run), and its example runs from its code, which `coverage_of_the_interpreter` in `examples/component-gallery` checks. The check runs the example interpreted and compiled, and the two have to build as many elements, with no problem later in a paint or a handler. An example the interpreter cannot read yet goes in `examples/component-gallery/live-pending.txt`, with why; today the list is empty.

## Code style

- `cargo fmt` formatting (see `rustfmt.toml`) and no clippy warnings.
- No `unsafe` (the workspace forbids it; the one exception is the single call in `ravenrs-wayland-gestures`) and no `unwrap()`/`expect()` outside tests.
- Comments explain why, not what. Keep functions small and names plain.
- Public API: builder methods that take `self` and return `Self`, `Binding<T>` for values a control edits.

## Licensing

By contributing you agree that your contribution is dual-licensed under MIT and Apache-2.0, like the rest of the project, without additional terms.
