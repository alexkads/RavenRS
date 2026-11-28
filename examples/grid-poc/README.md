# grid-poc

`cargo run --manifest-path examples/grid-poc/Cargo.toml`.

## End-to-end tests (Raven Test Studio)

The scenarios of this example are in `tests/e2e/`, as `.raventest` files the Raven Test Studio reads: it starts grouped by region, with the 100,000 rows, and in pages it shows the rows. The app is only a binary, so `tests/scenarios.rs` opens it with `--e2e-serve` in a process of its own and plays the scenarios over the channel, as the Studio does. The app brings them (`SCENARIOS`), and `tests/scenarios.rs` plays them over the channel on every `cargo test`.

To run them as the Studio does, with the app in a process of its own, no window and no cache:

```bash
raven-test-studio --project examples/grid-poc --run-all --no-cache --format json --out target/e2e/grid-poc
```

The app answers the Studio when opened with `--e2e-serve`; its name on the channel, which the `open` block of a scenario uses, is "Grade de dados" (`[package.metadata.ravenrs.e2e]` in `Cargo.toml`). A step that fails says the step, why, and the screen of the app at that moment; with `--film` the report keeps a film of the failure.

