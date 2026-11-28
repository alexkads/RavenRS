# site-home

`cargo run --manifest-path examples/site-home/Cargo.toml`.

## End-to-end tests (Raven Test Studio)

The scenarios of this example are in `tests/e2e/`, as `.raventest` files the Raven Test Studio reads: the counter of the page starts at zero, and a click counts one; the counter is found by its id, `clicks`. The app brings them (`SCENARIOS`), and `tests/scenarios.rs` plays them on every `cargo test`.

To run them as the Studio does, with the app in a process of its own, no window and no cache:

```bash
raven-test-studio --project examples/site-home --run-all --no-cache --format json --out target/e2e/site-home
```

The app answers the Studio when opened with `--e2e-serve`; its name on the channel, which the `open` block of a scenario uses, is "RavenRS Home" (`[package.metadata.ravenrs.e2e]` in `Cargo.toml`). A step that fails says the step, why, and the screen of the app at that moment; with `--film` the report keeps a film of the failure.

