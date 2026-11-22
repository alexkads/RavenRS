# Calculator

A simple calculator in RavenRS, with buttons and the keyboard, on the desktop and in the browser.

```bash
cargo run -p calculator
```

```bash
./scripts/build-web.sh calculator && python3 -m http.server -d examples/calculator/web 8080
```

## Comparing performance

It is also a small screen that stays the same, so its numbers can be compared between versions of RavenRS. The scene `calculator` in `crates/ravenrs-perf` measures this very view: moving the pointer between buttons, clicking one, standing idle and resizing the window.

```bash
cargo run --release -p ravenrs-perf -- report calculator
```

```bash
cargo run --release -p ravenrs-perf -- time calculator
```

On a 14" MacBook Pro with M2 Pro, at 1280x800 at 2x, a click costs 0.16 ms on the CPU and 0.13 ms recording the GPU's work, drawing 41 instances in 41 draw calls.

The gate in `scripts/check.sh` holds its exact counters against `perf/baseline.json`, so a change that makes this screen do more work fails there.

## End-to-end tests (Raven Test Studio)

The scenarios of this example are in `tests/e2e/`, as `.raventest` files the Raven Test Studio reads: 7 + 8 = 15, e Clear volta a zero; o visor é achado pelo id `display`. The app brings them (`SCENARIOS`), and `tests/scenarios.rs` plays them in the same process on every `cargo test`.

To run them as the Studio does, with the app in a process of its own, no window and no cache:

```bash
raven-test-studio --project examples/calculator --run-all --no-cache --format json --out target/e2e/calculator
```

The app answers the Studio when opened with `--e2e-serve`; its name on the channel, which the `open` block of a scenario uses, is "Calculator" (`[package.metadata.ravenrs.e2e]` in `Cargo.toml`). A step that fails says the step, why, and the screen of the app at that moment; with `--film` the report keeps a film of the failure.

