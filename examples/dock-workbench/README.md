# dock-workbench

`cargo run -p dock-workbench`.

## End-to-end tests (Raven Test Studio)

The scenarios of this example are in `tests/e2e/`, as `.raventest` files the Raven Test Studio reads: the editor opens with the explorer; the folder src opens in the tree, and the search opens from its tab. The test session opens the editor workbench. The app brings them (`SCENARIOS`), and `tests/scenarios.rs` plays them on every `cargo test`.

To run them as the Studio does, with the app in a process of its own, no window and no cache:

```bash
raven-test-studio --project examples/dock-workbench --run-all --no-cache --format json --out target/e2e/dock-workbench
```

The app answers the Studio when opened with `--e2e-serve`, on data in memory: the files it keeps on disk are never opened. Its name on the channel, which the `open` block of a scenario uses, is "Dock workbench" (`[package.metadata.ravenrs.e2e]` in `Cargo.toml`). A step that fails says the step, why, and the screen of the app at that moment; with `--film` the report keeps a film of the failure.

