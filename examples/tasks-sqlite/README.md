# tasks-sqlite

`cargo run --manifest-path examples/tasks-sqlite/Cargo.toml`.

## End-to-end tests (Raven Test Studio)

The scenarios of this example are in `tests/e2e/`, as `.raventest` files the Raven Test Studio reads: the same, in memory: a test never opens the file `tasks.sqlite`. The app brings them (`SCENARIOS`), and `tests/scenarios.rs` plays them in the same process on every `cargo test`.

To run them as the Studio does, with the app in a process of its own, no window and no cache:

```bash
raven-test-studio --project examples/tasks-sqlite --run-all --no-cache --format json --out target/e2e/tasks-sqlite
```

The app answers the Studio when opened with `--e2e-serve`; its name on the channel, which the `open` block of a scenario uses, is "Minhas tarefas SQLite" (`[package.metadata.ravenrs.e2e]` in `Cargo.toml`). A step that fails says the step, why, and the screen of the app at that moment; with `--film` the report keeps a film of the failure.

