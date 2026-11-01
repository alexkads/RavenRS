# Contributing to RavenRS

Thank you for helping make RavenRS the most complete free component library for Rust. Bug reports, component requests, documentation fixes and code are all welcome.

## Ways to help

- **Report a bug** with the bug template: what you did, what you expected, what happened, platform and browser.
- **Request a component** with the component template. Check [docs/PARITY.md](docs/PARITY.md) first; it lists what exists and what is planned.
- **Improve docs and examples.** If something in the guide confused you, it will confuse others.
- **Send a pull request.** For anything larger than a fix, open an issue first so we can agree on the API.

## Building

You need Rust stable 1.88+. For the web build, also `rustup target add wasm32-unknown-unknown` and `cargo install wasm-bindgen-cli --version 0.2.130 --locked`.

```bash
cargo run -p component-gallery --release
```

Before opening a pull request, run every check locally (the project has no hosted CI):

```bash
./scripts/check.sh
```

It runs rustfmt, clippy with warnings as errors for desktop and wasm32, all tests and both web builds. Renderer tests draw on your GPU and skip themselves when no adapter is available.

## Adding a component

A component is done when it:

1. Uses the public API only (`View`/`Component` and `BuildCx`), like every built-in component.
2. Takes colors, radii and spacing from the theme (`Semantic` roles and theme scales), never hard-coded values, and looks right in Raven Dark, Light and Glass.
3. Works with the keyboard: focusable when interactive, Enter/Space to activate, arrows where it makes sense, Escape to close overlays.
4. Has behavior tests in `crates/ravenrs-widgets/tests/` that drive it through `Ui` (click, type, check state).
5. Appears in the gallery (`examples/component-gallery`), is listed in the README, and its row in `docs/PARITY.md` is updated.
6. Has rustdoc on the public type and builder methods.

## Code style

- `cargo fmt` formatting (see `rustfmt.toml`) and no clippy warnings.
- No `unsafe` (the workspace forbids it) and no `unwrap()`/`expect()` outside tests.
- Comments explain why, not what. Keep functions small and names plain.
- Public API: builder methods that take `self` and return `Self`, `Binding<T>` for values a control edits.

## Licensing

By contributing you agree that your contribution is dual-licensed under MIT and Apache-2.0, like the rest of the project, without additional terms.
