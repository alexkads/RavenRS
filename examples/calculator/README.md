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
