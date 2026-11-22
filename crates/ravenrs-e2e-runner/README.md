# ravenrs-e2e-runner

Runs the end-to-end scenarios of RavenRS apps and says how they went. It is the engine of Raven Test Studio (RTS) and of `cargo ravenrs e2e`, so a scenario gives the same verdict in the Studio's window, in a terminal and on a build server.

Part of phase F5 of [#627](https://github.com/alexkads/RavenRS/issues/627) ([#666](https://github.com/alexkads/RavenRS/issues/666)).

## What it does

A scenario is a `.raventest` file, one block per line (see [`ravenrs-e2e-wire`](../ravenrs-e2e-wire)). The runner:

1. **reads the project** (`project::read`) with `cargo metadata`, without building it. It finds the packages made with RavenRS, which are apps, whether each speaks the test channel (`--e2e-serve`), its `tests/e2e/` scenarios and the screens it builds;
2. **opens the app a scenario names** (`app::open`) in a child process with no window. For a source project it first compiles the app with `cargo build --message-format=json-render-diagnostics` and opens the executable Cargo reports, honoring its target directory. An explicit binary or an installed copy that speaks the channel opens directly;
3. **runs each block** (`session::perform`) as a step on the test channel. A failure comes back with its stable code (`E2E_NOT_FOUND`, `E2E_COVERED`, `E2E_TIMEOUT`, …), its meaning and fix in Portuguese, what came close and the app's screen;
4. **reports** (`report::Report`) as text, JSON, JSON lines (`trace`), JUnit or one HTML page (`html`), and keeps each run in a folder: `relatorio.json`, `relatorio.html`, `trace.jsonl` and, on a failure, `arvore.txt`.

Every tool runs the blocks with the same `driver::Driver`: it opens the app a block names, runs the step, times it, asks for the screen on a failure and, with `filming(FilmMode::OnFailure, folder)`, films each app over the test channel and keeps the film of the one that failed (`films`, `film_note` in the report).

Around it:

- `config` reads an app's `e2e.toml` (`folder`, `format`, `out`, `film`; the planned keys are read and said to do nothing yet; anything else is refused with the key);
- `report::schema()` and `ravenrs_e2e_wire::scenario_schema()` are the JSON Schemas (2020-12) of the report and of a scenario as JSON (`Scenario::to_json`, `Scenario::from_json`);
- `rust::test` turns a scenario into a Rust integration test of its app, which compiles and runs with `cargo test`.

The first block that fails stops the run, and the blocks after it are reported as not run.

The aggregated HTML output uses local file URLs for films, so different scenarios can keep the same film filename and the page can be saved anywhere. A `relatorio.html` saved beside its film uses a relative link, so the folder can be moved together. The `url` crate handles platform file paths and escaping; generating these links makes no network request.

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

An opener returns `Option<Result<Session, Box<session::Failure>>>`: `None` for an unknown app, a session when it opened, and a typed failure otherwise. It preserves the failure code, detail and fix through every report. `build::compile` uses `ravenrs-process` for streaming Cargo's two output streams, the build deadline and ending the process group. Compiler errors are `E2E_APP_BUILD`, with the rendered Rust diagnostic (source file, line and suggestions) in `detail`; a compiled app that does not open its channel is `E2E_APP_START`. Only the last 16 compiler diagnostics and 16 stderr lines are kept. The first build can take minutes (`app::BUILD_TIME`, 15 minutes); after it finishes, the app has `app::READY_TIME` (20 seconds) to open its channel.

An app can add `features = ["e2e-source", "mock-data"]` in `[package.metadata.ravenrs.e2e]`. These are the app's own Cargo features, enabled only for its source test build through `build::compile_with_features`; ordinary and installed binaries keep their build choices. Omit the list for Cargo defaults. Unknown features fail the build; a malformed list is a configuration error before execution. `App::declared_in` uses the existing TOML parser for inline and multiline arrays and shares the same interpretation as Cargo metadata. `Project::check_configs` is the shared execution guard used by both tools; discovery still lists the errors and the selected features.

## What it does not do

- With no window (the default) it does not draw: the app runs with no GPU, so a run proves behaviour, not looks. `window::Window` with `WindowMode::Headed` (`--headed`) opens the app's real window instead; `window::Control` pauses, runs one more block or stops a run, `Driver::presenting` paces it, and `KeepOpen` leaves the window open at the end (`Driver::inspection`). `Config::window_from` reads the command line over `e2e.toml`.
- Project discovery does not build anything. The first Open block builds a source app; installed and explicit binaries are never rebuilt.
- Linux and Windows have not been run yet ([#636](https://github.com/alexkads/RavenRS/issues/636)).
