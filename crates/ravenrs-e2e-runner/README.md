# ravenrs-e2e-runner

Runs the end-to-end scenarios of RavenRS apps and says how they went. It is the engine of Raven Test Studio (RTS) and of `cargo ravenrs e2e`, so a scenario gives the same verdict in the Studio's window, in a terminal and on a build server.

Part of phase F5 of [#627](https://github.com/alexkads/RavenRS/issues/627) ([#666](https://github.com/alexkads/RavenRS/issues/666)).

## What it does

A scenario is a `.raventest` file, one block per line (see [`ravenrs-e2e-wire`](../ravenrs-e2e-wire)). The runner:

1. **reads the project** (`project::read`) with `cargo metadata`, without building it. It finds the packages made with RavenRS, which are apps, whether each speaks the test channel (`--e2e-serve`), its `tests/e2e/` scenarios and the screens it builds;
2. **opens the app a scenario names** (`app::open`) in a child process with no window. It uses the app's code with `cargo run` when the project declares it in `[package.metadata.ravenrs.e2e]`, else an installed copy that speaks the channel, else the binary its variable names;
3. **runs each block** (`session::perform`) as a step on the test channel. A failure comes back with its stable code (`E2E_NOT_FOUND`, `E2E_COVERED`, `E2E_TIMEOUT`, …), its meaning and fix in Portuguese, what came close and the app's screen;
4. **reports** (`report::Report`) as text, JSON, JSON lines (`trace`) or JUnit, and keeps each run in a folder: `relatorio.json`, `trace.jsonl` and, on a failure, `arvore.txt`.

The first block that fails stops the run, and the blocks after it are reported as not run.

## Using it

```rust,no_run
use ravenrs_e2e_runner::{app, project, report, run};

let project = project::read(std::path::Path::new(".")).unwrap();
project.learn_apps(); // its apps open from this code
let reports: Vec<_> = project
    .scenario_folders()
    .iter()
    .map(|folder| run::run_files(folder, app::open).unwrap())
    .collect();
report::deliver(&reports, true, "junit", Some(&project.e2e_folder()));
```

A tool that ships apps of its own makes them known with `app::know`, as the Studio does with Raven Finanças. It can also pass its own opener: the Studio's opens the apps compiled into it before any apart.

## What it does not do

- It does not draw: the app runs with no window and no GPU, so a run proves behaviour, not looks.
- It does not build the app beforehand. `cargo run` builds it on the first Open block, which may take minutes the first time (`app::BUILD_TIME`).
- Linux and Windows have not been run yet ([#636](https://github.com/alexkads/RavenRS/issues/636)).
