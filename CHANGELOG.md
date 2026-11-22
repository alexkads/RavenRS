# Changelog

All notable changes to RavenRS are listed here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/). From 26.2.0 on, versions are `YY.R.P`: breaking changes only in each year's `.1` release, additions in `.2`, fixes in maintenance releases, always valid Cargo SemVer. See [docs/VERSIONING.md](docs/VERSIONING.md).

## [Unreleased]

### Breaking

- `ravenrs-flows-runtime`: `Request` has `priority` and `key`, `RunRecord` has `priority`, `key` and `joined`, and `Retention` has `flow_id`; field-by-field literals need them (`Request::new` and `..Retention::default()` keep working). ([#839](https://github.com/alexkads/RavenRS/issues/839))
- The function that makes the `ApiService` of `ravenrs_app_server::router`, `router_with`, `serve` and `serve_with` is now `Fn() + Send + Sync`, not `FnOnce`, since each thread that answers calls it once. A closure that only builds the service from what it captures, as all of them in this repository, does not change (#807).

- Four examples are tools now, with their own version, installer, news, icons, `--versao` and `--self-test`: **Raven Code (RCD)** in `tools/raven-code`, **Raven Movie Editor (RME)** in `tools/raven-movie-editor` (package `raven-movie-editor`), **Raven Report Designer (RRD)** in `tools/raven-report-designer` (package `raven-report-designer`) and **Raven Channel Video (RCV)** in `tools/raven-channel-video` (package `raven-channel-video`). A tool with no window says `"window": false` in its `tool.json` (#798).

- The feature center is the **Raven Feature Center (RFC)**, named as the other tools are (RTS, RCB): `tools/raven-feature-center`, package `raven-feature-center` (`cargo run -p raven-feature-center`), window "Raven Feature Center" (#796).

- The feature center moved from `examples/feature-center` to `tools/feature-center` and is a tool like the others: version 0.1.0 of its own, `install.cmd`, news, icons, `--versao`, `--self-test` and the tray. Paths that named the old folder change; `scripts/build-web.sh` finds a project in `tools/` too (#793).

- `ravenrs-e2e-wire`: `Command` has `Release`, and `Hello` a `window` field; exhaustive matches and field-by-field `Hello` literals need updating (`Hello::of` sets it). `ravenrs-e2e-runner`: `Report` has `window`, `slow_mo_ms` and `stopped`, `RunOptions` has `window` and `control` (`..RunOptions::default()` keeps no window), `Report::not_verified` returns a `Vec`, and `config::PLANNED` no longer lists `window`, now read by `e2e.toml`. ([#729](https://github.com/alexkads/RavenRS/issues/729))
- With `e2e-source`, the internal `Element` literal has a `view_source` field for the instrumented constructor's caller; `Element::new` initializes it. Default builds do not have this field. ([#698](https://github.com/alexkads/RavenRS/issues/698))
- `ravenrs-e2e-runner::app::scenarios` (also reexported by the Studio) returns `Box<session::Failure>` instead of `String` on failure, preserving the same typed diagnostics as the app opener. ([#692](https://github.com/alexkads/RavenRS/issues/692))
- `ravenrs-e2e-runner::app::App` has a `features` field for source test builds; field-by-field literals need it (`..App::default()` keeps Cargo defaults). ([#687](https://github.com/alexkads/RavenRS/issues/687))
- `ravenrs-e2e-wire`: `Command` has `Source { target }`, and `Reply` has an optional `source` field; exhaustive matches and field-by-field reply literals need updating. ([#685](https://github.com/alexkads/RavenRS/issues/685))
- `ravenrs-e2e-wire`: `Command` has `Frame`, and `Reply` has an optional `frame` field. Exhaustive matches and field-by-field reply literals need updating. Existing `Filming` implementations keep compiling through the default frame method. ([#683](https://github.com/alexkads/RavenRS/issues/683))
- `ravenrs-e2e-runner`: `session::Opener`, `app::open` and `app::open_from` (also reexported by the Studio) return `Box<session::Failure>` instead of `String`, preserving error codes and diagnostics from opening an app. A custom opener can return `Box::new(Failure::plain(message))` for a plain error. ([#681](https://github.com/alexkads/RavenRS/issues/681))
- `ravenrs-movie`: `TestFilm` is gone; `SessionFilm` with `Pace::Test` does what it did (`SessionFilm::start(&mut e2e, Pace::Test)`, `finish(&mut e2e, duration_ms)`). The `Recorder` now drives a `ravenrs-e2e` session and films with `SessionFilm` at a person's pace; its API is the same, and `Recorder::e2e_mut` reaches the session. ([#672](https://github.com/alexkads/RavenRS/issues/672))
- `ravenrs-e2e-wire`: `Command` has `Film` and `FilmStop`; an exhaustive `match` needs the new arms. ([#670](https://github.com/alexkads/RavenRS/issues/670))
- `ravenrs-e2e-wire`: `Command` has `Drag` and `Kind` has `Drag`; an exhaustive `match` on either needs the new arm. ([#663](https://github.com/alexkads/RavenRS/issues/663))

- `ravenrs-e2e-wire`: `Command` has `Fill`, `Select`, `Expand`, `Collapse`, `Scroll` and `Wait`, and `Kind` the same kinds; an exhaustive `match` on either needs the new arms. `Command::apply` now tries a step again for up to `PATIENCE` (2 s) while its target is not ready, and scrolls to a target out of sight first. ([#658](https://github.com/alexkads/RavenRS/issues/658))

- `ravenrs-agent-cli`: `Policy` has a new public field, `model`, and `Caps` one, `model`. Literals with `..Policy::default()` or `..Caps::default()` do not change. ([#634](https://github.com/alexkads/RavenRS/issues/634))
- `Labels` has `flow_open_search`, `flow_open_none` and `flow_open_close`. Literals with `..Default::default()` or `..Labels::pt_br()` do not change. ([#527](https://github.com/alexkads/RavenRS/issues/527))
- `Labels` has `inspector_drag`, `inspector_copy` and `param_no_field`. Literals with `..Default::default()` or `..Labels::pt_br()` do not change. ([#532](https://github.com/alexkads/RavenRS/issues/532))
- `Labels` has `expression_result` and `expression_failed`, the preview of `ExpressionEditor`. Literals with `..Default::default()` or `..Labels::pt_br()` do not change. ([#533](https://github.com/alexkads/RavenRS/issues/533))
- `Labels` has the words of `DataInspector` (`inspector_*`). Literals with `..Default::default()` or `..Labels::pt_br()` do not change. ([#531](https://github.com/alexkads/RavenRS/issues/531))
- `Labels` has the words of `ParameterForm` (`param_fixed`, `param_expression`, `param_mode`, `param_apply`, `param_discard`, `param_add`, `param_remove`, `param_loading`, `param_load_failed`, `param_retry`, `param_required`, `param_none`). Literals with `..Default::default()` or `..Labels::pt_br()` do not change. ([#530](https://github.com/alexkads/RavenRS/issues/530))
- `Labels` has `flow_skipped`, `flow_cancelled`, `flow_pinned`, `flow_item_one` and `flow_items`, the words a `FlowCanvas` says for a run told from outside. Literals with `..Default::default()` or `..Labels::pt_br()` do not change. ([#528](https://github.com/alexkads/RavenRS/issues/528))
- `FlowNode` and `FlowGraph` have a new public field, `data` (`serde_json::Value`), the app's own data the diagram carries without reading. Literals with `..Default::default()`, `..FlowNode::new(…)` or `..graph.clone()` do not change; a `FlowGraph { nodes, edges }` written field by field needs `data: serde_json::Value::Null`. `FlowGraph::to_json` writes a `format` field (version 2), and diagrams written before still read. ([#529](https://github.com/alexkads/RavenRS/issues/529))
- `Labels` has `flow_run_choose_many`, `flow_run_follow` and `flow_run_none_chosen`, and `ravenrs_flow_bpmn::elements::ELEMENTS` has fifteen elements (participants added). Literals with `..Default::default()` do not change.
- `Labels` has `code_find`, `code_replace`, `code_replace_all`, `code_find_previous`, `code_find_next`, `code_match_case`, `code_whole_word`, `code_find_count`, `code_no_results`, `code_show_replace`, `code_close_find` and `code_suggestions`, for the find bar and the suggestions of a `CodeEditor`; literals with `..Default::default()` do not change. ([#500](https://github.com/alexkads/RavenRS/issues/500), [#497](https://github.com/alexkads/RavenRS/issues/497))
- `Labels` has the fields of `FlowToolbox`, `FlowLibrary`, `FlowLayers`, the check and the debugger of `FlowCanvas`, its export, bend points and ports (`flow_toolbox*`, `flow_library*`, `flow_layer*`, `flow_check*`, `flow_debug`, `flow_run_*`, `flow_export*`, `flow_bend`, `flow_field_fill`, `flow_field_stroke*`, `flow_field_text`, `flow_field_dashed`, `flow_running`, `flow_done`), and `flow_shape_names` has ten names. Literals with `..Default::default()` do not change.
- `FlowNode` has two new public fields, `style` (`FlowStyle`) and `hidden`; `FlowEdge` has four, `waypoints`, `color`, `dashed` and `kind`. Literals with `..Default::default()`, `..FlowNode::new(…)` or `..FlowEdge::new(…)` do not change, and diagrams written before read as they were.
- `ravenrs-app-server`: an operation of `/api/batch` whose `body` is not a record (a list, a text, a number) is refused with 400; it used to be taken as an empty body, and a POST wrote a record of nothing but its defaults. No `body`, or `null`, is still empty. ([#508](https://github.com/alexkads/RavenRS/issues/508))
- `Labels` has `dock_switcher`, `dock_documents`, `dock_tools`, `dock_where_floating` and `dock_where_hidden`; literals with `..Default::default()` do not change. ([#490](https://github.com/alexkads/RavenRS/issues/490))
- `Labels` has `dock_documents_center`, `dock_tools_docks` and `dock_close_others`; literals with `..Default::default()` do not change. ([#399](https://github.com/alexkads/RavenRS/issues/399))
- `DockLayout::to_text` writes version 4 of the format, which keeps the floating panels; builds before refuse it, and texts of versions 1 to 3 still read. `Labels` has `dock_float_item`, `dock_floating`, `dock_dock_back`, `dock_move_floating` and `dock_resize_floating`; literals with `..Default::default()` do not change. ([#399](https://github.com/alexkads/RavenRS/issues/399))
- `DockLayout::to_text` writes version 3 of the format, which keeps the panels hidden at the edge of a dock; builds before refuse it, and texts of versions 1 and 2 still read. `Labels` has `dock_auto_hide`, `dock_auto_hide_item`, `dock_pin`, `dock_hidden` and `dock_pinned`; literals with `..Default::default()` do not change. ([#399](https://github.com/alexkads/RavenRS/issues/399))
- In a `DockArea`, Alt+Shift+↑ sends the tab with focus to the new top dock; it used to send it to the center, which is now Alt+Shift+Enter. `DockSide` has a new variant, `Top` (it is `#[non_exhaustive]`), `Labels` has `dock_top`, and `DockLayout::to_text` writes version 2 of the format, which older builds refuse; texts of version 1 still read, with an empty top dock. ([#399](https://github.com/alexkads/RavenRS/issues/399))
- `Theme` has a new public field, `based_on`, the built-in theme it starts from, which its tokens are kept against. A literal with `..Theme::raven_light()` (or another theme) inherits it; one without `..` stops compiling.
- `Theme` has a new public field, `charts` (`ChartSeries`), and `WizardOptions` in `ravenrs-report` has `on_accent`, `ink`, `muted` and `zebra`; `Labels` has `theme_charts` and `grid_select`. Literals with `..Default::default()` or `..Theme::raven_dark()` do not change.
- With two or more `GridColumn::fill` columns, what is left is shared among them by weight; before, only the first took it.
- `Labels` has the fields of the `FlowInspector` (`flow_inspector*`, `flow_field_*`, and the lists `flow_status_names`, `flow_shape_names`, `flow_line_names` and `flow_cap_names`). Literals with `..Default::default()` do not change.
- `FlowNode` has a new public field, `shape`. Literals with `..Default::default()` or `..FlowNode::new(…)` do not change.
- `FlowLayout` has a new public field, `kind`. Values made with `FlowLayout::new` or `FlowLayout::default` do not change.
- `FlowNode` has a new public field, `rotation`, and `Labels` has `flow_turned` and `flow_no_turn`. Literals with `..Default::default()` or `..FlowNode::new(…)` do not change.
- `Quad`, `ImageQuad`, `GlyphRun` and `Effect` have a new public field, `rotation`, and `Element` (doc-hidden) too. Literals built with `Quad::new`, `ImageQuad::new` or `..Effect::IDENTITY` do not change; a `GlyphRun` written field by field needs `rotation: 0.0`.
- `FlowEdge` has two new public fields, `start` and `end`, and `Labels` has `flow_title`. Literals with `..Default::default()` or `..FlowEdge::new(…)` do not change; `FlowEdge::default()` ends in an arrow, as every line did.
- `FlowNode` has two new public fields, `parent` and `group`, and `Labels` has `flow_group` and `flow_members`. Literals with `..Default::default()` (or `..FlowNode::new(…)`) do not change.
- `Labels` has `flow_arrange`, `flow_input`, `flow_output` and `flow_pick_port`, for the arrange button and the ports of a `FlowCanvas`. Literals with `..Default::default()` do not change.

### Added

- `.raventest` scenarios check the state of a part with `expect` (`estar` in Portuguese): `checked`, `unchecked`, `disabled`, `enabled`, `focused`, `visible` or `count N`, also read as `marcado`, `desmarcado`, `desabilitado`, `habilitado`, `com-foco`, `visivel` and `quantidade N`. An unknown state is refused when the scenario is read. The block goes over the channel as `Command::Expect`, is in the Studio's palette and in the Rust it writes, and `ravenrs-e2e` gains `to_be_unchecked`. RavenComponentBook's scenario now checks that its `disabled` setting turns on and the story's button turns disabled (#846).
- **Raven CI (RCI)**, phase 1 of [#803](https://github.com/alexkads/RavenRS/issues/803), in `tools/raven-ci`: a CI/CD server whose core holds only the queue, the model of jobs and builds and the register of plugins. `raven-ci-plugin-api` has `CiPlugin` over `ravenrs_app::Module`, with one extension point per function of a CI (steps as `NodeKind`, Scm, BranchSource, Trigger, Publisher, StatusReporter, Notifier, CredentialKind, AgentLauncher, PipelineFormat, Route, CliCommand), permissions declared as capabilities, a `Host` that checks and audits each service, and a fake plugin per point. `raven-ci-core` plans plugins, switches them on and off live, keeps each one's configuration and database, runs external plugins of any language over JSON-RPC by stdio (on demand, with a time limit per call, stopped when idle, never taking the server down; the example `plugins/hello` is in Python), and keeps folders, jobs, numbered builds with stages and steps, attachments and the retention of each job. `raven-ci plugins listar|info|ligar|desligar`, `--format json`, `--versao` and `--self-test`. ([#833](https://github.com/alexkads/RavenRS/issues/833), [#834](https://github.com/alexkads/RavenRS/issues/834), [#835](https://github.com/alexkads/RavenRS/issues/835), [#836](https://github.com/alexkads/RavenRS/issues/836), [#837](https://github.com/alexkads/RavenRS/issues/837), [#838](https://github.com/alexkads/RavenRS/issues/838), [#839](https://github.com/alexkads/RavenRS/issues/839))
- `ravenrs-flows-runtime`: the queue has priorities (`Request::priority`, `Runtime::set_priority`, `Runtime::queue`), requests that join a queued run of the same key (`Request::key`, `RunRecord::joined`), cancelling that reaches a running node (`Runtime::cancel`), retention of one flow (`Retention::flow_id`) and `Runtime::with_catalog`, the same queue with other kinds. Files made before get the new columns when they open. The Raven Test Studio's queue is meant to be this one. ([#839](https://github.com/alexkads/RavenRS/issues/839))
- `ravenrs-flows`: `Executor::cancel_cooperatively`, cancelling by a token with the nodes on the calling thread, as a durable runtime wants. ([#839](https://github.com/alexkads/RavenRS/issues/839))
- `ravenrs-app`: `plan_modules`, with `ModuleEntry`, `ModuleHost`, `ModulePlan` and `ModuleRefusal`: the order of modules and why each one that does not fit is left out, while the others go on. `ApplicationBuilder::build` uses it and refuses at the first reason, with the same words as before. ([#835](https://github.com/alexkads/RavenRS/issues/835))
- `ravenrs-metrics`: `rss_bytes_of(pid)`, the memory of another process, such as a child, on Linux and macOS. ([#836](https://github.com/alexkads/RavenRS/issues/836))
- RavenComponentBook: `--replay` takes any icon of the `Icon` card in an `icon` setting, not only the sample the cards offer; `DropdownMenu` gets the `label` and `icon` settings, `Select` gets `fill`, and the books of `Sidebar` (items with a state and an action) and `FlowCanvas` (cables; nodes with their own content and a selection) get new stories. The workspace screen of the Fermata UX is a replay fixture that RavenRS builds as drawn. ([#831](https://github.com/alexkads/RavenRS/issues/831))

- `check.sh` checks that every app with a window in `examples/` and `tools/` keeps scenarios the Raven Test Studio finds and runs (`scripts/e2e-scenarios.py`): the `[package.metadata.ravenrs.e2e]` declaration, the test channel, `tests/e2e/` scenarios that check something, and a test that plays them. CONTRIBUTING.md describes the pattern; the contracts and Raven Channel Video stay listed as having no window (#702). RavenComponentBook joins with a scenario of its own, and the Studio's own scenarios now play on `cargo test` too.

- The Raven Feature Center, Raven Code, Raven Movie Editor and Raven Report Designer have scenarios of the Raven Test Studio in `tests/e2e/` and answer it with `--e2e-serve` (#702, fifth lot). Raven Code edits and saves on a sample project of its own (`sample_in`, `e2e_session`; the editor of each file is `editor:<path>`), and Raven Movie Editor saves a chapter into a folder of its own, reopened by its test.

- Raven Finanças has four more scenarios of the Raven Test Studio (an income, an entry with no amount, changing months and paying a bill), and its test in a process of its own opens one app per scenario, as the Studio does. `raven-flows` and `mini-pdv` have scenarios too, `raven-flows` in a folder of its own under the temporary folder with runs done at once (#702, fourth lot).

- `flow-chat-bot`, `flow-design-board`, `flow-whiteboard`, `bpmn-editor` and `dock-workbench` have scenarios of the Raven Test Studio in `tests/e2e/`, served with `--e2e-serve` on data in memory (#702, third lot).

- `grid-poc`, `quick-plan`, `spreadsheet`, `site-home` and `component-gallery` have scenarios of the Raven Test Studio in `tests/e2e/` (#702, second lot). `Scenario::play_served` in `ravenrs-e2e-wire` plays a scenario on an app opened with `--e2e-serve` in a process of its own, for an app that is only a binary, as `grid-poc`. The click counter of the home page is `clicks`.
- `LogView` and `LogBuffer` in `ravenrs-widgets-lists`: a build log as it comes, in a monospaced font with the colors the program wrote (ANSI 30-37 and 90-97, bold, reset, mapped to the theme's colors, with every other escape taken out), over a `VirtualList` that follows the end, with a search that marks the lines and counts them, sections that fold by click or Expand/Collapse, and the number of a line as a button that points at it (`on_line`). New labels `log_name`, `log_search`, `log_go_to_end`, `log_matches`, `log_folded` and `log_point_line`. It has its RCB stories, its RTS contract and the page "Log de build" in the feature center (#824).

- `ravenrs-metrics`, metrics in the Prometheus text format with no dependencies: a `Registry` of counters, gauges and histograms with labels, safe from any thread, `render()` for a `/metrics` route with `CONTENT_TYPE`, and `process_rss_bytes()`, the memory the process holds, on Linux. Bad names, names taken and the wrong number of label values are errors, not panics (#820).

- `ravenrs-git`, git for RavenRS services over the git installed: shallow `clone`, `fetch` with refspecs, `fetch_pull_request` (`refs/pull/N/head` as `<remote>/pr/N`), `checkout` with no branch, `rev_parse`, `ls_remote`, and `merge`, which names the files of a conflict and undoes it. A `Credential` goes in the child's environment as `http.extraHeader`, never as an argument nor in the remote's address, and every command runs without the user's git configuration and never asks for a password (#817).

- `ravenrs-secrets`, the vault: secrets sealed with ChaCha20-Poly1305 under a key kept apart from them (`KeyFile`, `KeyFromEnv`, or the system's store with the feature `system-keyring`), an opaque `SecretRef` for documents and exports, separate rights to use (`use_with`), reveal and administer, rotation, revocation, `MemoryStorage` and `FileStorage`, and one `Redactor` that keeps every value handed out of text as `•••`. `ravenrs-agent-cli` masks with the same `redact` (#815, part of #546).

- The live channel: `ravenrs-app-server` streams events as they happen at `GET /api/live/{topic}`, as Server-Sent Events, from a `LiveChannels` the application publishes to from any thread (`open`, `publish`, `ApiOptions::live`). Each topic keeps its last 256 events for whoever comes back with `Last-Event-ID`, and sends `gap` when some are gone. The streams take none of the threads that answer, and end when their token does. `ravenrs-app-remote` listens with `RemoteClient::listen`, on the desktop and in the browser (`LiveFeed`, `SseParser`), coming back from the last event when the stream is lost (#810).

- `hello-world`, `calculator`, `quick-app`, `tutorial-tasks` and `tasks-sqlite` have scenarios of the Raven Test Studio in `tests/e2e/`, served with `--e2e-serve` and played on every `cargo test` by `tests/scenarios.rs` (#702). `Scenario::play` in `ravenrs-e2e-wire` plays a scenario on a session in the same process. The display of the calculator is `display`.

- The application framework is in the testability contract of the RTS (#739): an `ActionButton` is `action:<id>`, whatever its caption says in the language on screen; a `DataGrid` with rows on a server (`DataGrid::remote`, the remote `ListView`) is busy while a page is on its way, and `RemoteRows::is_loading` says so. With this, every type of the feature center keeps the contract or has no screen.

- Imaging and chat are in the testability contract of the RTS (#741): `ColorWheel` takes `SetValue` with a color as `#ff0000`; `TransformHandles` and `CropOverlay` take their box by value (`x, y, width × height, angle`), and `testing::CropDriver` sets and reads it; the messages of a `Chat` are `message:<n>`, and the sphere of a `FloatingPanel` is `orb` and opens and closes with `Expand` and `Collapse`. `RuleOfThirdsGrid` has no screen.

- `testing::SortableDriver` moves an item of a `SortableList` before another by their keys, with Alt and the arrows as a person does (#740). Each point of a chart is a node of its own, `point:<series>:<category>`, named with its value, so a screen reader reads the data one by one; the commands of a `Ribbon` are `command:<label>`.

- Lists, scheduler, dock and charts are in the testability contract of the RTS (#740): document tabs are `tab:<id>`, the tabs of a `Ribbon` `tab:<title>`, the items of a `SortableList` with a key `item:<key>`, the days of a `YearView` and an `EventCalendar` `day:<yyyy-mm-dd>`, calendar events `event:<id>` and timeline items `item:<id>`. `FileBrowser` and `Filmstrip` keep the `row:<key>` of their list.

- The components of `ravenrs-widgets-kit` are in the testability contract of the RTS (#738): tabs are `tab:<label>` and take `Select`; the items and sections of a `Sidebar` are `item:<label>` and `section:<title>` (sections open and close with `Expand` and `Collapse`); breadcrumbs are `crumb:<label>`, wizard steps `step:<n>`, uploaded files `file:<name>` and the remove buttons of a `TagBox` `remove:<tag>`; `Disclosure`, `TagBox` and `LookupField` open and close with `Expand` and `Collapse`; `TextArea` takes `SetValue`. `IconTile` gets a story with a label; `QuadField` has no screen.

- Every type of `ravenrs-widgets-base` is in the testability contract of the RTS (#737, last lot): `Stepper` and `ColorPicker` take `SetValue` (a whole number, a color as `#3B82F6`); `ColorPicker` and `DateRangePicker` open and close with `Expand` and `Collapse`; the jacks of a `PatchPanel` are `jack:<id>`, the dots of a `Carousel` `slide:<n>`, the colors of a `SwatchGrid` `swatch:<name>`, and the points of a `NinePointAnchor` `option:<name>` and take `Select`.
- `ApiOptions::workers(n)` in `ravenrs-app-server`: that many requests answered at the same time, each by a thread with its own service, so one slow request no longer holds up the rest. The threads share who is signed in: signing in on one, signing out, a session running out and a new password count on all of them, and the limit on tries to sign in adds up across them. The default stays one thread (#807).

- 50 more types of `ravenrs-widgets-base` are in the testability contract of the RTS (#737, third lot): the segments of a `SegmentedControl` and the items of a `ToggleGroup` are `option:<label>`, the pages of a `Pagination` are `page:<n>`, and the display and layout components are found by the test id the app gives them, which a test of RavenComponentBook checks on every story of every component the contract lists as keeping it. `Canvas`, `Icon`, `Skeleton`, `AspectRatio` and `CablePlug` have nothing to find, and the contract says why.

- The overlays of `ravenrs-widgets-base` keep the testability contract of the RTS (#737, second lot): `Dialog`, `Sheet`, `CommandPalette`, the menus and the toasts close with `Dismiss`; `DropdownMenu`, `ContextMenu` and the titles of a `Menubar` (`menu:<title>`) open with `ShowMenu`, `Expand` and `Collapse`, and a `Popover` with `Expand` and `Collapse`; menu items are `item:<label>`, commands `option:<label>`, toasts `toast:<id>`, and the close buttons `close`. `BuildCx::on_access` called again on a node adds to what it takes.

- `ravenrs-schedule`, the scheduling engine of the report server, made to be shared with the app framework, Raven Flows and Raven CI, with no runtime of its own: the `cron` schedules move to it unchanged, and it adds `Trigger::cron` (in a time zone) and `Trigger::every`, `check` (one pass: seed, wait, run or record a miss, and the next run to store first), `Misfire::Within` and `Misfire::Always` (however many were missed, at most one runs) and `Running`, the guard that keeps one job from running twice. `ravenrs-report-server` uses it, and `ravenrs_report_server::cron` stays as a re-export (#804).

- The input controls of `ravenrs-widgets-base` keep the testability contract of the RTS (#737, first lot): `TextField`, `Slider`, `Combobox`, `DatePicker` and `InputOtp` take `SetValue`; `Select`, `Combobox` and `DatePicker` open and close with `Expand` and `Collapse`; the options of `Select`, `Combobox` and `RadioGroup` are `option:<value>` and take `Select`; the days of the calendar are `day:<yyyy-mm-dd>` and take `Select`. `ravenrs-e2e` says so when a component refuses a value or an action.

- The testability contract of the Raven Test Studio is checked: `tools/raven-test-studio/contrato.json` has an entry for each type of the feature center (test ids, actions, driver and scenario, `sem_tela`, or pending with its issue), and `scripts/rts-coverage.py`, in `check.sh`, refuses a type with none, a test id numbered by position and an action the code and the contract disagree on. The Studio's run log has the ids `log:N` (#736).

- Accessible actions `SetValue`, `Expand`, `Collapse`, `Select`, `ScrollIntoView`, `ShowMenu` and `Dismiss`, declared with `BuildCx::on_access`, and `AccessNode::busy`, set with `BuildCx::busy` or `busy_while` and a `BusySignal` for work away from the `Ui`. AccessKit gets them as themselves (Expand and Collapse are no longer a click) and the browser gets `aria-busy`. `ravenrs-e2e` has `set_value`, `show_menu`, `dismiss` and `perform`, waits for a busy node before acting, and `.raventest` reads `set-value`, `show-menu` and `dismiss`. `Chat` is busy while it waits for an answer (#735).

- `cargo-ravenrs` says when it is behind: an unknown command gives the installed version and the install command (exit code 2), and `--version` and `doctor` warn when the project asks for another RavenRS (#751).
- RCB: stories for `FormGroup` (inside `FormLayout`) and for tabs without content, sizes and disabled in `Tabs` (#768).

- `FlowCanvas::pan_with_wheel(false)`: the wheel over a diagram goes to what holds it, so a page scrolls past a diagram set in it; dragging and the keys still pan. ([#790](https://github.com/alexkads/RavenRS/issues/790))

- `ravenrs-app-ui`: `Words` has `access`, the words a screen reader hears, in the language of its catalog, and a session starts with them; an `AppShell` or `EntityForm` made with `Words::pt_br()` no longer announces in English ("Showing Contas"). `Words` is `#[non_exhaustive]`, so the new field breaks nothing. ([#768](https://github.com/alexkads/RavenRS/issues/768))
- RavenComponentBook puts the themes of the applications in the design system (Raven Finanças writes its own with `Theme::to_tokens_json` to `examples/family-finance/design/`), gives `EntityForm` its stories, and keeps each story in one language. ([#768](https://github.com/alexkads/RavenRS/issues/768))
- RavenComponentBook 0.1.2 says how each component sizes: every story is laid out in parents of 320, 760 and 1200 pt, and its JSON, its card (*Sizing*) and its preview tell whether it fills its parent or hugs its content, with its limits and the three sizes. Its anatomy also lists the surfaces and texts around the controls: a dialog's panel, titles, labels, navigation, toolbars, menus, lists and tooltips. It also measures, on each component's first control, the pointer over it, the focus brought with Tab and the button held down, and says which tokens change (`interaction` in the JSON, *Interaction* on the card, the states in the preview). Stories are measured in Raven Glass too, and a color between two tokens is named by them (`mix(ink, border, 25)`, a CSS `color-mix` in the preview), so the preview follows the theme. Each card has a *Behavior* section (events, the keys its first control handles once focused, its accessibility role and the focus order), and the card of `Icon` lists every icon with its character in `RavenIcons`. `scripts/rcb-coverage.py` also refuses a component whose stories leave out an option of a choice or a state it has (`disabled`, `read_only`, `loading`, `error`), unless `pendentes.json` lists it with `"conteudo": true` and its issue; nine components got the stories they lacked. The design system also has five patterns under `patterns/` (a form, a page beside its sidebar, a filtered grid, a confirmation and an empty page), measured like the components, and the clips of a preview no longer share their ids. `--replay <screen.rcb.json>` checks a screen that comes back from the design tool: each element marked `data-rcb` (with `data-rcb-args`) is built with its story and settings in a parent as wide as its box and compared with it. `ravenrs_widgets::book` has `Sizing` and `Extent`, and `Meta::sizing` declares what measuring cannot tell; `Meta` has a public `sizing` field. ([#768](https://github.com/alexkads/RavenRS/issues/768))
- RavenComponentBook 0.1.1 writes the whole design system and guards it: tokens read from `ravenrs-style`, `tokens.css`, the Inter and RavenIcons fonts, a cover drawn from the tokens and an index locked to the code (`"editing": "code"`); `--verify` compares a published copy with what it writes. `ravenrs-app-ui` (`AppShell`) and `ravenrs-movie` (`MoviePlayer`, `MovieAssistant`) have the feature `book` with their stories. ([#754](https://github.com/alexkads/RavenRS/issues/754))
- `ravenrs_widgets::book` (feature `book`): every component gives its stories through the trait `ComponentBook`, as a Storybook `*.stories` file: its place in the book, its settings (`ArgType`, each with a control and a default) and its named states (`Story`). 174 of 186 public components implement it; `book::all` lists them. RavenComponentBook (`tools/raven-component-book` 0.1.0) draws them live, measures what RavenRS paints and writes their JSON and the design system the design tool reads; `scripts/rcb-coverage.py`, in `check.sh`, refuses a component without it unless it is listed with its issue. ([#753](https://github.com/alexkads/RavenRS/issues/753), [#754](https://github.com/alexkads/RavenRS/issues/754))
- `ravenrs-app-update` has the feature `ui`: `NewsDialog`, `release_note`/`release_notes`, `version_button` and `with_notice` draw the news of each version and the update's notice the same way in every app, and `Updater::app` gives the app it updates. Raven Finanças 0.1.44, Raven Flows 0.1.1 and Raven Test Studio 0.1.15 use it: in the Studio the version opens its news and the notice's Novidades, Como atualizar and Detalhes open what they say. ([#744](https://github.com/alexkads/RavenRS/issues/744))
- Scenarios run with the app's real window on screen: `--headed` in Raven Test Studio 0.1.14 and `cargo ravenrs e2e run`, with `--slow-mo <ms>`, `--step` (Enter runs one block, `c` carries on, `p` pauses, `q` stops), `--keep-open never|on-failure|always` and `--headless`; `e2e.toml` takes `window`, `slow_mo` and `keep_open`, below the command line. The same `.raventest`, blocks and codes run on the window's own interface: `ravenrs_widgets::UiDriver` drives a window's `Ui` on its thread through `ravenrs::native::run_driven`, `ravenrs_e2e_wire::Headed` answers the test channel there (`--e2e-serve --e2e-window`), and `E2e::for_window`/`E2e::in_window` borrow the window's interface for each step. The app's clock stands still 0.6 s after each step until the next (`SETTLE`), so the pace never changes a verdict. New codes `E2E_NO_WINDOW` and `E2E_WINDOW_CLOSED`; the report says `window`, `slow_mo_ms`, `platform` and `stopped`. Raven Finanças 0.1.43 opens its test window on the sample family in memory. Run on macOS; Linux and Windows pending. ([#729](https://github.com/alexkads/RavenRS/issues/729))
- `cargo ravenrs e2e install [--project <folder>]` equips existing generated apps with the shared native test channel and initial scenario. It validates the plan before writing, preserves custom fields/comments/scenarios, recognizes formatted entries and leaves adequate projects unchanged. Unknown/custom entries are refused for manual integration. Template generation now explicitly enables the wire's `serve` feature. ([#701](https://github.com/alexkads/RavenRS/issues/701))
- Optional constructor origins for untagged `Button::new` and `Text::{new,bound,dynamic}`: `E2e::source` and the existing Source channel return the caller found by role/text queries, including after reactive rebuilds. Explicit test-id origins take priority; default builds and screen-reader trees keep no source metadata. Button and Text now have complete demonstrations and tours in the feature center. ([#698](https://github.com/alexkads/RavenRS/issues/698))
- New apps from all four `cargo ravenrs new` templates (also used by `create-ravenrs`) include a native `--e2e-serve` entry, project metadata, opt-in source instrumentation and an initial `.raventest` scenario. They reuse the frame/film adapter and serve sample data in memory; the SQLite template's test entry and screen test never open its persistent database. ([#695](https://github.com/alexkads/RavenRS/issues/695))
- `cargo ravenrs doctor` reads e2e configuration before assistant integration, including `--no-ai` projects. Missing channels/declarations/scenarios produce warnings; invalid configuration, binaries or scenarios fail. It reuses project discovery without compiling or launching apps, and reports channel detection as a static hint, not a verified handshake. ([#693](https://github.com/alexkads/RavenRS/issues/693))
- Apps can declare Cargo test-build features in `[package.metadata.ravenrs.e2e]`. Both runners preserve them through discovery and pass them to the shared build executor; malformed lists are refused before execution. The Studio declares its optional source instrumentation for project test builds. ([#687](https://github.com/alexkads/RavenRS/issues/687))
- Optional test-source instrumentation: `e2e-source` records the caller of `ViewExt::test_id` and `BuildCx::test_id`, and `E2e::source` / `Client::source` return its file, line and column. The origin follows a container's test id, never enters the screen reader's tree, and is absent in default builds. Untagged targets refuse with `E2E_NO_SOURCE`. ([#685](https://github.com/alexkads/RavenRS/issues/685))
- Vector frames over the test channel: `Client::frame()` receives the current display list and assets as a single-frame `.ravenmovie`, provided by `ChannelFilm` through the existing `SessionFilm` writer. Capture needs no GPU or file, does not advance time and leaves an ongoing film recording. Apps without a frame adapter return `E2E_NO_FRAME`. ([#683](https://github.com/alexkads/RavenRS/issues/683))
- `ravenrs-e2e-runner`: source apps compile before their test channel opens, through `build::compile` and the shared `ravenrs-process` executor. Cargo's JSON selects the executable, honors custom target directories and carries rendered Rust errors with source locations as `E2E_APP_BUILD`; a compiled app without a channel stays `E2E_APP_START`. ([#681](https://github.com/alexkads/RavenRS/issues/681))
- Filmed failures: `--film never|on-failure|always` in `cargo ravenrs e2e run` and Raven Test Studio 0.1.13 (and `film` in `e2e.toml`) films each app an Open block opens, over the test channel, and keeps `filme-N.ravenmovie` beside the run's report; on failure only the film of the app a block failed on stays. The report gains `films` and `film_note` (JSON, schema, text, HTML link, JUnit attachment). One driver runs every step for every tool, `ravenrs_e2e_runner::driver::Driver`, which the Studio's diagram uses too. Raven Finanças 0.1.42 and the Studio are served with filming (`serve_filmed`, `ChannelFilm`). ([#674](https://github.com/alexkads/RavenRS/issues/674))
- One film writer for every purpose: `ravenrs_movie::SessionFilm` films tests (`Pace::Test`), the `Recorder`'s trainings (`Pace::Person`), the `FlightRecorder`'s bug reports (fed by hand with `new`, `at`, `frame`, `pointer`, `click`, `key`, `movie`) and apps over the test channel (`ChannelFilm`). `ravenrs-e2e` gains raw input for scripts (`pointer_to`, `mouse_down`, `mouse_up`, `key_down`, `scroll_at`, `focus`, `frame_at`) and `find_all`. A `FilmBuilder` frame drawn at the moment of the last one replaces it. Raven Finanças's six trainings were recorded again this way. ([#672](https://github.com/alexkads/RavenRS/issues/672))
- Films of end-to-end tests: `ravenrs-e2e`'s `FrameObserver` (`E2e::observe`, `stop_observing`, `mark_step`, `size`, `background`) tells a watcher each frame, pointer move, click, key and step, and costs nothing when no one watches; `ravenrs_movie::TestFilm` makes a `.ravenmovie` from it at a test's pace, a chapter per marked step, the same bytes on every run, with a protected field hidden as the session draws it. Over the test channel, `Command::Film { path }` and `Command::FilmStop` film an app served with `ravenrs_e2e_wire::serve_filmed` and `ravenrs_movie::ChannelFilm`; one served with `serve` answers the new code `E2E_NO_FILM`. The `Recorder` finds `#test-id` as a test does. ([#670](https://github.com/alexkads/RavenRS/issues/670))
- End-to-end reports as one HTML page (`--format html`, and `relatorio.html` beside each kept run), JSON Schemas of the report and of a scenario as JSON (`ravenrs_e2e_runner::report::schema`, `ravenrs_e2e_wire::scenario_schema`, `Scenario::to_json` and `from_json`, `cargo ravenrs e2e schema`), an `e2e.toml` per app (`folder`, `format`, `out`) and `cargo ravenrs e2e rust <file>`, which prints a scenario as a Rust integration test of its app. Raven Test Studio 0.1.11 takes `--format html`. ([#668](https://github.com/alexkads/RavenRS/issues/668))
- `ravenrs-e2e-runner`: the engine that runs `.raventest` scenarios on apps opened apart and reports them (text, JSON, JSON lines, JUnit, kept in a folder), with the reading of a project (`project`), the opening of apps (`app`), the steps and failures in Portuguese by code (`session`) and `run::run_files`, which runs files straight from their blocks. Raven Test Studio now uses it, with no change in what it does. ([#666](https://github.com/alexkads/RavenRS/issues/666))
- `cargo ravenrs e2e run [path] --format text|json|jsonl|junit --out <folder>` runs a project's end-to-end scenarios with no window and no Studio, exit code 0, 1 or 2, each run kept in `target/e2e/`; `cargo ravenrs e2e list` says the project's apps, test channel, scenarios and screens. An Open block that names no app fails with `E2E_APP_START` and says where the name comes from. ([#666](https://github.com/alexkads/RavenRS/issues/666))
- Test drivers for the `Scheduler` and the `DockArea`, feature `testing`: `SchedulerDriver` (`move_event`, `resize_event`, `move_to_resource`, by the arrow keys, one step of the snap each) and `DockDriver` (`float`, `dock_back`, by Ctrl+Alt+F or Cmd+Alt+F), in `ravenrs_widgets::testing` too. The `Scheduler` gives each bar the test id `event:<id>` and each resource's row `resource:<id>`; the `DockArea` gives each tab `panel:<id>`, a floating panel `floating:<id>` and its bar `floating-move:<id>`. ([#665](https://github.com/alexkads/RavenRS/issues/665))
- Test drivers, feature `testing`: `DataGridDriver` (`edit_cell`, `sort_by`, `clear_sort`, `filter`) in `ravenrs-widgets-grid` and `FlowCanvasDriver` (`connect`, `move_node`) in `ravenrs-widgets-charts`, re-exported as `ravenrs_widgets::testing`. They act on test ids the components now give: a `DataGrid` with `key_column` gives each cell `cell:<key>:<column>`, each header `column:<column>` and each filter field `filter:<column>`, and a `FlowCanvas` gives each port `port:<node>:<port>`. A grid cell that does not edit is read only for assistive technology. ([#663](https://github.com/alexkads/RavenRS/issues/663))
- `ravenrs-e2e`: `Locator::fill` on a grid cell opens its editor with a double click, writes and confirms with Enter, and fails with `E2E_DISABLED` on a cell that does not edit; `Locator::drag_to` looks for where to drop after the drag started, so a part that shows only while dragging can be the target; `E2e::failure` and `E2eError::hint` for drivers. ([#663](https://github.com/alexkads/RavenRS/issues/663))
- Raven Test Studio 0.1.10: the block `drag`/`arrastar` (`drag #from = #to`). ([#663](https://github.com/alexkads/RavenRS/issues/663))
- Raven Test Studio 0.1.9: `--format jsonl` (one event per line, with the time of each step) and `--format junit` (JUnit XML, the failure's code in `type`, the blocks after it skipped); `--out <folder>` keeps `relatorio.json`, `trace.jsonl` and, on a failure, `arvore.txt` with the app's screen, and with `--project` it defaults to the project's `target/e2e`. Each block of the JSON report says how long it took (`duration_ms`), and an unknown `--format` is refused. ([#661](https://github.com/alexkads/RavenRS/issues/661))
- The menu beside the clock: `TrayMenu` and `TrayEntry` in `ravenrs-core` (state, item with detail and shortcut, check, submenu, separator), `SystemTray::menu` and `SystemTray::on_select` in `ravenrs-platform-native`, on macOS and Windows (`tray-icon`) and Linux (`ksni`); the menu redraws when the app sets new entries, and an id that starts with `open:` opens the window first. `ravenrs-app-update` gives the update's entries with `Updater::tray_entries` and acts on them with `Updater::tray_select`. Raven Finanças 0.1.41 shows the overdue bills, Nova despesa and Nova receita; Raven Test Studio 0.1.8 the self-test, Rodar autoteste and the update. ([#643](https://github.com/alexkads/RavenRS/issues/643))

- `ravenrs-e2e`: a session clock (`E2e::advance`, `E2e::clock`), waits (`E2e::wait_until`, `E2e::patiently`, `Locator::wait_for`, `Locator::wait_to_mention`) and the code `E2E_TIMEOUT`; actions `Locator::fill`, `select`, `expand`, `collapse` and `scroll_into_view`, which also finds a row a long list has not built. `VirtualList` with `key` and `DataGrid` with `key_column` give each row the test id `row:<key>`, and `TreeView` each item `item:<key>`. ([#658](https://github.com/alexkads/RavenRS/issues/658))
- `ravenrs-agent-cli`: `Cli::installed`, whether the program is on the machine without running it. ([#637](https://github.com/alexkads/RavenRS/issues/637))
- Raven Test Studio 0.1.7: the blocks fill, select, expand, collapse, scroll and wait; "Delegar ao Codex" in the window, in a thread of its own, with "Parar"; the local AI's suggestions use only targets the app shows, and one that acts on anything else is set aside with the reason. ([#637](https://github.com/alexkads/RavenRS/issues/637), [#658](https://github.com/alexkads/RavenRS/issues/658))

- Raven Test Studio tests itself with `.raventest` scenarios in `tools/raven-test-studio/tests/e2e/` (health, running, stepping, stopping, the cache), run on another Studio in a process of its own: it serves itself on the test channel with `--e2e-serve` and declares itself in `[package.metadata.ravenrs.e2e]`. An app learned from a project comes before one compiled in with the same name. ([#656](https://github.com/alexkads/RavenRS/issues/656))
- Raven Test Studio: `--project <folder>` reads a Rust project without building it and says which packages are made with RavenRS, which are apps, whether each opens on the test channel, its scenarios and the views it builds with file, line and test ids; `--run-all` runs every app's scenarios from the project's code, and `--suggest` has an AI, local by default (Codex or Claude with `--delegate`), write `.raventest` scenarios that the Studio runs on the app (`--write` keeps them in `tests/e2e/sugestoes/`). Apps declare themselves for the Studio in `[package.metadata.ravenrs.e2e]`, and the Studio keeps no list of apps; the Claude pane gets a `scan_project` tool. ([#654](https://github.com/alexkads/RavenRS/issues/654))
- `ravenrs-e2e-wire`: scenario files, `.raventest`, one block per line (`open`, `click`, `type`, `check`, `mention`, read in Portuguese too), with `Scenario::parse` and `Scenario::write`; the hello of an app can carry the scenarios it keeps in its `tests/e2e/` (`Hello::scenarios`, `Client::scenarios`), and `check_folder` keeps that folder and what the app embeds the same. Raven Finanças brings its scenario this way, and Raven Test Studio runs an app's scenarios or files from any folder (`--run <file|folder>`) and brings none of its own. ([#652](https://github.com/alexkads/RavenRS/issues/652))
- `ravenrs-e2e-wire`, the test channel of RavenRS apps: a tool opens an app in a child process with no window (`Client::spawn`) and drives it one JSON line per step (`Command`: click, type, check, mention, snapshot, bye); the app, opened with `--e2e-serve`, answers with an `E2e` session of its own (`serve`, feature `serve`, on by default). A failure crosses the channel with its code, fix, close matches and screen; the channel adds `E2E_APP_START`, `E2E_TIMEOUT` and `E2E_WIRE`. `Command::apply` runs the same step in process. ([#649](https://github.com/alexkads/RavenRS/issues/649), part of [#627](https://github.com/alexkads/RavenRS/issues/627))
- `ravenrs-app-ui`: the editors of a generated form carry the test id `field:<field>`, and the buttons of `EntityForm` `form:save` and `form:cancel`, so a test finds them whatever their captions say. ([#649](https://github.com/alexkads/RavenRS/issues/649))
- `ravenrs-mcp`: `ravenrs_tools` lists the short names of a tool (`also_called`) and finds it by them, such as `RTS` for Raven Test Studio. ([#649](https://github.com/alexkads/RavenRS/issues/649))
- `ravenrs-e2e`, end-to-end tests of RavenRS screens with no window: an `E2e` session finds a part of the screen by the id the app declared, by role and name, by the text it shows or by its reference in a snapshot (`Query`, `Locator`, `within`, `nth`), acts on it with real input events (`click`, `double_click`, `right_click`, `hover`, `type_text`, `press`, `drag_to`) and refuses when the target is disabled, outside the window or covered, checks it (`to_have_value`, `to_have_name`, `to_be_checked`, `to_be_disabled`, `to_be_enabled`, `to_be_focused`, `to_be_visible`, `to_have_count`), and writes the screen as text with a reference per node (`E2e::snapshot`). Every failure has a stable code (`E2E_NOT_FOUND`, `E2E_AMBIGUOUS`, `E2E_DISABLED`, `E2E_COVERED`, `E2E_NOT_VISIBLE`, `E2E_PROTECTED_VALUE`, `E2E_EXPECTATION`), the screen and up to three close matches. ([#632](https://github.com/alexkads/RavenRS/issues/632), part of [#627](https://github.com/alexkads/RavenRS/issues/627))
- `ViewExt::test_id` and `BuildCx::test_id`: an id tests and automation find a node by, which survives reordering and wrapping, reaches the first accessible node inside a plain container, and is `author_id` to AccessKit and `data-testid` on the web, never read by screen readers. `AccessNode::test_id`, `AccessTree::by_test_id`, and `AccessNode::protected` with `Semantics::protected`, set by `TextField::secure`, which AccessKit now reports as a password field. `Ui::access_node_at` tells which accessibility node is under a point. ([#632](https://github.com/alexkads/RavenRS/issues/632))
- Raven Test Studio (`tools/raven-test-studio`, version 0.1.0), the end-to-end test tool of RavenRS as an app made only of flow components: a scenario is a flow of blocks (open, click, type, check, mention) run with `ravenrs-e2e`, each block marked done, failed or not run as it goes. It tests itself: "O Studio testa o Studio" opens another Studio and checks running, stepping, stopping and failing, 18 of 18 blocks. With no window, `--run <scenario> --format json` gives a tool the verdict, the stable code of a failure, what it means, how to fix it and the screen under test; runs are cached by scenario and build (`--no-cache`, `--clear-cache`). `claude-pane/` shows it beside a Claude Code conversation, with a `run_scenario` tool the AI calls. ([#634](https://github.com/alexkads/RavenRS/issues/634), part of [#627](https://github.com/alexkads/RavenRS/issues/627))
- `tools/`, the folder of installable RavenRS tools: each has its own version, news, installer, icons, `--versao`, `--self-test`, tests and a `tool.json` saying what it offers, which `scripts/check-tools.py` checks in `check.sh`. ([#634](https://github.com/alexkads/RavenRS/issues/634))
- Raven Test Studio tests itself when its window opens and says how it went in the status bar, and its Claude pane does the same as the session starts, running the installed Studio outside the RavenRS checkout. `--delegate codex` hands a scenario to the Codex CLI with its cheapest model, reading only, to run it and find the cause of a failure in the Rust sources; the Studio then runs the same scenario and says whether it confirms the answer. The pane's `run_scenario` takes `delegate` and `model`, and the plugin brings the skill `raven-test-studio`. ([#634](https://github.com/alexkads/RavenRS/issues/634))
- Raven Test Studio 0.1.1 stays beside the clock when its window closes, as Raven Finanças does, and runs as one copy; every tool in `tools/` now does, and `scripts/check-tools.py` checks it. ([#641](https://github.com/alexkads/RavenRS/issues/641))
- `ravenrs-app-update`: `check_now` and `install_now` ask for a newer version and install it on the calling thread, by the same rule and with the same installer as the window, for a command line or another program asking the app. Raven Test Studio 0.1.2 answers `--atualizacao` and `--atualizar` with them, refuses options it does not know (`--help` lists them), and its Claude pane says when a newer Studio is out, every hour, and installs it from there. ([#645](https://github.com/alexkads/RavenRS/issues/645))
- `ravenrs-mcp`: `ravenrs_tools` answers with what each tool of `tools/` offers, from its `tool.json`, with its README; `skills` lists and shows the skills of the repository (`.claude/skills`, the plugin's and the tools'). The tools' sources and READMEs are indexed with the examples', so `guides` and `component_examples` reach them. ([#634](https://github.com/alexkads/RavenRS/issues/634))
- `ravenrs-agent-cli`: `Policy::model` and the node parameter `model` choose the model, refused before running by a CLI that cannot; `cheapest_model` and `cheapest_codex_model_in` find an agent's cheapest one, for Codex from the list its installed CLI keeps. ([#634](https://github.com/alexkads/RavenRS/issues/634))
- `Expect::to_have_description` and `Expect::to_mention` in `ravenrs-e2e`, and every node of a `FlowCanvas` has the test id `node:<id>`. ([#634](https://github.com/alexkads/RavenRS/issues/634))

- `ravenrs-agent-cli`: the flow nodes `codex`, `claude` and `code_review`, which call the coding agents installed on the computer through `ravenrs-process`. Reading only by default (`--sandbox read-only`, `--permission-mode plan`); every limit checked against the installed CLI's `--help` and refused before running when it does not keep it; JSON events read into `AgentEvent`s with secret values taken out; the session to resume; the cost when the CLI says it, unknown otherwise. Raven Flows offers them in its group Agentes. ([#559](https://github.com/alexkads/RavenRS/issues/559))
- `ravenrs-process`, one executor of child programs: arguments with no shell, an environment of allowed variables only (`BASIC_ENV`, `allow_env`, or `inherit_env`), input lines, output and error lines as events in order on each stream, a time limit, and a stop that reaches everything the child started (its process group on Linux and macOS, `taskkill /T` on Windows); `resolve` and `version`. Raven Code runs cargo with it, and `ravenrs-pagamento` its TEF helper (`CanalProcesso`), with the same behavior. ([#558](https://github.com/alexkads/RavenRS/issues/558))
- `FlowCanvas::open_end(models)`: letting go of a connection where no port is, or pressing Enter again on the port it started from, opens a search of the models there; only those the port takes, by the same rules as any connection, are listed, and choosing one creates it there connected, as one change that one Undo takes away. Escape closes it and creates nothing. On the Diagrama de fluxo page of the feature center. ([#527](https://github.com/alexkads/RavenRS/issues/527))
- A field of data can be carried to where an expression is written: `FieldDrag` (and `FIELD_DRAG`, `field_expression`) is what the drag carries; the field chosen in a `DataInspector` shows as a tag to carry, with Copiar como expressão for the keyboard; an `ExpressionEditor` takes it at the caret as `[cliente.nome]`, a text field of a `ParameterForm` as `{{ [cliente.nome] }}` (an expression field at its end), and any other field refuses it, saying why. ([#532](https://github.com/alexkads/RavenRS/issues/532))
- `ravenrs-criteria` has `Template`: text with expressions inside it (`Olá {{ item.nome }}`, a field path needs no brackets, `\{{` writes braces), every problem found at once with where it is, `references` for the fields read, and `render_within` that stops past a limit; a failed expression is `RenderError::Failed` with where, never an empty value. In `ravenrs-flows`, `expression` gives what an expression of a flow reads (the item, `nodes.<id>` of the nodes that ran, `vars`, `run`), `check` marks a node the flow lacks, `preview` works a template out under `PREVIEW_LIMIT`, `NodeContext::text` reads a parameter as a template, and a field of `set` may be one. `ExpressionEditor` shows a `preview` or a `preview_error` under the text, and has its page, Editor de expressões, in the feature center. ([#533](https://github.com/alexkads/RavenRS/issues/533))
- `DataInspector` looks at JSON items as a table (a column per key), a tree (`TreeView`), the JSON itself and a schema (the kinds at each path and in how many items it is). The item and the field chosen stay from one way to the next, the field as a path (`field_path`: `cliente.nome`, `itens[0].preço`, `["nome completo"]`); absent shows apart from null; whole numbers stay exact; past `limit` (`SAMPLE` by default) a list is cut with a notice. On the Inspetor de dados page of the feature center, in the gallery and in the playground. ([#531](https://github.com/alexkads/RavenRS/issues/531))
- `ParameterForm`, a form made from a schema of parameters (`ParamField`, `ParamKind`): text, long text, number, yes or no, a choice, options the app loads (`ParamOptions`: loading, ready, failed with Tentar de novo) that are asked again when a field they depend on changes and drop a choice they no longer have, named groups, lists of rows that grow and shrink, fields shown in one case, defaults and required fields. Each field may hold an expression instead of a value (`EXPRESSION_KEY`). It edits a copy of its `ParamValues`, which Aplicar gives back and Descartar forgets. Made of the editors there are; on the Formulário de parâmetros page of the feature center, in the gallery and in the playground. ([#530](https://github.com/alexkads/RavenRS/issues/530))
- `FlowCanvas::execution(FlowRunView)` draws a run that happened outside the canvas, as the app tells it: each node's status, drawn and said to screen readers, and on each connection how many items went along it, after its own words, on a copy for drawing only. `FlowNodeStatus` has `Waiting`, `Skipped`, `Cancelled` and `Pinned` (it is `non_exhaustive`). `FlowCanvas::read_only` locks editing for good, offering no unlock, while the view pans, zooms and selects. Both are on the Diagrama de fluxo page of the feature center. ([#528](https://github.com/alexkads/RavenRS/issues/528))
- `ravenrs_flows::card` draws a node of a kind with its ports, for `to_graph` and for an app's `FlowToolbox` alike, and `port_name` reads a drawn port back. ([#590](https://github.com/alexkads/RavenRS/issues/590))
- `wait_event` of `ravenrs-flows` can be a gate: with `keep` true it gives the items that waited, each with the first item the event brought under the field `as` (`"event"` by default), instead of only the event's items. ([#590](https://github.com/alexkads/RavenRS/issues/590))
- `examples/raven-flows`, the automation app made only of RavenRS components, has its first version: flows kept as JSON files, the editor on `FlowCanvas` with the toolbox of `Catalog::builtin` and parameters in JSON, runs worked by `ravenrs-flows-runtime` on a thread of its own and drawn on the diagram with each node's `FlowNodeStatus` and each connection's item count, the runs with Repetir and Aprovar e continuar, and its news in the format of Raven Finanças. ([#590](https://github.com/alexkads/RavenRS/issues/590))
- `ravenrs-app-update`: installing, updating and versioning an app by compiling it on the user's machine, taken out of Raven Finanças, which keeps the same folders, addresses and behavior. `InstalledApp` says who the app is; `Updater` looks for a version every hour and runs the installer in the background; `render_installer` makes the app's `install.cmd` from one template for Windows, macOS and Linux, and `assert_installer_is_current` and `assert_release_files_agree` keep the installer and the three release files in step. ([#589](https://github.com/alexkads/RavenRS/issues/589))
- Diagrams carry the app's data: `FlowNode::data` and `FlowGraph::data` ride through copy, paste, undo and JSON untouched. A node lists where its data names other nodes under `NODE_REFS` (`"$node_refs"`, JSON pointers), and pasting renames those names to the copies' ids; `FlowGraph::insert_with` lets the app rename what the convention does not cover. `FlowGraph::from_json` moves JSON of an earlier version to `FLOW_FORMAT` and refuses a newer one (`FlowJsonError::Newer`). In `ravenrs-flows`, `to_graph` keeps each node's setup in its card and the flow's name in the graph (`GRAPH_KEY`), and `FlowDefinition::from_graph(graph, catalog)` no longer needs the flow the drawing came from. ([#529](https://github.com/alexkads/RavenRS/issues/529))
- `ravenrs-flows-runtime`, a new crate for durable runs on SQLite. `Runtime::enqueue` keeps the flow as it is and the input. Workers `claim` the run due first with a lease, in one statement, so two never take the same run. Each node is written as it starts and as it settles. A run whose worker died goes back to the queue when the lease runs out, and goes on from its checkpoint without running again what was over: a node left running is asked to `reconcile` first. Runs wait for a time or an event (`signal`) and can be run again on the original or current flow (`rerun`). Old runs are forgotten with their files (`prune`, `Retention`). ([#535](https://github.com/alexkads/RavenRS/issues/535))
- `ravenrs-flows` can be kept and resumed: `Executor::on_step` (`Step`), `Executor::resume`, `events` and `woken`; nodes wait with `NodeError::wait_until` and `wait_for` (`Failure::Suspended`, `Wake`, `NodeStatus::Waiting`, `RunOutcome::Waiting`); `NodeContext::event` and `woken`; `NodeRun`, `NodeStatus` and `RunOutcome` are serializable; new built-in `delay` and `wait_event`. ([#535](https://github.com/alexkads/RavenRS/issues/535))
- `ravenrs-flows` stops and tries again: `CancelToken` (cancelled from any thread, with children for subflows) through `Executor::cancel_with`, time limits for each try (`NodeDef::timeout_ms`) and for the run (`Executor::time_limit_ms`), and `Retry` (`tries`, `wait_ms`, `backoff`, `on_timeout`) by the `Failure` a kind reports (`NodeError::retryable`, `unknown`, `cancelled`, `timed_out`); a call of unknown result goes to `NodeKind::reconcile` (`Reconciled`) before any new try. `NodeContext` has `token`, `check`, `sleep`, `time_left_ms` and `attempt`; `Clock` has `sleep_ms`, and `ManualClock` moves instead of waiting. A run says how it ended (`Run::outcome`, `RunOutcome`), `NodeStatus` has `Cancelled`, `TimedOut` and `Uncertain`, and `NodeRun` has `attempts` and `reconciled`. New built-in `wait`, and `nodes::Subflow` to run another flow; `Executor::start_with` gives a node the items a run starts with. ([#536](https://github.com/alexkads/RavenRS/issues/536))
- `CablePlug`: cables that hang, swing and settle between any two places of the window, two widgets, a widget and the pointer, or a widget and a point (`CablePlug::between`, `from`, `loose`, `to_point`, `sides`, `via`). They have a color, thickness, shine, plugs, dashes and a texture (`CableTexture::Smooth`, `Braided`, `Coiled`, `Striped` or a picture of yours repeated along them), a slack and a whole `CablePhysics`; when they are no longer built they fall, vanish or are pulled back to their start (`release`, `retract`), and they answer `on_click` and `on_drop`. `BuildCx::cables` and `Ui::cables` read their shapes back. ([#575](https://github.com/alexkads/RavenRS/issues/575))
- `ravenrs_core::cable`: the rope model behind them, shared by widgets, exports and renderer: `Cable`, `CableEnd`, `CableView`, `CableBook`, the `CableWorld` trait with `CpuCables`, `cable_rest` (the same rest for the same ends), `cable_primitives`, `cable_plugs` and `cable_curve`, and `Primitive::Cable`. The step is fixed (1/120 s), so the same events give the same shape at any frame rate, and the window paints only while a cable still moves. ([#575](https://github.com/alexkads/RavenRS/issues/575))
- `ravenrs-render`: `GpuCables` simulates every cable of a window in one compute dispatch and the renderer draws them straight from its buffer (`Renderer::set_cables`), reading back only four bytes of movement and the points, a frame late; the native and the web platforms use it where compute shaders run, and WebGL2 keeps the CPU world. `render_with` draws a scene with a renderer of the caller's. ([#575](https://github.com/alexkads/RavenRS/issues/575))
- `PatchPanel`: a panel of jacks joined by cables that hang and swing, for the back of an amplifier, a rack or a switchboard. A cable goes from an output (`Jack::output`) to an input (`Jack::input`) of the same signal, one to an input; the plug of an input is taken out to plug it elsewhere and let go on the panel to unplug; `accept` changes the rule, `retract` sends a cord let go of back to its jack, `pair` and `on_join` make cords that answer and call, and `front` with `flipped` turns the equipment around. The keys do all of it (Enter, the arrows to the next jack that takes the cable, Delete, Ctrl+Z), each jack says what it is joined to, and the panel asks for frames only while a cable moves. `on_connect`, `on_disconnect`, `color_of`, `texture_of`, `thickness`, `fit`, `size`, `label` and `background` complete it; the page Painel de tomadas of the feature center shows every feature. ([#578](https://github.com/alexkads/RavenRS/issues/578))
- Cables with a picture on them (`CableTexture::Image`) are drawn by the GPU too, from a buffer of pictures sent once each, as the shapes draw them. ([#575](https://github.com/alexkads/RavenRS/issues/575))
- `ravenrs-perf`: the scenes `cables` (200 `CablePlug`s), `flow-cables` and `flow-cables-large` (a `FlowCanvas` of 60 and of 1,000 nodes joined by cables) and `patch-panel`; the harness gives each scene the GPU world of cables a window has, or the CPU one with `--cpu-cables`, and `gate --bless <filter>` writes only the scenes asked for. `ravenrs_core::cable::cable_body` draws a cable without its plugs. ([#575](https://github.com/alexkads/RavenRS/issues/238))
- `FlowCanvas::cables` and `FlowNotation::cables` draw the connections as cables: simulated in the diagram's units so panning does not shake them, a new one drops, moving a node or an end swings them, one taken away falls; clicks and labels follow their shape, reduced motion shows them at rest, and `FlowGraph::picture_with` exports them at rest (`FlowGraph::edge_cable` gives a connection's cable). `flow-whiteboard` and `flow-chat-bot` have a Cabos switch. The feature center shows `FlowCanvas` with every feature ("Diagrama de fluxo") and `CablePlug` with every feature ("Cabos"): flow, processes, the back of a rack, a switchboard, a modular synthesizer, a network rack, matching columns, a tip tied to a button and a lab with every knob. `scripts/feature-center-coverage.py` also counts the types that build themselves into a `BuildCx` without being views, like `CablePlug`. ([#575](https://github.com/alexkads/RavenRS/issues/575))
- `DropdownMenu::open(Binding<bool>)` opens and closes a menu from code; in a `FlowCanvas`, Cmd/Ctrl+Shift+E now opens the Export menu.
- A press held still on a drag source for `HOLD_TO_CARRY` (0.4 s) starts carrying it, and `Ui::is_dragging` counts it, so on touch a finger picks up a toolbox model instead of scrolling.
- The debugger of `FlowCanvas` takes one or more ways at a `FlowGate::ChooseMany` (`FlowRun::step_ways`), whose join waits only while a token can still come; BPMN inclusive gateways use it.
- BPMN participants (pools) and message flows: a line between participants becomes a message, and BPMN XML writes and reads a collaboration with a process per participant; `FlowLibrary::starter` gives a library diagrams to start from while nothing is kept.
- Dashed lines of a diagram print dashed in reports (PDF, SVG and pictures).
- The diagram examples (`bpmn-editor`, `flow-chat-bot`, `flow-design-board`, `flow-whiteboard`) build for the browser, keeping their diagrams in its storage; the local AI stays on the desktop.
- `ravenrs-flows` has built-in kinds, all in `Catalog::builtin()` (module `nodes`): `manual`, `if`, `switch` (one output per case, from `NodeKind::outputs`), `merge` (append, by position, by key, first), `batches` (a loop in batches), `set`, `filter`, `sort`, `group` (sum, count, min, max, avg, list), `split` and `join`, with conditions and values in the criteria language. A run now goes round loops (`NodeContext::iteration`, `Plan::loop_body`, `Executor::loop_limit`, `LOOP_LIMIT`), counts how often each node ran (`NodeRun::runs`), and every node has an error output (`ERROR_PORT`, `Item::failed`) with a new policy, `ErrorPolicy::Route`. ([#534](https://github.com/alexkads/RavenRS/issues/534))
- `ravenrs-flows`, a new crate that runs flows drawn on a `FlowCanvas`, with no screen: `FlowDefinition` (nodes with kind, version, parameters, disabled state, error policy and pinned items, and connections between named ports, as versioned JSON), `NodeKind`, `NodeSpec` and `Catalog` (kinds by name and version), `Plan` (checks a flow and orders it, refuses a cycle with no loop node, lists the nodes no trigger reaches) and `Executor` (the whole flow, from a node with an earlier run's items upstream, or one node alone, on an injected clock, with `Item`s that say where they came from). `FlowDefinition::to_graph`, `from_graph` and `Catalog::run_rules` connect it to the canvas and its debugger. ([#525](https://github.com/alexkads/RavenRS/issues/525))
- Reports draw flow diagrams: `ControlKind::Diagram` (`Control::diagram`) takes the JSON a `FlowCanvas` keeps, fixed or from a field, and fits it into the control with its titles as text, only some nodes if asked; "Diagrama" in the report designer's toolbox with its field, JSON and nodes. `ravenrs_report::diagram::fit` lays a diagram into a box, and `ravenrs-flow-export` writes its PDF through it; `ravenrs_report::flow` re-exports the diagram model.
- `examples/raven-code` grows into an editor for every day. It asks before closing the window or the tab of a file with changes, and keeps how each project was left (the arrangement, the open files, where their carets were) and the recent projects in the user's folder of settings. It opens a folder from the toolbar (Ctrl+O). The Explorer makes, renames and deletes files and folders, and follows the disk: files changed outside reload, or ask when they have changes. A Search panel looks through every file (Ctrl+Shift+F). The toolbar has Test (Ctrl+Shift+T), Check and Clippy, a choice of binary, and Stop (Shift+F5). The terminal takes what the program reads, with Ctrl+C to stop it. With rust-analyzer installed, problems show while typing, with suggestions, tips and F12 to the definition. Pictures in `docs/media/raven-code/`. ([#496](https://github.com/alexkads/RavenRS/issues/496), [#497](https://github.com/alexkads/RavenRS/issues/497), [#500](https://github.com/alexkads/RavenRS/issues/500), [#501](https://github.com/alexkads/RavenRS/issues/501), [#502](https://github.com/alexkads/RavenRS/issues/502), [#503](https://github.com/alexkads/RavenRS/issues/503), [#504](https://github.com/alexkads/RavenRS/issues/504))
- Undo and redo in every `TextField`, `TextArea` and `CodeEditor`: Cmd/Ctrl+Z undoes, Cmd+Shift+Z or Ctrl+Y redoes. A word typed letter by letter is one step, and so is each space, paste, cut and line break; moving the caret closes the step, and a change the app makes by itself is never undone past. `TextHistory` keeps the steps, only what changed in each, up to 200. `CodeEditor::history` lets the app keep them per document and undo from a button, and `TextHistory::record_change` makes the app's own change a step. ([#499](https://github.com/alexkads/RavenRS/issues/499))
- Find and replace in a `CodeEditor`. Cmd/Ctrl+F opens a bar over the code with what is selected, and Ctrl+H opens it with replacing. The matches are marked in the code, with "2 of 5", capitals and whole words. Enter and Shift+Enter walk the matches, going round, and Escape goes back to the code at the match. Replace all is one undo. `find_all` finds the same way for any text. ([#500](https://github.com/alexkads/RavenRS/issues/500))
- Help from a language server in a `CodeEditor`, which knows nothing of any server. `on_complete` and `completions` give a list under the caret after `.`, `::` or Ctrl+Space; it narrows by the word typed, the arrows choose, and Enter or Tab put the choice in (one undo). `on_hover` and `hover` give a tip once the pointer rests half a second. `on_go_to` goes to a definition, by F12 or a Cmd/Ctrl click. `CodeCompletion` is a suggestion. ([#497](https://github.com/alexkads/RavenRS/issues/497))
- `CodeEditor::follow_end`: the last line stays in sight as the text grows, unless it was scrolled away from, for what a program prints. `TextField::recall`: the lines Enter sent come back with Up and Down, as at a terminal. ([#496](https://github.com/alexkads/RavenRS/issues/496))
- `TreeView::context_menu`: actions for an item, by its key. A right click selects the item and opens them there; Shift+F10 or the Menu key open them under the active item. ([#502](https://github.com/alexkads/RavenRS/issues/502))
- `OpenDialog::folders`: the system's dialog chooses a folder instead of a file. ([#501](https://github.com/alexkads/RavenRS/issues/501))
- `BuildCx::on_close_request`: a view holds the window when its close button is pressed, to ask first (unsaved changes, say). It closes it afterwards with a `WindowCloser` from `BuildCx::window_closer`. Natively the window stays open, and does not go to the tray, until then. On the web the browser asks before leaving the page. `Ui::may_close` and `Ui::take_close` are what platforms read. ([#503](https://github.com/alexkads/RavenRS/issues/503))
- `FlowToolbox`: models of node in groups that open and close, with a search, each a drag source (`FLOW_NODE_DRAG`) the canvas takes as a copy with an id of its own, on the grid, as one change; Enter on a model leaves it in `FlowCanvas::arrivals`, which places it in the middle of the view.
- Bend points on connections: `FlowEdge::waypoints`, a `+` in the middle of each stretch of the line selected to drag into a point, a double click on a line, Tab to reach the points, the arrows to move them and Delete to take them out; a point let go on the straight between its neighbours goes, lines whose two ends move take theirs along, and step lines turn at them.
- Export: `FlowCanvas::on_export` puts PNG (2×), SVG and PDF in the controls, the whole diagram or the selection, and Cmd/Ctrl+Shift+E exports again; `FlowGraph::picture` and `FlowPicture::to_svg` in `ravenrs-graphics`, and the new crate `ravenrs-flow-export` writes the PNG and the PDF and hands the file over (`saver`).
- `FlowLibrary`: the diagrams an app keeps in a `Storage`, to make, save, open, rename and delete (asked first), with changes not saved said; switching keeps the one open first and starts the history over; diagrams go to and come from JSON files, and other formats with `FlowFileFormat`.
- `FlowNotation`: a notation as data (palette, how kinds look, how connections are drawn and which may be made, the next node, checks, runs and file formats), given to `FlowCanvas`, `FlowToolbox`, `FlowInspector` and `FlowLibrary` with `.notation(…)`.
- `FlowCanvas::checks`: Check in the controls says what is wrong with the diagram in a box kept up to date and writes it on the nodes it is about (`FlowIssue`).
- `FlowCanvas::runs`: Debug runs the diagram a step at a time (`FlowRun`, `FlowRunRules`, `FlowGate` in `ravenrs-graphics`): Start, Step, Continue and Stop, the way out chosen at a choice, breakpoints with F9 or a button, the nodes and lines the run went through shown, and dead ends, joins that never complete, loops and a missing start said on the node.
- `FlowCanvas::connection` shapes each connection made (its kind, caps and dashes); `FlowCanvas::ports_on_hover` shows ports nothing joins only by the node under the pointer; names of ports show by them; guides also line a node up with the frame it is in.
- `FlowStyle`: fill, line color and width and text color per node, the title under the outline and a sign in its middle; `FlowEdge::color` and `dashed`; the `FlowInspector` edits them; `FlowShape::Text` for words alone; `FlowNodeStatus::Running` and `Success`.
- `FlowLayers`: the nodes in the order they are drawn, to pick, hide (`FlowNode::hidden`) and bring forward or send back; `FlowLayout::mind_map` grows a mind map to both sides and turns the ports of the left half.
- `ChatMessage::when_done` and `LocalAssistant::reply` (and `answer_long`) take a local model's answer to a `Chat`, written as it comes, its text handed to the app once it is over.
- `ravenrs-flow-bpmn`: BPMN 2.0 for the diagram components as data and functions: the notation, BPMN XML with its drawing in and out (other tools' files open), the checks of a process, how it runs, and plans a local model writes, read even when their JSON is broken and mended out loud.
- Examples made only of library components to prove the diagram component: `bpmn-editor` (BPMN 2.0 with a local AI that drafts and reviews processes, checks and a debugger), `flow-chat-bot`, `flow-design-board` and `flow-whiteboard`.
- `FlowInspector`, the properties of what is selected in a `FlowCanvas`: title, line, kind, shape, size, place and status of a node; label, shape, ends and motion of a connection; for several nodes, the kind and shape they share, a value that differs shown as mixed and, once chosen, given to all. It works on the canvas's diagram, selection and history, asks the same `on_change` (`FlowChange::Edit`) and shows a refusal under its fields; text is kept on Enter or leaving, choices and numbers as they are made; `read_only` for a locked canvas. ([#396](https://github.com/alexkads/RavenRS/issues/396))
- `FlowCanvas` alignment guides: a dragged node lines its edges or middle up with another node's within 6 pixels on screen, before the grid, and a 1-pixel guide shows the line; Alt drags without them. They take a new token, `guide` (`Semantic::Guide`), magenta in every theme, apart from the accent of the selection and the focus ring. ([#396](https://github.com/alexkads/RavenRS/issues/396))
- `FlowCanvas` quick creation: arrows on the sides of the node under the pointer (and of a node selected alone) make the next node there, a layer away on the grid and lower down while that place is taken, joined output to input, selected and with its title open to type. Cmd/Ctrl+Enter does it to the right of the selected node and Cmd/Ctrl+Shift+Enter below. It is one change to undo, asked of the app as `FlowChange::Create`. `FlowCanvas::next_node` chooses what is made (a copy of the node by default) and `FlowCanvas::quick_create(false)` turns it off. ([#396](https://github.com/alexkads/RavenRS/issues/396))
- `ravenrs-app-server`: the bodies in `/api/openapi.json` say exactly what the server takes: `additionalProperties: false`, no field it refuses, the aggregated collections with their records to create, to change by key or to delete (`<Entity>.<collection>.Create` and `.Change`), and each operation of `/api/batch` by entity and method. Patterns with `.`, negated classes, `\D`, `\S` and `\W` go out too, read alike with and without the `u` flag. ([#508](https://github.com/alexkads/RavenRS/issues/508), [#509](https://github.com/alexkads/RavenRS/issues/509))
- `ravenrs-app-server`: `/api/openapi.json` has `<Entity>.Create` and `<Entity>.Change` for the bodies of POST and PATCH, with what the rules of the entity always hold a write to: `required`, `nullable`, `minLength`, `minimum`, `maximum` and `pattern`. Only what every client reads the same goes out, so the description never turns down what the server takes; the record as read stays as it was. ([#505](https://github.com/alexkads/RavenRS/issues/505))
- `examples/raven-code`: a small editor for Rust projects on the `DockArea`, the start of the RavenRS IDE. It opens a folder (or a sample crate), shows its files in a tree, edits and saves them (Ctrl+S, Cmd+S on a Mac), and builds (Ctrl+B) and runs (F5) with cargo on a thread of its own, the output in a Terminal panel and the compiler's errors and warnings marked in the code and listed in Problems, where a click goes to the place. `cargo run -p raven-code -- <folder>`; `RAVENRS_CAPTURE_DIR=<folder> cargo test -p raven-code --test capture` draws it to PNG. Picture in `docs/media/raven-code/`. ([#495](https://github.com/alexkads/RavenRS/issues/495))
- `Cursor` is exported from `ravenrs-widgets` (and the prelude's widgets), so an app can hold the caret of a `CodeEditor` or `TextField` in a `State<Cursor>` and move it. ([#495](https://github.com/alexkads/RavenRS/issues/495))
- `examples/dock-workbench`: three desktop apps built on the `DockArea` (a code editor, an IDE and a chat app), to check that the dock holds one together. `cargo run -p dock-workbench -- editor` (or `ide`, or `chat`); `RAVENRS_CAPTURE_DIR=<folder> cargo test -p dock-workbench --test capture` draws them to PNG. Pictures in `docs/media/dock-workbench/`.
- Ctrl+Tab in a `DockArea` (Ctrl, not Cmd, on a Mac too): a quick press goes back to the panel used before; held for 0.15 s it shows a list of the panels by how lately they were used, documents apart from tools, each with where it is (a dock, floating, hidden at an edge). Tab and Shift+Tab walk it while Ctrl is held, letting Ctrl go opens the choice (over the rest if it is hidden, in front if it floats), a click picks a row, and Escape gives it up. The order of use is kept in memory only, never in what is saved. ([#490](https://github.com/alexkads/RavenRS/issues/490))
- `BuildCx::held`: the keys held now, for a view that looks different while Ctrl or Shift is down; the views are built again when one is pressed or let go. ([#490](https://github.com/alexkads/RavenRS/issues/490))
- `BuildCx::is_focus_within`: whether keyboard focus is on a view or inside it. The hidden panel of a `DockArea` open over the rest uses it to go back when Tab takes focus out of it. ([#484](https://github.com/alexkads/RavenRS/issues/484))
- `MenuItem::disabled(reason)`: the item shows greyed, with the reason where its shortcut would be; neither a click nor Enter chooses it, the arrows still reach it, and screen readers hear it as disabled, with the reason. The Move to… menu of a `DockArea` shows the places a panel may not go that way. ([#492](https://github.com/alexkads/RavenRS/issues/492))
- `DockPanel::unsaved(bool)`: a dot on the panel's tab, on its rail when it hides at an edge, and on its header when it floats, and "Unsaved changes" for screen readers, as `DocumentTab::unsaved` does. ([#493](https://github.com/alexkads/RavenRS/issues/493))
- `CursorIcon::Move`, on the web and native: the header of a floating panel shows it, and the edges a floating panel is drawn to show as warning-colored lines while it moves. ([#487](https://github.com/alexkads/RavenRS/issues/487))
- Documents and tools in a `DockArea`: `DockPanel::kind(DockPanelKind::Document)` makes a panel a document. As soon as an area has one, documents go only in the center and tools only in the docks: a drag marks nothing elsewhere, Move to… does not offer it, and the keys say why ("Documents stay in the center"). `DockArea::free_placement(true)` lifts that, and an area with no documents is as before. `DockPanel::close_action(DockCloseAction::Remove)` closes a panel for good, so the menus do not offer it back. A document's menu has "Close others", which asks the app about each and stops at the first it keeps. `DockArea::on_documents_empty` puts the app's own view in an empty center, with what can come back under it. ([#399](https://github.com/alexkads/RavenRS/issues/399))
- Drag and drop between components: `BuildCx::drag_source` with a `DragPayload` (a kind, a label and the app's value) and `BuildCx::drop_target` with a check that can refuse and say why. The pointer carries past a few pixels (a shorter press still clicks), and the keys carry too: Space picks the focused source up, the arrows move it (Shift for four times as far), Enter drops and Escape gives up. A tag with the label, or the refusal, follows what is carried, screen readers hear it, and `BuildCx::carrying` lets a target draw its own preview. ([#396](https://github.com/alexkads/RavenRS/issues/396))
- Panels of a `DockArea` float over the area in boxes of their own: "Float" in the menu of a panel, Ctrl+Alt+F (Cmd+Alt+F on a Mac), or a tab dragged with Ctrl held or let go outside the area. A floating panel moves by its header, drawn to the edges of the area and of the other floating panels within 8 pixels, and lands only on the tabs of a group or the border of a dock that does not show, so it passes over the groups (with Shift, anywhere a tab lands). Its edges and corners resize it, no smaller than 200 by 120; the arrows move it and Alt with them resizes it; its button, a double click on its header or Ctrl+Alt+F put it back beside the panel it was with. It stays inside the area as the area shrinks, without changing what is saved. `DockLayout::float`, `floating_home`, `is_floating`, `floating_panels`, `floating_rect`, `set_floating_rect` and `bring_to_front`; `DockChange::Float`, `Dock` and `Place` for `on_change`; `DockArea::floatable(false)` to leave it out. ([#399](https://github.com/alexkads/RavenRS/issues/399))
- `CursorIcon::NeswResize`, the diagonal resize cursor from the bottom left or the top right corner, on the web and native.
- `FlowCanvas` shapes: `FlowNode::shape` takes a `FlowShape` (`Card`, as always, `Rectangle`, `RoundedRectangle`, `Ellipse`, `Diamond`, `Parallelogram`, `Document`, `Cylinder`, `Note`). Ports sit on the outline, the pointer finds the node only inside it, and a note wraps its words on a tinted square. `ravenrs_graphics::flow::shape_outline` draws each outline for any engine. Underneath, `BuildCx::hit_outline` lets any node be found only inside a polygon. ([#396](https://github.com/alexkads/RavenRS/issues/396))
- Panels of a `DockArea` hide at the edge of their dock: a pin at the end of each strip of a dock, "Auto-hide" in the menu of its panels, or Ctrl+Alt+P (Cmd+Alt+P on a Mac) puts the panel on a 28-pixel rail at that edge, its name down the rail. A click on its tab, or the pointer resting on it for 0.4 s, opens it over the rest, as wide as the dock was and without moving anything; a click outside, Escape, or (when the pointer opened it) the pointer leaving for 0.6 s lets it go back, and its pin puts it in its dock again, beside the panel it was with. Its edge resizes it, its tab can be dragged anywhere like any other, and the arrows walk the rail. `DockLayout::auto_hide`, `pin`, `is_auto_hidden`, `auto_hidden_panels`, `auto_hidden_side`, `auto_hidden_size` and `set_auto_hidden_size`, `DockChange::AutoHide` and `Pin` for `on_change`, and `DockArea::auto_hide(false)` to leave it out. ([#399](https://github.com/alexkads/RavenRS/issues/399))
- `BuildCx::flush_overlay`: a layer that touches its anchor with no gap and may reach the edge of the window, for a panel that opens out of a rail.
- Org charts and lines around nodes: `FlowLayout::tree` (and `FlowLayoutKind`) arranges a diagram as a tree, each node centred over the nodes it leads to, down as an org chart or across as a mind map; a node reached twice hangs under the first, and a forest is trees side by side. A step line goes around the nodes in its way (`ravenrs_graphics::flow::route_around`), and `RouteCache` works a line out again only when its ends or the nodes near it change; `FlowCanvas` keeps one. ([#397](https://github.com/alexkads/RavenRS/issues/397))
- `FlowCanvas::content` puts any view of the app inside a node: asked for each node on every build, the view is laid out at the node's size in units of the diagram and drawn at the zoom, its controls taking the pointer and keys while the rest of the card still moves and connects the node. Underneath, `BuildCx::scale(node, factor)` draws a subtree at a scale from its top left corner, and the pointer and screen readers find it where it shows. ([#238](https://github.com/alexkads/RavenRS/issues/238))
- `FlowCanvas` turns nodes: a round handle above a selected node turns it 15° at a time (with Shift, freely), and Alt+Shift with the arrows does the same, Alt+Shift+0 putting it straight. The whole drag is one change, the app is asked with `FlowChange::Rotate`, groups refuse, and `FlowCanvas::rotatable(false)` turns it off. `FlowNode::rotation` (degrees) turns the node's ports and the side they face (`FlowNode::facing`), `FlowNode::extent` is the box it shows in, and screen readers hear "Turned 30°". A turned node still resizes along its own sides. ([#395](https://github.com/alexkads/RavenRS/issues/395))
- `QuarterTurn` turns a view a quarter, clockwise or counterclockwise (`Turn`), in a box as wide as the view is tall and as tall as it is wide, so it lays out standing up: a name down a narrow strip at the edge of a window, or the title beside a chart. Drawing, the pointer, the focus ring and screen readers follow the turn, on top of the turns of #414. ([#458](https://github.com/alexkads/RavenRS/issues/458))
- Turns: `BuildCx::rotate(node, radians)` paints a node and its subtree turned around its center, text and images too, and the pointer and screen readers find it where it shows. Underneath, `Effect::rotated` and `Effect::rotation`, and `Quad::rotated`, `ImageQuad::rotated` and `GlyphRun::rotation`, which the renderer draws; movies keep the turn and read the ones recorded before as they were. ([#414](https://github.com/alexkads/RavenRS/issues/414))
- A top dock in `DockArea`: `DockLayout::top(node, size)` and `DockSide::Top`, a strip between the columns above the center, 120 pixels tall by default and kept between 60 and 400 (`dock_limits(DockSide::Top, …)`). It has its bar, its collapsing strip, "Top dock" first in Move to…, and a drop at the very top of the area, between the columns, opens it; only the top 4 pixels, so that drops among the tabs below keep working. When a short area has too little for both, the top gives way before the bottom. ([#399](https://github.com/alexkads/RavenRS/issues/399))
- `FlowCanvas` titles are typed in place: a double click, F2, or Enter on a selected node opens a field over the title with all of it selected; Enter keeps it, Escape gives it up, and leaving keeps it. An empty or unchanged title changes nothing, the app is asked with `FlowChange::Rename`, and below the zoom at which titles show the view goes to 100% around the node while it is typed. `FlowCanvas::editable_titles(false)` turns it off. A line's ends take `EdgeCap` (`None`, `Arrow`, `OpenArrow`, `Diamond`, `Circle`) through `FlowEdge::start` and `FlowEdge::end`, drawn by `ravenrs_graphics::flow::cap_outline` with the line stopping inside each; saved diagrams read and write as before. ([#395](https://github.com/alexkads/RavenRS/issues/395))
- `DockArea::on_change` asks the app before every change a person makes to the arrangement, with the arrangement before and after and a `DockChange` that says what it is (bringing a panel to the front, moving it, closing it, reopening it, resizing with a bar, collapsing a dock). A refusal leaves the arrangement as it was, calls no `on_end`, and shows its reason under the tab (or at the top of the area) for four seconds or until the next gesture, as a status screen readers announce; a refused drag of a tab or a bar puts it back. What the app writes to the binding itself is not asked. The bubble with the reason is shared with `FlowCanvas`. ([#399](https://github.com/alexkads/RavenRS/issues/399))
- `FlowCanvas::on_change` asks the app before every change to the diagram, with the diagram before and after and a `FlowChange` that says what it is (moving, resizing, connecting, deleting, pasting, grouping, arranging, undoing, reordering). A refusal leaves the diagram and the history as they were and shows its reason, which screen readers announce; a refused drag puts the nodes back. Cmd/Ctrl+] and Cmd/Ctrl+[ bring the selected nodes to the front or send them back, through `FlowGraph::bring_to_front` and `send_to_back`, which keep a group behind what it holds. ([#395](https://github.com/alexkads/RavenRS/issues/395))
- `FlowCanvas` groups: Cmd/Ctrl+G puts the selected nodes in a group sized to hold them, Shift+Cmd/Ctrl+G takes it apart, and dropping a node inside a group or outside it puts it in or takes it out. A group moves, copies and goes with what it holds, groups nest, a press on a line inside a group still picks the line, and arranging moves a group as one box. In `ravenrs-graphics`: `FlowNode::parent`, `FlowNode::group`, `FlowNode::new_group`, `FlowGraph::group`, `ungroup`, `set_parent`, `children`, `descendants`, `with_descendants` and `top`, and `FlowJsonError::MissingGroup` and `GroupCycle` when a diagram is read. ([#238](https://github.com/alexkads/RavenRS/issues/238))
- `FlowCanvas` arranges a diagram in layers from a button in its controls, as one change to undo, and resizes a selected node by its corner or with Alt and the arrows, on the grid and no smaller than 96 by 48. `FlowLayout` and `FlowDirection` in `ravenrs-graphics` do the arranging for any engine: connections run one way, crossings are few and a chain comes out straight; `FlowCanvas::layout` and `FlowCanvas::resizable` choose them. `CursorIcon::NwseResize` is the diagonal resize cursor, on the web and native. ([#238](https://github.com/alexkads/RavenRS/issues/238))
- `FlowCanvas` undoes and redoes (Cmd/Ctrl+Z, Shift+Cmd/Ctrl+Z, Cmd/Ctrl+Y), copies, cuts and pastes nodes with the connections between them and new ids, moves the end of a selected connection to another port, and connects ports with the keys (Tab, Enter). `FlowCanvas::history` shares the `FlowHistory`, and `FlowGraph::to_json` and `from_json` save and read a diagram, checking that it holds together (`FlowJsonError`). ([#238](https://github.com/alexkads/RavenRS/issues/238))
- **`ravenrs-deck`**, a new crate: the presentation model (slides, layouts and masters, shapes, text, tables, charts, animations and transitions), reading and writing PPTX and a native format, slides drawn to pixels, PDF export with notes pages and handouts, embedded audio and video probed and given poster frames, and the commands with undo an editor drives; it builds for the web too. ([#373](https://github.com/alexkads/RavenRS/issues/373))
- `App::themes` and `AppShell::themes` offer only the application's own themes in the Theme menu, start on the first, and keep the one each person chooses, as before. `ThemePicker::of` is that menu on its own.
- The tokens of a theme take a `"name"`, shown in menus and to screen readers. `to_tokens_json` writes it for a theme with a name of its own, and `from_tokens_json` takes it, up to 64 characters; without it nothing changes.
- Following the system can switch between an application's own light and dark: `SystemAppearance::set_themes`, `SystemAppearance::theme_of` and `Ui::system_themes`. `App::themes` sets that pair itself, from the first light and the first dark among them.
- `SecuredApp::door`, which lets the application arrange the screen of signing in from `DoorParts`: the fields of the name and password, Sign in, the last error, the buttons of each `sign_in_action` and `fill(name, password)` for chips of a demonstration, which only fills in. Checking the password, permissions, the audit trail and choosing a new password stay in the framework, and the first administrator and a new password keep its screens. `SecuredApp::aside` puts a view beside the card in a window at least 720 pixels wide. ([#366](https://github.com/alexkads/RavenRS/issues/366))

- `App::brand` and `App::brand_subtitle` (and the same on `AppShell`) put a mark, the title and a line under it at the top of the navigation. Reduced to icons, only the mark stays, and screen readers hear the title and the line, as with any `SidebarBrand`.
- `App::navigation_count` puts a `NavigationCount` beside an item of the navigation, worked out again whenever records change, with words of its own for screen readers (`NavigationCount::spoken`, `SidebarItem::count_spoken`).
- Cmd/Ctrl+K in an `AppShell` opens a `CommandPalette` with the places of the navigation the person can open.
- Reports take the colors of the theme on screen: `ravenrs_grid_export::paper_colors`, `document_of_with` and `export_with`, `Session::theme`, and the list report of `ReportModule` uses them. Paper stays white, so a dark theme keeps dark text.
- A theme chooses the series of a chart after the first in `Theme::charts`, `Theme::set_color` or its tokens, as `chart-2` to `chart-5`, and the `ThemeEditor` has a group for them. `Palette` does not change.
- `DataGrid`: the check box of a row says its name ("Select Ana", from `name_column`, the key or the first column); `GridRowAction::show_text` shows the label beside the icon and `GridRowAction::visible` leaves the button out of some rows, keeping its room; `GridColumn::fill_weight` fills by weight, like `fr`.

- `AnalyticsPanel` and `AnalyticsApp::analytics_panel` in `ravenrs-app-report`: a panel of numbers, bars, lines, pies and doughnuts declared in code over the records of a struct, with a filter and a date interval per tile, in the navigation like any view and read with the person's permissions. ([#68](https://github.com/alexkads/RavenRS/issues/68))
- `ApiService::webhook` and `Webhook` in `ravenrs-app-server`: `POST /api/hooks/{source}` checks the organization and the HMAC-SHA256 of the body before reading it, keeps each event in `WebhookEvent` by organization, source and provider id, applies it once and never lets an older fact undo a newer one. `ApiRequest` has `headers`, `header` and `bytes`. ([#256](https://github.com/alexkads/RavenRS/issues/256))
- `Playground::file_name`, a tab over the editor with the file's name and language, `Espaços: 4` in its status line, and a problem that takes the caret to its place when clicked, in the list or on the number of a marked line. `CodeEditor::focus_request` and `offset_of`. ([#166](https://github.com/alexkads/RavenRS/issues/166))
- `IconName::SquareFunction`, used by the buttons that edit an expression in the report designer; `scripts/gen-icons.py --add` copies new glyphs into the icon font without touching the others. ([#43](https://github.com/alexkads/RavenRS/issues/43))
- `cargo ravenrs doctor` reads RavenRS versions inherited from the workspace (`ravenrs.workspace = true`), renamed dependencies and tables per target, and every member at a virtual workspace root, without changing any manifest. ([#209](https://github.com/alexkads/RavenRS/issues/209))
- `create-ravenrs`: a guide for machines with little memory (LLD, two jobs, no debug info), and `npm run test:e2e:linux` takes `--memory`, `--jobs`, `--logs` and `--case`, building `hello`, `app-sqlite` and `relatorios` in a container of 4 GB. ([#220](https://github.com/alexkads/RavenRS/issues/220))

### Fixed

- `LogView` marks a section that folds with the chevron icons, and `DataInspector` marks the chosen field with →: they drew ▸ and ▾, which no embedded font has, as an empty box. The book refuses a design system with a letter no embedded font draws. ([#849](https://github.com/alexkads/RavenRS/issues/849))
- `ravenrs-e2e` reaches a row at the end of a long list: the hunt for a row not built yet scrolls to the end of the list instead of giving up after 240 turns of the wheel (a list of 10,000 rows did not reach row 9,999).

- `YearView` no longer overflows on a day before 1970 (a debug build panicked).

- `ravenrs-e2e`: actions scroll a target out of sight into view first, also inside nested scroll views and when it is inside the window but hidden by a short scroll view (#732). `Ui::scroll_frames` lists the parts that scroll around a node.

- Timeout reports describe the step's time limit without claiming the app was stopped: a failed `wait` can leave its process answering normally. The original condition or transport detail, code and screen remain in text, JSON and JUnit. ([#697](https://github.com/alexkads/RavenRS/issues/697))
- Raven Test Studio: preflight of an app's embedded scenarios (`--run financas`) preserves build/start error codes, details and fixes in JSON and JUnit, just as running scenario files does. Malformed embedded scenarios remain parse errors. ([#692](https://github.com/alexkads/RavenRS/issues/692))
- The aggregated HTML report links to each film's real file URL, so equal film names in different scenario folders no longer collide. Saved per-scenario pages keep portable relative links; file URL escaping handles spaces, Unicode and `#`. ([#690](https://github.com/alexkads/RavenRS/issues/690))
- The public API snapshot includes the e2e drivers already delivered in F3, so the local gate no longer reports them as unexpected additions. ([#684](https://github.com/alexkads/RavenRS/issues/684))
- `cargo ravenrs e2e rust`: generated test names have a `scenario_` prefix, so titles such as `type`, `self`, `open` and `step` compile without reserved words or collisions with the generated helpers. ([#678](https://github.com/alexkads/RavenRS/issues/678))
- Raven Test Studio and `cargo ravenrs e2e run`: a relative `--out` keeps the film beside the report even when the app under test runs in another project's folder. ([#676](https://github.com/alexkads/RavenRS/issues/676))
- Raven Test Studio: the app opener takes its session type from the shared runner directly, removing the three-module cycle that blocked the local gate. ([#677](https://github.com/alexkads/RavenRS/issues/677))
- `Scheduler`: clicking an event gives it the focus, so the arrow keys move the event that was clicked; they used to move the one that had the focus before, the first event when nothing was chosen yet. ([#665](https://github.com/alexkads/RavenRS/issues/665))
- Raven Finanças: the test of pairing Telegram from Ajustes failed on Mondays. The fake Telegram took the last message sent, which on a Monday is the week's summary sent to a new chat, not the answer to `/start`. It now takes the answer, and a failure shows what the bot said. ([#650](https://github.com/alexkads/RavenRS/issues/650))

- `FlowCanvas` keeps what it paints inside its frame: a connection to a node out of view, a cable's sag or a line being drawn no longer crosses the panel beside the canvas. ([#640](https://github.com/alexkads/RavenRS/issues/640))
- `ravenrs-e2e` waits for the frames an action asks for (up to four) before it reads the screen, so a check right after a click sees what the click changed. ([#634](https://github.com/alexkads/RavenRS/issues/634))
- Cables plugged into widgets under a scale or a turn end where the widgets show, thinner by as much, and the children of a scaled widget laid out past the window are no longer skipped when they show inside it. ([#575](https://github.com/alexkads/RavenRS/issues/575))
- On the GPU, a frame with no step due no longer tells the cables they stopped: a cable just plugged in used to freeze in the middle of its fall. ([#575](https://github.com/alexkads/RavenRS/issues/575))
- A run of cables draws their bodies together and their plugs after: 200 cables went from 401 draw calls to 3 on the GPU and from 407 to 13 on the CPU, and a `FlowCanvas` of 60 nodes with cables from 340 to 134. ([#575](https://github.com/alexkads/RavenRS/issues/575))
- A `FlowCanvas` with 1,000 nodes no longer walks every connection for every node on each build: dragging a node there went from 268 ms to 17 ms of CPU with cables and from 27 ms to 16 ms with lines (`ravenrs-perf` scene `flow-cables-large`). ([#238](https://github.com/alexkads/RavenRS/issues/238))
- The feature center opens a component's page in the whole center with `pagina=<id>` (it said the model had no such view), and the pages Cabos and Diagrama de fluxo fit a phone. ([#575](https://github.com/alexkads/RavenRS/issues/575))
- `FlowDefinition::to_graph` gave an input and an output of the same name (`main`, in most built-in kinds) one id, which a `FlowCanvas` port must not share within its node: a line leaving such a node started at its input, its label sat on the node's title, and a connection from that output was refused as input to input. The input is now drawn as `in:main` and read back as `main`. ([#590](https://github.com/alexkads/RavenRS/issues/590))
- `ravenrs-app-server`: a pattern in `/api/openapi.json` that is not anchored at the start, such as `\d` or `@`, goes out after as little as needed of anything, so a client that only tries it at the start of the text, as the Python one from openapi-generator does with `re.match`, no longer turns down what the server takes. Checked against that client, the patterns with `.` and negated classes read alike there too. ([#570](https://github.com/alexkads/RavenRS/issues/570))
- `ravenrs-app-server`: in `/api/openapi.json`, what a body writes is no longer marked read-only: clients generated from it (orval, openapi-generator) left out the state of a record and the key of one to change or delete. The records of a collection go in the order the service reads them, so a client that drops what a shape does not know keeps the key of one to change instead of making it a new one. A reference is a whole number or a text, `servers` and an `operationId` for each operation are there, and the description passes `redocly lint`. ([#514](https://github.com/alexkads/RavenRS/issues/514))
- Raven Code finds the errors rust-analyzer reports: it looks for rust-analyzer with the toolchain of the project's folder, as rustup picks it, names files by their real path (a Mac's temporary folder is reached through a link), and answers the server's question for settings. ([#524](https://github.com/alexkads/RavenRS/issues/524))
- `QuarterTurn` keeps a name on one line in a strip narrower than it: it is measured lying down in a room wide enough for it, so "Server Explorer" on a 28-pixel rail no longer breaks in two.
- A window arranged for a counter (`App::counter`, `CounterTemplate`) has no Cmd/Ctrl+K palette of places, as it has no navigation; an application shown inside another no longer answers that key twice. `FrameTemplate::shows_navigation`.
- A theme of an application's own, such as one made with `Theme { name: "Mine", ..Theme::raven_light() }`, wrote its tokens against itself, so the menu could not bring it back the next time. It now writes them against its base.
- `HeroBanner` in the accent tone writes in the theme's `on-accent`, and on any other tone or color in black or white, whichever has more contrast, so it reaches 4.5:1. It used to pick by luminance, which left white text at 2.6:1 on a light accent.
- In a window narrower than 412 pixels, the card of signing in fits the window, keeping 16 pixels on each side, instead of running past it. ([#366](https://github.com/alexkads/RavenRS/issues/366))

## [27.1.28] - 2026-11-14

### Breaking

- `SpaceError` has new variants, `Unavailable`, `Temporary` and `Unknown`; `AuthError` has `Unavailable`; `SqlError` has `Busy`. A `match` without a catch-all stops compiling.
- The backups of `ravenrs-app-sqlite` are the trait `SqliteBackups`: bring it into scope to call them.
- `ravenrs-nfe` (beta): `Signer::sign_rsa_sha1` and `Signer::sign_rsa` return `DfeError`, and `HttpRequest` has a new field, `metodo`; build one with `HttpRequest::post`. `CasoFalha::regra` is now the trait `RegraDoCaso`. The lab keys moved to `crates/ravenrs-dfe/fixtures/laboratorio`.
- `Modifiers` has a new public field, `base_key`, the letter of the key while Alt composes another character. A literal without `..Modifiers::default()` stops compiling.
- `Key` has a new variant, `Key::Function`. `Key` was already `#[non_exhaustive]`, so a `match` still compiles, but F1 to F24 no longer arrive the way they did.
- `Hotkey` can now carry Alt, through `Hotkey::alt`, and its `label` says so. Code that only builds hotkeys does not change.
- `ravenrs_graphics::barcode::Pattern` moved to `ravenrs-barcode`, and is re-exported where it was. It is no longer `#[non_exhaustive]`.
- `Item` in `ravenrs-report` has new variants, `Item::BarcodeMark` and `Item::PrinterCommand`, which print nothing. `Item` was already `#[non_exhaustive]`; an exporter of your own should pass over them.
- `Report` has new public fields, `columns`, `continuous` and `printer`, and `ControlKind` has `ControlKind::PrinterCommand`. A `Report` literal without `..Report::default()` stops compiling.
- `TemplateParts` has new fields, `notices`, `keys` and `kind`. It is `#[non_exhaustive]`, so only a template that builds one by hand is affected.
- `ravenrs-widgets` is now a facade over ten crates: `ravenrs-widgets-base`, `ravenrs-widgets-kit` and one per family (`-grid`, `-lists`, `-scheduler`, `-dock`, `-imaging`, `-charts`, `-chat`). Every name is still re-exported where it was, checked against `crates/ravenrs-widgets/public-api.txt`; code that only uses `ravenrs_widgets::` does not change. The facade no longer brings `ravenrs-app-i18n`, `ravenrs-text`, `tracing`, `serde` or `image` with it. ([#293](https://github.com/alexkads/RavenRS/issues/293))
- A `Decimal(n)` field is stored rounded to its places, half to even, and more than 15 significant digits is refused with `SpaceError::Refused`. In PostgreSQL it is `NUMERIC(18, n)` instead of `DOUBLE PRECISION`; an existing column is converted once by a migration (`decimal <table>.<column> <n>`). In SQLite it stays `REAL`, with what the column holds rounded once. `MemoryDatabase` rounds too. ([#337](https://github.com/alexkads/RavenRS/issues/337))
- `SqlBackend::column_type` returns `Cow<'static, str>`, and the trait has `decimal_upgrade`, which does nothing by default. ([#337](https://github.com/alexkads/RavenRS/issues/337))
- `ObjectSettings` in `ravenrs-app-spec` has new public fields, `empty_text` and `singleton`, and `WindowOptions` in `ravenrs-platform-native` has `follow_system_theme`. A literal of either without `..Default::default()` stops compiling.
- `ObjectSpaceExt` has a new method, `emit`. The trait is implemented for every `ObjectSpace`, so code that calls it does not change.
- `SqliteDatabase`, `SqliteBuilder` and `SqliteObjectSpace` in `ravenrs-app-sqlite` are now type aliases of `SqlDatabase`, `SqlBuilder` and `SqlObjectSpace` from `ravenrs-app-sql`, over `SqliteBackend`. Their methods are the same; a pattern or an `impl` that needs a struct of that name stops compiling.
- `Lint` in `ravenrs-lint` has a new variant for `module_cycle` before `AllowWithoutReason` and `UnknownLint`, whose discriminants moved. Code that cast a `Lint` to a number changes.
- `ravenrs-sheet`, following its engine: `Alignment` has `reading_order`, `ShapeFormat` has `text_shadow`, `Workbook` has `region`, `CsvOptions` has `max_used_cells`, `Arg` has `blank_value` and `Ctx` has `recalc_all`, so literals of them need the new fields; `ShapeKind` has a new variant, `WordArt`; and `numfmt::builtin_format` and `numfmt::builtin_id` moved to `craft::sheet`, as `builtin_format` and `builtin_format_id`.

### Added

- **`ravenrs-pdf`**, a new crate: reads PDFs, encrypted and damaged ones too, saves them, draws pages to pixels, merges and splits them and handles attachments, with patched copies of hayro, vello_cpu and lopdf as modules of its own. ([#315](https://github.com/alexkads/RavenRS/issues/315))
- **`ravenrs-word`**, a new crate: the text document model, reading and writing DOCX (with password-protected files), reading DOC, WMF/EMF, and TXT, Markdown, HTML/MHT, RTF and ODT; it builds for the web too. ([#312](https://github.com/alexkads/RavenRS/issues/312))
- **`ravenrs-app-postgrest`**, a new crate: an `ObjectSpace` over PostgREST, for your own server, Supabase and Neon's Data API. The station holds the signed-in person's token, never the database password; filters run on the server, a commit is one call to `ravenrs_commit` (installed with `SERVER_SQL`) in one transaction under the database's row level security, and a commit with no answer is reported as `Unknown` and never sent again. ([#330](https://github.com/alexkads/RavenRS/issues/330))
- `ravenrs-app-postgres` connects to managed PostgreSQL: `PostgresBackend::connect(url)` and `connect_with(url, PostgresOptions)` read the address as a panel gives it; TLS through rustls always verifies the server, with `sslrootcert` or `root_certificate_pem` for an authority of its own; `Statements::Unprepared` turns on by itself behind Supavisor, Neon's pooler and `pgbouncer=true`; a dropped connection is reopened outside a transaction, a read is retried once and a write is never sent twice. ([#324](https://github.com/alexkads/RavenRS/issues/324))
- `ravenrs-app-testkit` (not published): `contract::run` is one battery of 15 cases that every `ObjectSpace` adapter runs, with `Capabilities` for what an adapter declares it does not guarantee, reported as a limitation, never as a pass. Memory, SQLite, PostgreSQL and PostgREST run it. ([#336](https://github.com/alexkads/RavenRS/issues/336))
- `scripts/check.sh` ends with the time of each step, slowest first, the test step split into compiling and running, and the ten slowest tests; `RAVENRS_CHECK_TIMINGS=1` adds Cargo's per-crate compile report. ([#346](https://github.com/alexkads/RavenRS/issues/346))
- `scripts/check.sh` uses a compiler cache that worktrees share, so a new worktree restores what another one compiled: [Kache](https://github.com/kunobi-ninja/kache) when it is installed, or `RAVENRS_XCACHE=1` for `ravenrs-xcache`, a cache of this repository's own (not published) that takes the checkout and build paths out of what rustc compiles and says why each compilation came from the cache or did not. Measured on the full check in a new worktree: 31m04s without a cache, 11m30s with Kache, 14m15s with `ravenrs-xcache`. ([#350](https://github.com/alexkads/RavenRS/issues/350), [#352](https://github.com/alexkads/RavenRS/issues/352))
- `ravenrs-lint` has a new rule, `module_cycle`: the top-level modules of a crate that use each other in a loop of more than two are an error, reported at the `use` that closes it, with the way around. Modules in a loop can only be read, tested and moved to a crate of their own together. An app with such a loop fails the rule until it is untangled, or until `max-module-cycle` in `[package.metadata.ravenrs-lint]` holds the size it has today; once the loop shrinks, the rule asks for the lower number. `cargo ravenrs-lint --modules` shows each module's size, who uses whom and every loop; `--only <rule>` checks one rule and `--workspace` every package. The crates of RavenRS hold their loops this way while #293 and #294 untangle them. ([#292](https://github.com/alexkads/RavenRS/issues/292))
- The report designer offers the theme menu inside an application too, with `ThemePicker` and its translated words, and `Designer::theme_menu(false)` leaves it out; `AppShell::without_theme_menu` reaches it through `Session::theme_menu`. Band captions follow the theme on screen, while the page and all drawn on it stay light. **Following the system:** `SystemAppearance`, `Ui::set_system_dark`, `Ui::follow_system_theme`, `WindowOptions::follow_system_theme` and `WebOptions::follow_system_theme`; the desktop listens to the window's theme and the browser to `prefers-color-scheme`, and `ThemePicker` adds "Follow the system" (`Labels::theme_system`) where the system is known. `WindowOptions` and `WebOptions` have a new field, `follow_system_theme`. ([#283](https://github.com/alexkads/RavenRS/issues/283))
- `Dispatcher` delivers events with limited tries: each event is kept as `sending`, with a deadline (`lease`), before it is sent, so a worker that stops between the send and the answer leaves it to be checked with the destination, never sent blindly; `backoff` waits by the clock, twice as long after each temporary failure; `run_at` takes the instant, and `run_as` reads the worker's permissions again on every run and refuses one turned off or no longer granted the events. The event entity has `next_at` and `last_error`, and the action `Reprocess` (`OUTBOX_REPROCESS`) sends a failed event again under the same key, for whoever may write events. `Report` has a new field, `waiting`. `ravenrs-app-testkit` adds `Organization::add_user`, `context_of`, `turn_off`, `roles::events_worker` and `Clock::date_time`. ([#255](https://github.com/alexkads/RavenRS/issues/255))
- `ravenrs-movie-video`, a new crate: turns films into a QuickTime video (H.264 pictures, 16-bit sound at 48 kHz), the films one after the other, with their boxes, the pointer, the zooms, clicks, keys and the recorded narration. Each picture is drawn by the renderer with no window, and OpenH264 is built from its source, so nothing has to be installed. `VideoFormat` has `WIDE` (1920 × 1080), `VERTICAL` (1080 × 1920, for short vertical videos), `PORTRAIT` (1080 × 1350) and `SQUARE`, with the longest each place takes; `Video::thumbnail` gives a cover as PNG, `subtitles` the boxes as `.srt` and `chapters` the lines a video description turns into chapters.
- `MovieFrame`: one moment of a film as someone who does not stop at the steps sees it, with the mark and the box of what it explains and nothing that answers clicks. The player draws its boxes with the same code.
- `SocialFrame`: one moment of a film laid out for a tall or square screen, with the chapter on top, the film, and the title and words of what it explains under it.
- `Movie::following(zoom)`: the same film with the camera brought closer to each spot it explains.
- `Soundtrack`: the sound of a whole film as samples, with its clicks and keys and the narration of each box when it shows.
- `DataGrid`: the "Edit filter" window has Visual and Text tabs over the same filter. Text is the `ExpressionEditor` with the grid's columns as fields; the grid filters by what is written only once it reads and names known columns, and keeps the last such filter meanwhile. `Labels::grid_filter_visual` and `grid_filter_text` name the tabs.
- **API keys** in `ravenrs-app-server`: for other systems, shown once, kept as a hash, acting as a user and never more, with rotation, expiry and revoking. `security_entities()` has a new entity for them, so storage learns a table.
- **`SettingsStore`**, with `MemorySettings`, `RecordSettings` and `settings_entity`: JSON under a name, for everyone or for one person, reached at `/api/settings/{name}` when declared. `App::settings` keeps the theme someone chose, also over a server with `RemoteSettings`.
- **`ravenrs-app-remote`:** the screens over the web API, on the desktop and in the browser: a copy in memory of what the person can see, a way in with `RemoteApp`, and commits that go as one batch. `ApiOptions::allow_origin` lets a page on another origin call the API. `MemoryDatabase::replace_records`, `value_to_json` and `object_from_json` in `ravenrs-app`.
- **`ravenrs-app-postgres`:** records in PostgreSQL with the same builder as SQLite, meeting the column types of tables that came first. What SQL databases share moved to **`ravenrs-app-sql`**, whose `SqlBackend` is how another database comes in. `SqliteDatabase`, `SqliteBuilder` and `SqliteObjectSpace` are now names for its generic types.
- **`ravenrs-app-server`:** a web API over an application's model, with sign-in by token, protected CRUD, filters in the criteria language, pages, deep updates of aggregated collections, batches that stay or go together, endpoints of the application's own and an OpenAPI description of what each person can reach. Reads and writes go through the same permissions, rules, states and audit trail as the screens.
- A shared canonical policy for Pages and Vercel, crawlable canvas content, static
  playground examples, social PNG cards, complete sitemaps and a local SEO build gate.
  ([#156](https://github.com/alexkads/RavenRS/issues/156))
- `EntityInfo::empty_text` and `#[object(empty_text = "...")]`: what a generated list says with no
  records, kept on the list view as `EmptyText`. Without it a grid with no rows says
  `widgets.grid_no_records` ("Nada por aqui ainda.") and keeps "no rows match the filters" for
  when its filters hid them. ([#233](https://github.com/alexkads/RavenRS/issues/233))
- `ravenrs-app-sqlite` backups: `SqliteDatabase::backups(BackupStamp)` saves a checked, consistent
  copy while the database is in use, stamped with the application, tenant and migrations;
  `restore` refuses another application, tenant or migrations, keeps a copy first and replaces
  everything in one transaction or nothing; `keep_copy` keeps copies in a folder with a retention.
  ([#229](https://github.com/alexkads/RavenRS/issues/229))
- The screen of signing in of `SecuredApp` takes `description`, `notice`, buttons of its own with
  `sign_in_action(SignInAction)`, such as going back to choosing the organization, which forget
  what was typed and leave whoever was halfway in behind, and `demo_credentials`, which fills in a
  demonstration's name and password and still checks them on Sign in.
  ([#191](https://github.com/alexkads/RavenRS/issues/191))
- `GridCurrent` and `DataGrid::current` give what a grid shows now, its query and its rows as a
  table, to something outside it. ([#181](https://github.com/alexkads/RavenRS/issues/181))
- `FlowCanvas`: diagrams of nodes joined by connections on an endless surface, with pan,
  zoom, nodes that move on the grid and take focus, ports that check what connects and say
  why not, curve, step and straight lines with labels and travelling dashes, selection by
  press, Shift and rectangle, a lock, zoom controls and a map. The model and the routes of
  the lines live in `ravenrs_graphics::flow`, and `BuildCx::on_wheel` lets a surface take the
  wheel before any scroll container around it. ([#195](https://github.com/alexkads/RavenRS/issues/195))
- Permissions on records can go through references, such as
  `[class_id.teacher_id.login] = ?CurrentUserName`, and hold for records being created, changed
  and read: `SecuredObjectSpace` follows them in storage, and a value put on the record under the
  path is replaced and never kept. ([#232](https://github.com/alexkads/RavenRS/issues/232))
- `ravenrs-app` outbox: `ObjectSpaceExt::emit` keeps an `Event` with the record in the same commit, and `OutboxModule` and `Dispatcher` hand what is pending to an `EventSink`.
- `Provider`, `EventSink` and `CAN_TELL_STATUS`: what a module needs from outside, said once and checked by contract tests.
- `ModuleManifest`, with `switch_off` and `Application::guard`: a module says its version, what it accepts of its dependencies, where it runs and what it needs from the host, and a module switched off keeps its records read only.
- `ExternalSignIn`, `IdentityProvider` and `Claims`, with `link_external` and `unlink_external`: signing in through another system, tied to a user of the application.
- `ravenrs-app-testkit` and `ravenrs-app-requests` (not published): contract tests for reusable modules and a module of requests built on them.
- `ravenrs-mcp`: `app_contracts`, and `parity_status` with `document: "application"`. `docs/APP-CONTRACTS.md` lists the scenarios and groups of capabilities a reusable module honors.
- **Beta, proof of concept:** `ravenrs-nfe`, NF-e and NFC-e with no GUI, and `SefazSandbox`, a local SEFAZ in the style of MSW that replaces the `Transport` and checks what arrives. It is not homologated and has never talked to a real SEFAZ. `Ambiente` has no production variant, and an emitter only exists with `BetaAck`.
- **Beta, proof of concept:** `ravenrs-app-nfe`, the module with the issuer, the documents issued, consulting, canceling and the receipt as a stored report. A notice that cannot be dismissed says it is a beta.
- **Beta, proof of concept:** `ravenrs-dfe`, what the fiscal engines share: XML, C14N, enveloped XMLDSig with `Algoritmo::NFE` (SHA-1) or `Algoritmo::SHA256`, `Transport` with `Metodo` (POST, GET, HEAD), the MSW-style `Cadeia` of handlers with `Rota`, GZip + Base64 with a limit, and the laboratory of ICP-Brasil test certificates. `ravenrs-nfe` re-exports what moved, with the same paths.
- **Beta, proof of concept:** `ravenrs-nfse`, NFS-e of the national standard: the DPS, `POST /nfse`, recovery of a lost answer by the Id of the DPS, query, cancellation and the agreement of the city, with `SefinSandbox`. Never sent to the Sefin; see its README for what is not confirmed.
- **Beta, proof of concept:** NFS-e from an application: `NfeService::emitir_nfse`, `reenviar_nfse` and `cancelar_nfse` in `ravenrs-app-nfe`, with the record `NfseDocumento`, kept before the DPS goes so that one without a sure answer goes again with the same number. `ravenrs-app-pos` gains the services (`PosServico`) and "Vender serviço" (F8).
- **Beta, proof of concept:** the DANFSe A4 as a stored report in `ravenrs-app-nfe`, with the QR Code and the notice of no legal validity the NT 008 asks.
- **Beta, proof of concept:** `ravenrs-pagamento`, electronic payments with a journal, a provider simulator, the ABECS protocol and IntPos files. Never tested against a real provider, pinpad or card.
- **Beta, proof of concept:** `ravenrs-app-pos` and `examples/mini-pdv`, a checkout used from the keyboard and a scanner, with shelf labels and receipts made in the standard report designer. `ravenrs-pos` has its rules, with no screens.
- **Beta, proof of concept:** `ravenrs-perifericos`, printers, label printers, pinpads and scanners described as data. See `docs/PERIFERICOS.md`.
- `docs/AUTOMACAO-COMERCIAL.md`, in Portuguese: what exists for a commercial automation product, how each part was checked and what is missing to sell it.
- `ravenrs-report-label`: a report on label and receipt printers, as a 1-bit bitmap in ZPL II, EPL2/PPLB, PPLA, TSPL or ESC/POS, sent over TCP 9100, CUPS, a Windows share or a file. Output was read back by a parser and by a separate barcode decoder. Nothing has been printed on a physical printer.
- `ravenrs-barcode`: barcodes as plain data, with GTIN checks and ITF-14, out of `ravenrs-graphics`, which re-exports them.
- Reports: `Report::columns` for sheets of labels, `Report::continuous` for one long page on a roll, `Report::printer` for where it prints, `ControlKind::PrinterCommand` for cutting, opening the drawer, beeping and feeding at a place on the page, and `labels` with common rolls, sheets and a first shelf label.
- Report designer: paper and columns in the report's panel, and "Impressora direta", with a preview in dots and printing straight to a printer. It is the one editor of layouts, receipts and labels included.
- `KeyBar` and `KeyBarItem`: the keys of a screen always in sight, working wherever the focus is. `ScanField` and `Scan`: a field for a barcode scanner that keeps the focus, tells a scanner from fingers by the pace and empties after each code.
- `Module::notices`, `Notice` and `NoticeKind`: what a module keeps on top of every window, and no template can leave out.
- `Session::run_background`, `cancel_background`, `background_jobs`, `JobProgress`: work out of the way of the screens, with progress and canceling, and its result on the screens' thread. On the web it runs in line.
- `EntityInfo::singleton` and `#[object(singleton)]`: an entity with one record, which opens on its form.
- `AppRecordsModule`, `next_number`, `save_draft`, `drafts`, `take_draft`: numbers with no gaps within a unit of work, and work kept to pick up later.
- `CounterTemplate` and `App::counter`: a window for use at a counter, with a bar of keys made from the actions. `App::theme`: an application that starts on its own theme. `Session::show_shortcuts` and `Session::lock`.
- Actions take `f1` to `f24` and `alt+letter` as keys. The field editor `Gtin` checks the digit while typing, and `rust_decimal::Decimal` works as a field type, exact up to 6 places.
- `render_to_rgba` in `ravenrs-render`: a screen as pixels, with no window.
- `ravenrs-mcp`: the tools `search_api`, for items of any crate by words, and `guides`, for the repository's documents. `parity_status` also reads `docs/APP-PARITY.md`, and `component_examples` also reads the guides.
- `feature-center`: a counter window with its own notice, keys, numbers, drafts, work in the background and lock, a single record, a GTIN editor, a sheet of labels and a receipt on a roll. Run it alone with `cargo run -p feature-center -- caixa`.

### Changed

- `ravenrs-sheet` follows GridCraft at `c0ee120`: work limits per formula and per function, results closer to Excel's (implicit `@` in legacy formulas, `LINEST`, `REGEXREPLACE`, `INDEX`), faster structural edits, Arabic sorting, bidi and AutoFit, the Hijri calendar and regional formats for 20 regions. Its file formats are read through the craft-libs crates, brought in as modules. ([#311](https://github.com/alexkads/RavenRS/issues/311))
- The quick `scripts/check.sh` runs the tests on `cargo-nextest`, which every contributor installs once, and reads `Cargo.toml` and `Cargo.lock` entry by entry, so a new crate or a version bump is no longer the whole workspace. ([#280](https://github.com/alexkads/RavenRS/issues/280))
- Dependencies updated within their current requirements, among them `wgpu`/`naga` 30.0.2, `rustls-webpki` 0.103.17, `webpki-roots` 1.0.12, `zip` 9.5.1 and `ureq` 3.5.2. ([#344](https://github.com/alexkads/RavenRS/issues/344))
- The integration tests of each package are one binary, `tests/suite/main.rs`, with each file a module, so they link once: building the tests of `ravenrs-widgets` went from 30 s to 18 s, and from 12.7 s to 5.5 s after a change. Commands that named a file, such as `cargo test -p ravenrs --test visual`, are now `cargo test -p ravenrs --test suite visual::`. `scripts/check.sh` fails when a file in `tests/suite/` is not declared. ([#291](https://github.com/alexkads/RavenRS/issues/291))

### Fixed

- A remote grid with no rows said it had a chunk's worth: an empty chunk past the end stretched
  its total. It now ends where the earliest end says, and shows what it says when empty.
  ([#233](https://github.com/alexkads/RavenRS/issues/233))
- The report of a list (`ReportModule`, `Session::list_table`) has the rows the list shows: its
  search, column filters, sorting, columns and grouping, and a remote list asks storage for all of
  them. ([#181](https://github.com/alexkads/RavenRS/issues/181))
- `AppShell` in a window narrower than 720 points shows a button in the header that opens the
  navigation sheet; choosing a screen closes it. `AppShell::sidebar` shares the state with the
  application. ([#184](https://github.com/alexkads/RavenRS/issues/184))
- Dates and moments in generated lists show the language's way (`format.date`,
  `format.date_time`: `dd/MM/yyyy` in Brazilian Portuguese) and still sort and filter as dates;
  a column with a format of its own filters a number by what it shows when it does not compare.
  ([#231](https://github.com/alexkads/RavenRS/issues/231))
- Closing a `Dialog` whose field took the focus gives it back to where it was.

## [27.1.27] - 2026-11-11

### Added

- Interactive project creation with arrow-key target/template selection, contextual descriptions,
  RavenRS branding, asynchronous build progress, saved diagnostic logs and `--verbose`. ([#213](https://github.com/alexkads/RavenRS/issues/213))
- npm discovery metadata, complete installation reference, AI development guide, llms.txt,
  llms-full.txt and structured documentation metadata on Pages and Vercel. ([#211](https://github.com/alexkads/RavenRS/issues/211))

### Fixed

- Repeating creation preserves the original template/target when choices are omitted and refuses
  incompatible choices before changing files or installing prerequisites. All four templates are
  checked on desktop, web and both targets, with native screen tests. ([#215](https://github.com/alexkads/RavenRS/issues/215))

### Changed

- Publish all framework crates, CLI, plugin, npm and documentation together as 27.1.27.
  npm and CLI patches published since 27.1.23 are included in this complete release. ([#218](https://github.com/alexkads/RavenRS/issues/218))

## [27.1.23] - 2026-11-11

### Security

- Upgrade the MCP SDK to rmcp 3.9.0, fixing RUSTSEC-2026-0462 (unbounded HTTP client response buffering). RavenRS uses its stdio server transport.

### Added

- A documentation portal at `/docs/`, generated locally from repository Markdown: navigation by topic, full-text search with Ctrl/Cmd+K, on-page index, highlighted copyable code, light/dark themes and mobile navigation. The same guide ships on GitHub Pages and Vercel; installation remains documented in `docs/GETTING_STARTED.md`.

- `cargo-ravenrs`: create hello, app, SQLite and report projects for desktop and web, with version-matched AI instructions, skill and MCP configuration. Includes `new`, `ai`, `mcp` and `doctor`. `npx create-ravenrs` prepares Rust and the matching CLI before generating the project, with consent for installations and repeatable setup. ([#205](https://github.com/alexkads/RavenRS/issues/205), [#206](https://github.com/alexkads/RavenRS/pull/206), [#207](https://github.com/alexkads/RavenRS/pull/207))

- `Chart::animate_changes`: when the data changes to as many series and categories, the chart goes from its old shape to the new one in 0.75 s, quick to start and settling softly: lines and bars rise or fall, slices turn and grow, and the value axis slides along instead of jumping between round numbers. A change in the middle starts from where the chart is; under reduced motion it changes at once. `ravenrs_graphics::plot::value_range` gives the value axis a chart would pick for some data, for such in-between frames.
- `CountUp` also counts when its value changes, from what it shows to the new value, even halfway through a count, and `CountUp::start_at_value` leaves out the first count from zero. Its digits keep the width of the widest one, so a number does not shake as it counts.
- `BuildCx::animated_numbers`: a list of numbers that tweens from where it is to a new target in full `f64` precision, picking up from where it is when the target changes again, and at once under reduced motion.
- Raven Finanças: on another account, the dashboard's figures count to their new values and its balance, income and expense and category charts move to their new shapes.

### Fixed

- `#[ravenrs_live::expose]` preserves conditional compilation on methods, constants and module functions. The `live` feature can be used without image decoding, including in `ravenrs-movie`.

- `TileLayout` keeps the same 16 px margin around what a tile holds, on both sides of the header too, and its header is tall enough for a small control (a `Select`, a button) to keep 8 px above and below it. The corner that resizes is two diagonal strokes in that margin, along the curve of the edge, instead of a few dots over the content: faint while the pointer is over the tile, plain under the pointer and while it drags. A tile's content is 12 px shorter and 8 px narrower.
- A chart with `tight_scale` keeps a little room above its peak and below its lowest point: the scale ends there instead of at the next round number, so a peak no longer runs along the top edge of the plot.

## [27.1.22] - 2026-11-08

### Added

- `Tile::accessory`: something at the end of a tile's header, after its title, such as a help button, a badge or a choice that changes what the tile shows. Raven Finanças' dashboard is now a `TileLayout` with it.
- `ravenrs-lint`: lints for applications built with RavenRS, reported like the compiler's, from `cargo ravenrs-lint` or from `cargo test` with `ravenrs_lint::assert_clean()`. It reads the syntax tree and denies literal colors and font sizes, spacing off the 4px grid, captions without a verb, icon buttons without a name, more than one primary button, `State::new` and blocking work in `body` or `build`, and files over 600 lines. Exceptions need a written reason, in a comment or in `[package.metadata.ravenrs-lint]`. `--init` writes the strictest rustc and Clippy levels, a `clippy.toml`, the test and the same instructions for every coding agent (`AGENTS.md`, `CLAUDE.md`, `GEMINI.md`, `.github/copilot-instructions.md`); `--gate` runs formatting, Clippy, the rules and the tests.

## [27.1.21] - 2026-11-08

### Breaking

- `MotionTokens` has a `draw_in` field (the `duration-draw` token, `Transition::draw_in`): code that builds it field by field adds `draw_in: Transition::draw_in()`.

### Added

- `Playground` goes full screen: a button beside Run (in the ⋯ menu below 768 px) puts it over the whole screen and brings it back, and the component pages allow it. Full screen is a service the platform installs, as links are: `FullScreen` and `FullScreenProvider`, through `BuildCx::full_screen`, `Ui::full_screen` and `Ui::set_full_screen`. The browser uses the page's full screen and hides the button where the frame does not allow it; the desktop turns the window into a borderless full screen.
- `Playground` has a new layout: one bar (examples, the app's actions, Restore, Download project, Run), the code beside the result, and an **Output** panel under the result with Console, what the code prints, and Problems, where it went wrong, including closures that fail later on a click. Picking an example runs it. The last result that worked fades while the code has problems. Below 768 px: Code, Result and Output tabs, the other actions in a ⋯ menu and Run at the bottom. `Runner::take_output` and `Runner::watch` carry what a runner hears, and `LiveRunner` hears `println!` and its family through `ravenrs_live::printed` and `on_output`. `ViewExt::opacity` (`Faded`) draws any view faded, and `DropdownMenu::icon_only` shows only its icon.
- A chart's data draws in the first time it is painted, over a second that starts fast and settles softly, with its axes, grid and their labels there from the first frame: lines, areas, dots, candles and the flows of a `SankeyChart` come in from the left, bars grow out of the zero line together, a pie or a doughnut sweeps clockwise with its labels and guides coming in at the end, and a radar grows out of its center over rings that stay put. The tooltip waits until the data is whole, and with less motion asked for it is whole at once. `Chart::no_appear` and `SankeyChart::no_appear` turn it off, and `appear_key` draws it in again when its key changes. It runs on the GPU over the pictures a chart already keeps, so it draws none anew.
- `BuildCx::appear`: how far a view has appeared since it was first built, for content that draws itself in.
- `ravenrs-graphics`: a `Plot` says which layers are its `backdrop` and its `notes`, which labels are `data_labels`, where bars start (`baseline`), and its `uncut_rules`, which `rules_around_bars` cuts again for bars of another length.
- The site also publishes to Vercel: `scripts/deploy-vercel.sh` (`--prod` for the production address) sends the same files `scripts/deploy-pages.sh` sends to GitHub Pages, both built here by `scripts/build-site.sh`. Vercel builds nothing.
- The site has a playground at `/playground/`: every example in one place, picked from its bar, edited, run and downloaded as a Cargo project. `?example=Slider` opens on one. Each component page links to it, and so does the home page.
- `ravenrs-project`: code of the playground becomes a Cargo project, `Cargo.toml` on the published crates, `src/lib.rs` with the code and the browser's entry, `src/main.rs`, `web/index.html`, a README and the records as `src/seed.json`, as files or one `.zip`. `Playground::download` adds the button that saves it, with the system's dialog or as a download, and marks in the code why it cannot. The gallery's playground downloads every example this way.
- Actions declared as data. `#[action(caption, set = "state = 'done'; finished = Today()", enabled, selection, confirm, id)]` on a business object, or `EntityInfo::action(RecordAction::new(..))`, sets fields of the records selected from expressions, checks the rules and keeps all of them or none. `Action::assign` does the same for any action, and `Criteria::assignments` reads `field = expression; ...`.
- `ravenrs-live`, an interpreter of the Rust small examples are written in: `let`, closures, chains of builders, tuples, `vec!`, ranges and iterators, `format!`, `if`, `match`, `for` and `State`, with every mistake placed by line and column. `#[ravenrs_live::expose]` (`ravenrs-live-derive`) registers what an `impl` block or enum offers, and `scripts/gen-live.py` puts it on the components and the types under them behind a `live` feature. `LiveRunner` (in `ravenrs-widgets` and `ravenrs`, feature `live`) runs a `Playground`'s code with it, so the component pages run edited examples: all 181. It also reads `Result` and `?`, `let .. else`, the program's own structs, `impl` blocks and `impl View for ..` with a `cx` that keeps its states, struct literals and fields of the host's structs, closures that get references or give host values, functions named where a closure goes, float ranges and `Binding::map`. `ravenrs-movie`, `ravenrs-app` and `ravenrs-app-ui` have a `live` feature too, with `live_expose`.
- `ravenrs-app-spec`: what `#[derive(BusinessObject)]` and `#[derive(BusinessEnum)]` read from a struct or an enum, as data, now shared by the derive. With the new `source` feature, `ravenrs_app::entities_from_source` reads the same structs from Rust text while the program runs and gives the `EntityInfo`s the derive would, refusing the same mistakes with the same words and each `SourceError`'s line and column.
- Records and settings that stay between runs. `App::keep_in(storage)` keeps them in a `Storage` (new in `ravenrs-core`): `FileStorage` on the desktop, `IndexedDbStorage` on the web (`ravenrs::web`), `MemoryStorage` for tests; the `seed` runs only while nothing was kept. Underneath, `MemoryDatabase::stored_in` reads what was kept and writes after every commit, `export_json` and `import_json` move all records out and in, all or nothing, and `StoredModel` keeps the person's settings. `examples/quick-app` keeps its records in the browser.
- `CodeEditor` and `Playground`, behind the new `code` feature of `ravenrs-widgets` and `ravenrs`. `CodeEditor` edits source code in Fira Mono (embedded by the new `mono` feature of `ravenrs-text`), with line numbers, Rust colors by token, Tab and Shift+Tab indenting, Enter keeping the indentation, and `CodeDiagnostic`s marking their lines and columns. `Playground` puts code beside what it makes: examples, Restore, Ctrl/Cmd+Enter running it through a `Runner`, problems listed and marked, the last result that worked kept, and tabs below 768 pixels. The component pages of the site open each example in one.

### Fixed

- `CodeEditor`'s line numbers keep their background down to the bottom of the editor, even when the code ends higher, and no longer cover the editor's border.
- An optimized build with the `live` feature never ended: `#[ravenrs_live::expose]` wrote `Name { .. }` for a struct as one expression with every field, and for `Labels`, with hundreds, the compiler worked on it for hours. A struct with more than 32 fields is now built from `..base`, or from its `Default` when every field is given, one field at a time. `ravenrs-widgets` with `live` builds in release in about a minute and a half.
- On a component's page the playground fits its box: every line of the code and the status under it show, the code gets the wider side (the result starts at 40%), and an example taller than the room scrolls in the result instead of pushing the rest out. `Playground::output_open(false)` starts the output folded, as the component pages do; a problem opens it. The page's code block shows the `use` lines the playground shows.
- `CodeEditor` scrolls sideways: a line longer than the editor no longer gets cut. The wheel or trackpad moves it, the caret brings its column into view as it moves or types, and the line numbers stay put over the code that passes under them. Shift with a wheel that only goes up and down scrolls sideways on the desktop, as browsers do.
- An `Action` changed through a copy, as the playground's interpreter makes one, kept none of the change: `selection`, `on_execute` and the rest were lost without a word. The change now goes to an action of its own. `ActionContext` copies too, so a closure written in the playground gets it instead of `()`.

## [27.1.20] - 2026-11-07

### Breaking

- `WebOptions` has a `backend` field: code that builds it field by field adds `backend: WebBackend::Auto`. A browser without WebGPU no longer stops at `WebError::WebGpuUnavailable` unless `WebBackend::WebGpu` is asked for; it opens with WebGL2.

### Added

- Showcase pages. `Link` opens an address through the new `Links` service (`BuildCx::links`, `Ui::set_links`): the browser navigates, the desktop asks the system's browser. A `#name` address glides the page to the `Anchor` of that name, landing below a pinned bar (`BuildCx::anchor`), and jumps under reduced motion. `CountUp` counts up to its number the first time it shows on screen, through `BuildCx::on_appear`, which calls a handler once when an element first shows. `Ticker` keeps a strip of content moving sideways through `BuildCx::time_linked` and `TimeLink`, an effect of the clock worked out when painting, without rebuilding; it stands still under reduced motion.
- `QuadField` draws a plane of squares that waves like a floor, lifts under the pointer and, as the scroll goes through its scene, stands up into the picture a function paints, one color per square, on every frame without rebuilding. `BuildCx::scroll_progress` gives drawing like this the scroll progress along a `ScrollRange` before each paint.
- `PageFan` in `ravenrs-report-viewer` spreads real pages of rendered reports like a hand of cards: they open as the fan scrolls in, the page under the pointer or keyboard focus comes forward with an accent ring while the others step back, and a click opens its address. Each page is painted once and reused.
- On the web, a touch scroll keeps gliding after the finger lifts and slows down as on a phone; a finger on the page stops it.
- `examples/site-home`: the project's home page as a RavenRS app, drawn with WebGL2 in every browser and phone. The floor of squares that stands up into an app screen, the six steps held by the scroll, live components in moving strips, counters, real report pages in a fan, the platforms and themes switching the whole page, and the command palette are all library components. `scripts/deploy-pages.sh` puts it at the root of the site, in place of the HTML page.
- The web backend also draws with WebGL2, so RavenRS opens in browsers and phones without WebGPU. `WebBackend::Auto`, the default, uses WebGPU where it is and WebGL2 otherwise or if WebGPU fails to start; `WebOptions::backend` asks for `WebBackend::WebGpu` or `WebBackend::WebGl2`. A WebGL2 device asks only for WebGL2's limits. The WebAssembly grows by about 0.7 MB compressed.
- `ravenrs-spreadsheet`: the `Spreadsheet` component and `SpreadsheetState`. It opens XLSX, XLS, ODS and CSV and saves XLSX, with formulas that recalculate, editing in the cell and in the formula bar, selection with the pointer and the keys, the clipboard, undo, bold, italic, underline, alignment, number formats, column widths and sheet tabs, on the desktop and the web. `examples/spreadsheet` shows a household budget in it.
- `ravenrs-sheet`: the spreadsheet engine underneath, from GridCraft (MIT OR Apache-2.0): 500+ functions with dynamic arrays, incremental recalculation, number formats, XLSX and CSV reading and writing, charts, and sessions in which every action is a command with undo. It runs in WebAssembly too.
- Scroll-driven pages. `Sticky` keeps its content on screen while its section scrolls past, like CSS `position: sticky`, and clicks, focus and screen readers find it where it shows. `ScrollScene` holds its content for a length of scrolling, the stage for steps that change as the reader scrolls. `ScrollLinked` paints its content with the `Effect` a `ScrollLink` gives for the scroll position: a `ScrollRange` (the scene, the pass across the view, entering, leaving), a window with `between`, an `Easing`, and keyframes or any function, with `fade_in`, `fade_out`, `parallax` and `grow_from` ready. Scrolling still neither rebuilds nor lays out again. With reduced motion, linked effects only fade. The same powers are on `BuildCx`: `pin`, `scroll_track` and `scroll_linked`.
- `Effect::lerp` interpolates between two effects and `Effect::then` combines them.

### Fixed

- After the graphics device was lost, the web backend started WebGPU again even on a page that ran on WebGL2; it now starts the API the page was using.
- `ravenrs::web` exports `WebBackend`, which the WebGL2 release added to `ravenrs-platform-web` but left out of the re-export.


## [27.1.19] - 2026-11-07

### Breaking

- `ClipKind::Title` has a `look` field: code that builds it field by field adds `look: CardLook::Plain`. Films saved before read as `Plain`.

### Added

- `ravenrs-movie` opens and closes films with moving cards: `CardLook::Opening` and `CardLook::Closing`, put in with `Project::insert_card`, show the app's animated logo playing in the time of the film (it follows a pause or a jump), then the title, a line and the words under it coming in. `Composition::logo`, `MoviePlayer::logo` and `MovieAssistant::logo` take the logo, a `LottieAnimation`. The player says the words of a card once they are in, only from a recording, and `Movie::poster_ms` shows an opening with its words.
- `Recorder::narration`: with the recordings the player will use, each box of a recorded film lasts as long as its recording and a breath, instead of a time guessed from its letters. `Narration::seconds` says how long a recording lasts without decoding it, and `Narration::pack_onto` adds recordings to a packed file, keeping the ones already there byte for byte.

### Fixed

- A recorded film no longer cuts its voice: a box that lasted less than its recording was replaced, mid-sentence, by the next one, and a film that ended on such a box stopped the voice at its end.

## [27.1.18] - 2026-11-07

### Breaking

- `SpeechSettings` has a `speaker` field: code that builds it field by field adds `speaker: None`.

### Added

- `ravenrs-ai` speaks with Kokoro 82M, in Rust: `VOICE_CATALOG` offers it first, with the Brazilian voices `pf_dora`, `pm_alex` and `pm_santa` (`KOKORO_SPEAKERS`). `SpeechSettings::speaker` picks one, `SpeechEngine::pieces` splits a text as each model says it best, and `VoiceManager` shows the three voices when Kokoro is in use. The phonemes come from the crate's own rules for Brazilian Portuguese (stress, open and nasal vowels, how a word changes before the next), without espeak-ng. `ModelFamily::Kokoro` is new.

## [27.1.17] - 2026-11-07

### Breaking

- `Labels::tag_add`, `wizard_progress`, `theme_changed`, `theme_contrast`, `short_date`, `lightbox_counter` and `rating_value` are templates now, with `{0}`, `{1}` where the values go, and no longer functions. Methods of the same name fill them in: `labels.short_date(day)` instead of `(labels.short_date)(day)`. `theme_changed` has three forms, for none, one and more.

### Added

- **`ravenrs-app-i18n`**: `Language`, `Catalog` and `Texts`. Every word of the application framework and of the components has a key, in English and Brazilian Portuguese, in JSON by part; a language without a key falls back to English. `App::texts` and `SecuredApp::texts` add or replace words with a flat JSON of keys, and `Session::texts` reads them, keys of the application's own included. `Words`, `RuleWords`, `ReportWords`, `SqlWords`, `Labels`, `ModelPickerLabels`, `ViewerLabels`, `ParameterLabels`, the labels of `ravenrs-movie` and of `ravenrs-ai` have `from_texts`, and their `default()` and `pt_br()` come from the catalog. `ravenrs_app_ui::Language` is the one of this crate, and the translations of the model that come with the framework are in `ravenrs_app_i18n::model`.

- `ExpressionEditor`: edits an expression of the criteria language, with fields, parameters, functions by group, operators and constants to search and insert at the caret, each item's signature and description, and a check while typing that gives the line and column of a syntax error and lists unknown names. Its words are in `Labels` (`expression_*`). The report designer opens it from the button beside an expression, the filter and a group's field, and from Expression on the Home page.
- `ravenrs-mcp` creates report layouts: `report_layout_format` describes the `.rvreport.json` format with examples made by `ravenrs-report`, `create_report_layout` builds a table layout from sample rows or from fields and a data URL, `check_report_layout` reports JSON errors by line and column, checks every expression and renders the layout, and `report_layout_templates` returns the ready layouts. The server still has no side effects: it returns JSON for the assistant to save.

- `ravenrs-app-a11y`: the accessibility of business applications in one crate. `AccessibleText` names fields and actions from the model, with the caption as default (`FieldInfo::accessible_name` / `accessible_description`, `Action::accessible_name` / `accessible_description`, `#[field(accessible_name, accessible_description)]`, copied to the form's items as `AccessibleName` and `AccessibleDescription`). `Announcer` and `LiveRegion` tell screen readers what changed, as a status or an alert. `AccessWords` holds their words under `a11y.*` keys. `Audit` walks an application with no window and reports unnamed controls, repeated ids, focus that is stuck or trapped, windows that Escape does not close, shared shortcuts and theme contrast under WCAG AA. `quick-app`, `quick-plan` and `feature-center` run it in their tests.
- `EntityInfo::focus_order` and `#[object(focus_order = "...")]`: fields that come first on the form, on the screen and for Tab.
- `ravenrs-app-ui` announces saving, a save that fails its rules, deleting and the list that opened, through `Session::announcer`, and names fields and commands from the model.
- `FormField::accessible_name` / `accessible_description`, `AccessibleControl`, `Button::accessible_description`, `IconButton::description`, `ToolbarAction::accessible_name` / `accessible_description` (a command with keys says them), and `Ui::shortcuts`.
- Background actions: `Action::background` and `in_background` run the work on a thread of its own, with a `JobContext` that reports progress, checks `cancelled`, and saves, deletes or tells. The `Frame` applies what comes back to the `ObjectSpace` on the UI thread, commits it and rolls back a batch that fails; the action stays off (`REASON_RUNNING`) while it runs, and an error or a panic becomes a message. `App::action` declares an action with no module or controller. The generated window shows a progress bar and Cancel at the foot of the navigation.
- `ConfirmDialog::ask_typing`: the button acts only after the exact word is typed, capitals included; Enter in the field confirms too. Its words are `Labels::confirm_by_typing`.

### Changed

- `DataGrid`: a column's filter by value lists only the values of the rows the other filters and the search leave, so it never offers one that would show nothing. Its own filter does not narrow it, and a value already checked stays to be unchecked. Rows from `DataGrid::remote` are listed as they come. The list's search says "Search values" (`Labels::grid_filter_search`).
- The report designer is laid out as a full report editor: a toolbox down the left, rulers in centimeters across the page and beside each band (marking the selected control), the field list and report explorer as tabs over the properties on the right with a bar to drag between them, Design / Preview / JSON tabs under the document and a zoom slider in the status bar. The Ribbon's Home page has Edit (cut, copy, paste, delete), Font (bold, larger, smaller), Alignment and Layout (bring to front, send to back); printing and export moved to an Output page and stay in the File menu and the quick access bar.

## [27.1.16] - 2026-11-06

### Breaking

- `FilterField` has a new public field, `values`; build it with `FilterField::new` instead of a struct literal.
- `ReleaseNote` is no longer `Send`, `Sync` or `UnwindSafe`, since `links` holds what each click does. Like every other view, build it on the UI thread.

### Added

- `DataGrid`: the filter panel has an "Edit filter" button that opens the `FilterBuilder` over every column's expression at once. It is there whenever something is filtered, with or without `filter_builder(true)`.
- `FilterBuilder`: "is any of" and "is none of" (`In` and `Not ... In`), picked as tags. A filter by value from a column's menu now opens as a line instead of raw text. `FilterField::values` lists what to pick from; the grid fills it with the column's values.
- `ReleaseNote::links(heading, links)`: a heading over links, each a label and what clicking it does, such as opening the issue a change came from.
- `ravenrs-ai` hears: `HEARING_CATALOG` offers Whisper Base and Small, and `Transcriber` turns mono speech at `HEARING_RATE` (16 kHz) into text on the CPU, in plain Rust, the way OpenAI's reference reads it. `ModelManager` downloads them like the other models, and its speed test times ten seconds of hearing.
- `ModelFamily::Whisper`.

### Fixed

- Icons sit at the center of their box: the Lucide font left every glyph's left side bearing at 0, so each one was drawn left of center (the checkbox check by 1.75 px at 14 px). `RavenIcons.ttf` and `scripts/gen-icons.py` now set it to the glyph's own left edge.

## [27.1.15] - 2026-11-06

### Breaking

- `ThemeSwitcher` is `Clone` but no longer `Copy`, `Send` or `Sync`, since it can hold a callback. Like every other view, build it on the UI thread.

### Added

- `ThemeSwitcher::on_change(action)`: runs with the theme the user picks, so the app can remember it for the next launch.

## [27.1.14] - 2026-11-05

### Breaking

- `ThemeSwitcher` is no longer a unit struct. Write `ThemeSwitcher::new()` where you wrote `ThemeSwitcher`.

### Added

- `ThemeSwitcher::themes(dark, light, glass)`: an app offers its own three themes in the switcher instead of the standard ones.

## [27.1.13] - 2026-11-05

### Breaking

- `ProgressBar` is no longer `Copy`, because it now holds a label and a detail. Clone it where you copied it.
- `Breadcrumb` and `Section` are no longer `Send`, `Sync`, `UnwindSafe` or `RefUnwindSafe`, because they now hold closures and views. Views were already built and kept on one thread.

### Added

- `DockArea` and `DockLayout`: panels in a tree of splits and groups of tabs at the left, the right, the bottom and the center. A tab goes to another group, to one side of it or to a border with a drag and a mark on the target, or by Alt and Shift with an arrow and the "Move to…" of its menu; it reorders with a drag or Alt and an arrow. Bars resize within limits by pointer and keys, docks collapse from the strips on their edges, and panels close and come back where they were. The arrangement holds ids alone, and what a panel shows comes from a function of the app. `DockLayout::to_text` and `parse` save it as versioned JSON: an unknown id is left out, and a text that cannot be read is a clear error.
- `ChatMessage::media` and `ChatMedia`: a photo that holds its place until it arrives, a document as a card or a recording with `AudioPlayerButton`, in the bubble, above the text. `ChatMessage::formatted` reads the text as a `RichText`, and `Chat::on_link` gets its links. Messages that ask for neither stay as they were.
- `monotone_slopes` in `ravenrs-core`: the slopes of a curve that never swings past its points, which charts and `CurveEditor` now share.
- `SelectionBar`: how many things are chosen, a way out by a button or Escape, and actions over a `Toolbar`, which go to a menu when they do not fit and are disabled while nothing is chosen.
- `AudioPlayerButton`: plays a recording and stops it over `Sound::speak_clip`, with progress drawn without rebuilding, audio the app can deliver after the press, and a transcript to open or ask for. Another recording sends it back to rest.
- `AttachmentStrip`: the files about to go with a message, as rows with an icon or thumbnail, a name that gives way, the size, a menu such as "Send as" when there is a choice, and a button to take each one out.
- `RichText`: bold, italic, struck text, code, lists, quotes and links in plain text, with the marks chat apps use. It wraps between words, links take a click, Enter or Space and go to `on_link`, and text with nothing to format costs what a `Text` costs. Italics are a muted color and code a chip, because `ravenrs-text` has one style per text and embeds no italic or monospace font.
- `DocumentTabs`: tabs for open documents that close with a button, Delete or the menu, change places by a drag or Alt and the arrows, are renamed with a double click, F2 or the menu, and carry a color and a dot for what was not saved. Tabs that do not fit scroll with arrows, and the menu takes the app's own items.
- `SortableList`: rows put in order by a drag, with a line at the target, or with Alt and the arrows, Home and End. Each move is announced to screen readers, and Escape calls a drag off. The drag arithmetic lives in one module, which `DocumentTabs` shares.
- `YearView`: the twelve months of a year, with marks on the days, a click on any of them and one Tab stop that walks past the end of the year. As many months go on a line as fit.
- `FileBrowser`: an explorer of folders and files from the app's data, with a clickable breadcrumb, sorting with folders first, selection with modifiers, icons, sizes and dates. It never touches the file system, and folders of any size stay virtual.
- `PagedDocumentView`: a bar with page, total and `ZoomControl` over a column of pages of which only those that show are built, with the pictures handed over by the app page by page and a placeholder until each is ready.
- `Filmstrip`: a horizontal strip of thumbnails of any length, whose cells come from the app. Single or multiple selection with modifiers, the arrows, Home and End, arrows on the ends that have more to show, and the selected cell kept in view.
- `VirtualList`: a list of any length with rows of any height, measured as they appear and remembered by key. It jumps to a row, can stay at the end as a chat does, and holds what is on screen still when rows come in above. It can also run sideways.
- `BuildCx::keyed_child_frame`, for a view that places its children by the room each one took, and `BuildCx::wheel_scrolls_sideways`, for a strip that a plain wheel should move.
- `ImageLightbox`: a picture over the whole app, with zoom by `ZoomPanView`, the arrows to go on, Escape to close, a caption and a counter. Each picture starts fitted, and an arrow pans once it is zoomed in.
- `CompareView`: two pictures side by side, with one that can be chosen, or across a divider that a drag and the arrows move. A drag paints without a build.
- `Eyedropper`: a button that arms, and `EyedropperArea` over a picture, which asks the caller for the color of the point clicked. `EyedropperState::pick` tells it directly. Escape disarms. It does not capture the screen.
- `CropOverlay`: a crop over a picture, built on `TransformHandles`, with a veil outside, thirds inside, a fine grid while it turns, a fixed proportion and an angle. It stays inside the content and hands back `CropArea`; the frame turns over a still picture, since the renderer has no transforms.
- `Protractor`: a band of ticks along an arc that sets an angle by a drag from anywhere on it, with a double click back to zero, the keyboard and the value in words.
- `TransformHandles`: eight handles, a turn and a pivot around a `BoxTransform`, in the units of a `ContentSpace`. Corners hold the proportions until Shift, Alt resizes around the pivot, and the keyboard moves, resizes and turns. It hands back geometry and turns no picture. A drag paints without a build.
- `Marquee`: the animated outline of a selection around a rectangle, a closed path or loose segments, which reads over any picture and asks for no frames with reduced motion.
- `RubberBand`: a rectangle drawn with a drag, in the units of the content, that goes through a `Binding` and selects on release. Escape cancels, and the keyboard moves and resizes it. A drag paints without a build.
- `RuleOfThirdsGrid`: composition lines over a picture, as thirds, equal divisions, squares or the golden ratio, in the units of a `ContentSpace` that can follow a `ZoomPanView`. It takes no input.
- `Minimap`: a small copy of the content with a rectangle around what is in view, by the same `ZoomView` as `ZoomPanView`, which a press, a drag or the arrow keys move.
- `ZoomPanView`: zoom and pan over an image or a drawing, by pinch, Cmd/Ctrl with the wheel, the wheel and a drag to move, a double click and the keyboard, with a coast that stops asking for frames and respects reduced motion. Gestures paint without a build.
- `ZoomControl`: fit, 1:1, the percentage and the buttons that zoom in and out, through a `ZoomView` that `ZoomPanView` and `Minimap` can share.
- `CurveEditor`: points you drag over a curve that never swings past them. A press adds one, a double click or Delete removes it, and the keyboard chooses, moves and adds. It can show a baseline and a histogram behind, and `CurveEditor::value_at` gives the same curve for applying it.
- `ColorWheel`: hue and saturation on a disc, by a drag or the arrow keys, with Shift to keep the hue and a double click to take the color out. The disc is computed once per size and shared.
- `Ruler`: horizontal or vertical, in the units of the caller, with scale, origin and marker, and a round step that keeps marks apart at any zoom.
- `Histogram`: overlapping channels from ready-made bins, whose colors add up as light does, in a linear or logarithmic scale, with a mark at each end that clips.
- `DropZone`: a dashed area around any content that takes dropped files, by the same rules as `FileUpload`, and says when files are over the window with a stronger edge and words. `choosable` lets the keyboard bring files through the system's chooser, and `quiet` keeps the dashes off until then.
- `ScrollEdgeArrow`: a strip that scrolls, horizontal or vertical, with an arrow over a fade at each end that has more to show. `Labels` gains `scroll_back` and `scroll_forward`.
- `FloatingToolbar`: a translucent row of commands over its content, placed by side, that takes the clicks that land on it; `movable` adds a handle, with the arrow keys and a double click that brings it home. `TitleBar`: title, actions and the window's buttons, drawing and calling back only. It does not drag the window nor hide the system's frame, which belong to the platform layer. `Labels` gains `window_minimize`, `window_maximize` and `window_restore`.
- `ValueSlider`: a value in small, as "100% ▾", that opens a `Slider` in a popover with the keyboard in it; with `field` the number can be typed. `InlineRename`: a name that turns into a field by a double click, F2 or Enter; Enter and leaving confirm, Escape cancels. `TextField::select_on_open`. `Labels` gains `value_adjust`, `rename` and `rename_hint`.
- `Chart`: under the pointer the other bars and slices fade, a slice comes forward, and the cursor and its dots glide between categories and fade out. `no_hover_focus` and `no_hover_motion` turn this off; less motion in the system does the same for the movement.
- `ChartTooltip`: `show_percent`, `value_color`, `no_indicator_for` and `content`. It takes the side with room on both axes and stays off what is in focus.
- `Chart`: `grid_dashed`, `grid_columns`, `value_ticks`, `align_axis_ends`, `value_domain`, `reference_line` and `reference` with `ChartReference`.
- `Chart`: `label_guides` puts the labels of a pie or a doughnut in columns with elbowed lines, none over another, and `slice_radii` gives each slice its own radius.
- `Chart`: `bars_from` with `ChartBarFrom` makes bars grow from the top or the right, `min_bar_length` keeps a least length, `labels_at_zero` puts categories on the line of zero on the other side from the bar, and `gradient` now fills bars from tip to base too.
- `Chart`: a `ChartKind::Scatter` or a sparkline given a `tooltip` gets the tooltip, the cursor, the dots in focus and the arrow keys. Without one it stays a plain picture that takes no focus.
- `ChartKind::Candlestick` and `Chart::candles`, with an axis of round numbers that does not have to start at zero. Rising candles are outlines and falling ones are solid. `Labels` gains `candle_open`, `candle_high`, `candle_low` and `candle_close`. Reports can draw the kind, as shapes, though none offers it yet.
- `SankeyChart`: how much flows from what to what, with `SankeyAlign`, ribbons in a gradient, the rest fading around the focus, arrow keys, and every flow read aloud. Flows in a circle say which nodes they go through in place of the diagram. Its layout is `ravenrs_graphics::sankey`; `Labels` gains `sankey_cycle`.
- `Semantic::ChartGrid` (`chart-grid`), `Placement::RightEnd`, `LeftStart` and `LeftEnd`.

### Changed

- The handle of a `Splitter` is now a stop in the Tab order, so its panes can be resized from the keyboard.
- `Chart` measures its labels with the real font, and its gridlines land on whole pixels. Reports draw as before.
- The gap between slices of a `Chart` is now really empty, so what is behind the chart shows through it, rather than being painted in the color of the surface.
- `Chart` draws bars as quads, with no picture, and gridlines stop at their sides, so a bar that fades shows none through it. A line with more points than columns draws straight lines through the extremes of each column, its picture covers only what it draws, and a new size costs far less. See `docs/QUALITY.md`.

### Fixed

- A stack in a `Chart` rounds whatever is on its ends, also when the last series is zero or the stack goes below zero, and an empty series leaves no line across it.
- Categories closer together than a pixel no longer pile up on the axis of a `Chart`.

## [27.1.12] - 2026-11-04

### Fixed
- Shapes of curves past what the coverage atlas holds in a frame are no longer left out. Only about 25 shapes the size of a 2560x1600 window fit before; now the main pass splits to draw more, in scene order.

### Added
- `examples/calculator`, a small screen for comparing performance between versions, measured by the gate.
- `ravenrs-perf capacity`: how much of each kind of work fits in a frame on this machine. `ravenrs-perf` also measures the Feature Center.

### Changed
- The finance example reads its history of versions only while the dialog showing it is open.

## [27.1.11] - 2026-11-04

### Breaking
- `FrameSample` has new fields, `work` and `render_work`, and `RenderStats` has new fields for draw call splits, bytes uploaded, the glyph atlas and GPU time. Building one field by field needs `..Default::default()`.

### Added
- The diagnostics panel shows a timeline of the latest frames, why the screen was rebuilt, the costliest types of view, draw call splits and GPU resources, and copies a capture that Perfetto opens. It now opens on the web too, with Ctrl/Cmd+Shift+D. [docs/PERFORMANCE.md](docs/PERFORMANCE.md) explains how to read it.
- `Ui::stats`, `Ui::take_frame_work`, `Ui::set_phase_clock`, `Ui::phase_times`, `Ui::set_view_profiling`, `Ui::view_costs`, `TextSystem::stats` and `Renderer::set_gpu_timing`, which measure the work of a frame. They cost nothing until asked for.
- `View::type_name`, the name a view's cost is shown under.
- `ravenrs-perf`, not published: a gate of exact counters for every scene against `perf/baseline.json`, part of `scripts/check.sh`, plus frame times by phase.

### Changed
- What is outside its clip is not painted. A scrolling list of 1,000 cards went from 68,904 instances per frame to 482, and scrolling it from 5.7 ms to 0.66 ms on the CPU.
- `TextSystem` keeps recent layouts, so a rebuild does not shape the same text again, and text cut to fit is not measured again on every paint.
- The pointer moving over views that never asked about it no longer rebuilds the screen.
- `Lottie` repeats only the painting while it plays, instead of rebuilding the screen. It now asks for frames at 60 Hz, like other animated drawing.
- A chart that returns to the screen unchanged keeps its picture: opening the finance example's dashboard again went from 25.4 ms to 1.1 ms.
- `RenderStats::draw_calls` now counts the draw call for the coverage of shapes.

## [27.1.10] - 2026-11-03

### Breaking
- `BugReport` has new fields, `kind`, `area`, `elements` and `errors`, so building one field by field needs `..BugReport::default()`. Older `.ravenbug` files still open.

### Added
- `ravenrs-lottie` and the `Lottie` widget, with the `lottie` feature, on by default: a Lottie player of our own, with no third-party vector code. It draws Bodymovin JSON as curves on the GPU, with shapes, masks, mattes, precompositions, trims, dashes, repeaters, gradients, embedded images and the Fill effect, and `LottieAnimation::unsupported` says what a file asks for that it does not do yet. Against lottie-web, on 16 files straight from LottieFiles, 13 match and 3 come close.
- `Primitive::Path` and `Scene::push_path`: any filled shape made of curves, with solid, gradient or image paint and a clip of another shape. Each pixel gets the exact area covered, with no tessellation or multisampling.
- `SplashScreen::reveal_after` holds the name and status back while an animated logo plays, keeping their places.
- `BugReporter` lets the person choose what kind of report it is and drag a box around the problem, which names what is under it and lights it in the film. All that goes along is listed before sending.
- `RecentErrors`: a `tracing` layer that keeps the last warnings and errors, for `BugReporter::errors`.
- `DataGrid::even_row`, `odd_row` and `horizontal_lines`.
- `SystemNotifier`, with the `notifications` feature: the system's own notifications on macOS, Windows and Linux.
- `BuildCx::ask_for_access_tree` and `UiActivity::Described`: what is on screen, for an observer, only when asked for.
- `BuildCx::keyed_overlay`: a layer that stays the same one when other layers before it come and go.
- `AppIdentity::close_to_tray` and `SystemTray`, with the `tray` feature: closing the window keeps the app running behind an icon beside the clock, and opening it again, or clicking its icon in the Dock on macOS, brings the same window back. Where the system shows no such icon, such as plain GNOME, closing the window closes the app.

### Changed
- `DataGrid::striped` is now visible in every theme, tinting rows with the control color, and starts counting again after each group row. Lists generated by `ravenrs-app-ui` are striped. Rows under the pointer and selected rows are a little stronger, to stay apart from the stripes.
- A sidebar opens and closes in 200 ms instead of over a second, and its names no longer squeeze on the way: the edge covers them. Submenus and other folding parts also take 200 ms.

### Fixed
- Dialogs and menus open in 120 ms without shivering, and leave by fading as one surface instead of vanishing. `Motion::surface_exit` does it for panels of your own.
- A selected pill or tab marker no longer comes loose when the layout around it moves, as when a sidebar closes.
- `SplashScreen` no longer starts over, showing the page behind it, when an app with layers of its own appears under it.
- A native window stays hidden until the GPU is ready and, on macOS, until its first frame is drawn, so nobody sees it empty or black.

## [27.1.9] - 2026-11-03

### Breaking
- `Query` in `ravenrs-app` has a new field, `only`, so building one field by field needs `..Query::default()`.
- A `FieldKind::Criteria` field is out of lists by default and in a group of its own on a form, as it takes the height it needs.
- Saving a form of a `transient` entity no longer goes to storage.
- `ActionResult` has a new variant, `RecordsChanged`, and `EntityInfo` new fields, `state_machines`, `reminder` and `editor_settings`.
- `SpaceError` has a new variant, `Denied`, and `FieldInfo` a new field, `secret`.
- `DataGrid` shows its group panel only when something is grouped, or with `group_panel(true)`; the Group menu takes its place in the toolbar. While rows are selected, the toolbar says how many instead of adding to the row count.
- `LookupField` and `ColorPicker` fill the width they are given, unless told a `width`. `DocumentViewer` follows the language of the application unless given `labels`.
- A `FrameTemplate` gets the divider inside `TemplateParts::header`, and `MainTemplate` adds no space between the parts.

### Changed
- **Screens reviewed against `examples/feature-center`**, with the rules now in `docs/design-system/README.md`: one accented action in a window, one page margin, page titles, a flat filter row lined up with its columns, fields that fill their column, collections with one heading and no owner column, full-width panels with their title and Close at the top, and a list that says it is empty instead of repeating its name.

### Fixed
- A user turned off or deleted loses their API session at once, and with it settings, which no permission guards.
- The theme at a sign-in door no longer carries over from the person who left, and a kept theme nobody could read goes unused (`Theme::is_readable`).
- An expression could nest past the depth limit through a condition inside a field, or through `Between`, and overflow the stack. `FormatString` and `PadLeft` no longer take widths that exhaust memory or panic, and date functions no longer overflow on absurd amounts.
- Sorting a field that holds values of mixed kinds could panic.
- `SecuredObjectSpace` refuses a field someone can neither read nor write even when the value sent is the stored one, which told them what it held.
- A wrong current password when changing it counts as a wrong try, with the same lock as signing in, and the new password has to differ (`PasswordError::SameAsCurrent`).
- A long `Sidebar` scrolls and keeps its footer in the window, instead of stretching the whole window past the bottom of the screen.
- In the browser, a field that takes the keyboard when the page opens gets what is typed from the start, without a click first.
- A filter or rule in the model can use a function the application registered; checking the model refused it.
- Any `FieldKind::Guid` field gets its value when a record is created, and not only a key.
- A new form nobody typed in no longer asks about losing changes.
- A `DateTime` field shows as a date and a time on a list, and a yes-or-no column follows the application's language.

### Added
- **`ThemePicker`**, a menu of themes, and **`ThemeEditor`**, which changes the tokens of the theme on screen, says the contrast of text, and copies what changed as JSON or takes it back. `AppShell` shows the menu in the footer of its navigation, unless told `without_theme_menu`, and `examples/feature-center` has the editor. In `ravenrs-style`: `Theme::builtin`, `base`, `changed_tokens`, `to_tokens_json`, `from_tokens_json`, and tokens by name on `Semantic`, `Palette`, `Radii`, `TextStyle` and `Typography`.
- **`TextField::flat` and `end_aligned`**, and **`Alert::message`** for an alert with no title.
- **Rules that only warn.** `Rule::warning` asks and lets people go on, and `Rule::information` leaves a note on the field. A rule's `message` can say what the record holds, with `{field}` and `{reference.field}`.
- **Concurrent changes.** Saving a form keeps what someone else changed in other fields, with `merge_changes`, and asks about the same field. A record someone deleted says so.
- **Editing on a list.** `EntityInfo::list_editable` changes values right on the list, checking rules and marking cells that fail.
- **`DataAccess`** says how a list reads its records: all of them, as needed, or only its columns, through the new `Query::only`.
- **Custom fields.** A model difference adds stored or calculated fields, which get a column and a place on the form. `App::administrator_settings` and `App::storage_for` bring them to storage.
- **`FieldKind::Guid`** and **`FieldKind::Tags`**, and `predefined`, `edit_mask`, `criteria_of`, `display_format`, `group` and the `Progress` editor on fields.
- **`Message`** and `ActionContext::message` tell people something from an action. `ToastExtras` gives a toast a button and something to run once it is gone. `FormField::note_text` says something that is not an error.
- **`ActionCategory::InForm`** and `EntityInfo::form_actions` put actions inside a form's layout.
- **Records no storage keeps** get `use_records` and `use_collection`, can point to stored records, and a navigation item can open a form directly, with `ObjectKey` for one by its key.
- **`DashboardInfo::variant`** changes a dashboard to another from a menu.
- **`export_captions`** and **`import_captions`** take what people see out to CSV and back, for translating outside the code.
- **Security.** `SecuredObjectSpace` applies `Role`s to every read and write, with or without a screen: a policy per role, permissions on entities, records and fields, with criteria, and on navigation and actions; several roles combine in any or in all. A field marked `secret` never leaves it.
- **Signing in.** `SecuredApp` puts sign-in in front of an application, with users and roles managed on generated screens, Argon2id password hashes, lockout after five wrong passwords, a first administrator on a new installation and `Authenticator` for other ways in.
- **`AuditedObjectSpace`** writes who changed what, from what to what, in the same commit, with the modes `Full`, `Lightweight` and `CreationOnly`, fields left out by `AuditSettings`, and changes in collections shown on their owner.
- **`ModelEditor`**: the model as a tree, with its settings beside it, kept in the person's own layer, for a role that `can_edit_model`.
- **Lists shown other ways.** A view picks `Chart`, `Calendar`, `Scheduler` or `Tree` by `EditorType` in the model, or a `ViewVariant` by `editor`, with what each needs under `ChartSettings`, `CalendarSettings`, `SchedulerSettings` or `TreeSettings`, set in code with `EntityInfo::editor_setting`. A record moved on a timeline is kept with its rules and permissions. `examples/quick-plan` shows all of them.
- **`examples/feature-center`**: every feature of the application framework, one small demonstration each, with what to try on each list, and a test that goes through them all as three people, in memory and in SQLite.
- **`EntityInfo::description`** says what a list is there for, above its records.
- **State machines.** `EntityInfo::state_machine` declares states and the ways between them. Any other change is refused when it is kept, in memory and in SQLite, with no screen around it; each way gets its own action, and `state_appearance` goes by the state. `check_transitions` is for whoever writes another `ObjectSpace`.
- **Reminders.** `EntityInfo::reminder` brings up what is due in a window, to open, put off 15 minutes or dismiss. Nothing runs in between: the screen asks to be built again at the next time due. `ravenrs-app-library`'s events have one.
- **`local_now`** is the time on this machine as a clock reads it. The audit trail, lockout and the day pickers of the screens use it, as `DateTime::now` is UTC on the desktop.
- **Reports people keep.** `StoredReportsModule` keeps reports as records, on the data of an entity: a first layout comes from its list, `Designer::embedded` opens the designer inside the application, and showing one asks for its parameters and exports to PDF and XLSX. A list shows its records in a report with `ShowInReport`. Permissions on the records say who sees or changes a report, and a report only reads what the person can read.
- **Panels people put together.** `AnalyticsModule` keeps panels as records, with tiles of numbers and charts over the entities, edited on generated forms.
- **`ravenrs-report-designer`**: the report designer as a crate, out of `examples/report-designer`, which now just runs it.
- **`Session::show_full`** shows something in place of the view, with the navigation around it. `chart_view` and `summary_text` draw what `Chart` draws from settings given directly.
- **`Tabs`** go to another line when they do not fit, instead of running over each other.
- **`ravenrs-app-library`**: people, organizations, addresses, countries, tasks, events, notes and files, ready to use or extend.

## [27.1.8] - 2026-11-02

### Breaking
- Until 28.1.0 any release may hold breaking changes, listed here. Pin an exact version if that matters to you. See [docs/VERSIONING.md](docs/VERSIONING.md).
- `EntityInfo` in `ravenrs-app` is no longer `Send`, `Sync` or unwind-safe, since rules and appearance can hold closures.

### Added
- **`#[derive(BusinessObject)]`** and **`#[derive(BusinessEnum)]`** declare records as Rust structs and options as enums, in the new `ravenrs-app-derive`. A wrong attribute is a compile error that points at it, and a calculated field's expression is checked there too. `ObjectSpaceExt` reads and writes these structs.
- **`App`** in `ravenrs-app-ui` puts an application together from its records in a few lines, with the language of everything in it. [`examples/quick-app`](examples/quick-app) is a whole one in under 100 lines, on desktop and in the browser. The guide is [docs/APP-FRAMEWORK.md](docs/APP-FRAMEWORK.md), and [docs/APP-PARITY.md](docs/APP-PARITY.md) says what exists.
- **Validation.** `Rule` covers required, range, pattern, text and value comparison, criteria, is true, unique, unique together, exists, referenced and custom, by context and target criteria. A generated form that fails keeps nothing, and says what is wrong on each field and in a list on top. `#[field(required, unique, min, max, matches)]` and `#[rule(...)]` declare them.
- **Conditional appearance.** `AppearanceRule` changes font, background, weight, enabled and visible on fields, groups and actions, by criteria or function, in order of priority. `#[appearance(...)]` declares them.
- `DataGrid::cell_style` decides how each cell looks from its row. `FormField` gains `error_text`, `label_ink` and `tint`, and `Label` gains `ink`.
- **`ravenrs-app-sqlite`** keeps records in SQLite, on tables it creates or on tables an application already owns, sharing its connection. Filters run in the database as bound values, and what the database refuses comes back in words.
- **`EntityPane`** is a list beside its form, for a screen an application made itself.
- `DataGrid::name_column` says what names a row for its buttons, and a filling column leaves room for them. `Select::fill` takes the width of its parent. `ChangeSet` and `run_query` are there for whoever writes an `ObjectSpace`.
- **Relations.** `FieldKind::Collection` shows on a form the records that point to it, with `Link` and `Unlink`. A reference with the `Select` editor is picked from a list, narrowed by `lookup_filter` from the record being edited, with `none_caption` for empty.
- **`EntityForm`** is a form in a window that checks its rules and hands over the record, for what an application keeps through its own rules. **`ActionButton`** shows one action as a button, and `Session::embedded` lets a screen made by hand use lists and actions of the framework.
- **Popups with forms.** `Action::popup` on a detail view asks for a record to be filled in, started by `on_prepare` and received as `ActionInput::Record`. A parametrized action reads what is typed as the kind it asks for, and `confirmation_words` says what its question says.
- **`DashboardInfo`** and **`DashboardView`** put views and registered controls together, each in a frame of its own. **`ViewVariant`** gives a list ways to be seen, picked from a menu, with the choice in the person's layer of the model.
- `ListView` gains `filter`, `sort`, `column_shape`, `grid_shape`, `single_selection`, `plain_toolbar` and `remote`, which brings rows through `RemoteRows`. Pictures and files on a form have real controls.
- **`ravenrs-app-report`** gives every list an action that lays it out as a report and shows it in the viewer. `Session::list_table` and `ravenrs_grid_export::document_of` are what it stands on.
- **List editors.** `register_list_editor` adds a way to show a list, picked by the model. `ListFilter`, `list_criteria` and `ViewVariant::columns` put filters and columns in the model, and `register_function` adds functions to every expression.
- **The model does more.** Rules can be set under an entity, `translation` and `set_language` change its words, and `SqliteModelStore` or any `ModelStore` keeps a person's settings through `use_model_store`.
- **`CloneModule`** clones what says it can be, with `non_cloneable` and `aggregated`. **`AttachmentStore`** keeps files apart from records, with `MemoryAttachmentStore` and `SqliteAttachmentStore`, and a form saves a file back out.
- `SqliteBuilder::migration` runs something once, `managed` adds columns for new fields, and `entities_from_schema` starts from an existing database.
- `checks_on_leave` validates as fields are left, through `on_leave` on `TextField` and `NumberField`. `transient`, `ListView::source`, `ApplicationBuilder::service` and `Module::initial_records` complete the framework's core. The derive has attributes for all of it, with `#[variant]` and `#[list_filter]`.
- `DatePicker::fill` takes the width of its parent. `ravenrs_report::base64` is public.
- `FilePicker::save` hands a file to the person: the system's dialog on a desktop, a download in a browser. Exports from generated lists use it unless the app takes them.

### Changed
- The registers of the Raven Finanças example are generated from what they are, and say what is wrong when a name is missing instead of doing nothing.
- The transactions and bills of the Raven Finanças example, their lists, forms and commands, and its plans and payments are generated from what they are too, with every write still going through its database rules.

### Fixed
- A field with options starts on the first one, which is what it shows, instead of holding nothing.

## [27.1.7] - 2026-11-02

### Added
- **Editors for business forms.** `TextArea`, `NumberField` (also money in whole cents), `MaskedField`, `TimePicker`, `DateTimePicker` and `LookupField`, whose options can come from a database or a server.
- **`FormLayout`** arranges fields in groups and columns that collapse on narrow windows, with tabs and a list of errors.
- **Commands.** `Toolbar` folds what does not fit into a menu, `SplitButton` offers alternatives to a main command, and `ConfirmDialog` asks before something final.
- **`FilterBuilder`** writes filters from lines of field, comparison and value, in the expression language, and keeps what it cannot show as a line.
- **`DataGrid` over records.** Columns can name their field (`GridColumn::field`) and hold dates (`GridColumn::date`, `Value::Date`). `key_column` and `selected_keys` keep a selection across data changes, `on_row_open` gives the row's values, `on_cell_edit` can refuse an edit and say why, and `row_action`, `on_add_row` and `on_delete_rows` add commands for rows. `GridData` gains `insert` and `remove`.
- **`Hotkey`** makes a key combination run a command, as `ToolbarAction::hotkey`, also from a folded toolbar.
- **`TreeView`** opens, selects, checks in three states, filters, brings items on demand and builds only the lines in sight. Screen readers hear it through `Role::Tree` and `Role::TreeItem`.
- **`TagBox`** picks several values as tags, **`ColorPicker`** offers a palette, a shade square, hue and opacity bars and hexadecimal, and **`DateRangePicker`** takes a period in two clicks or ready-made.
- **`Wizard`** goes through a task in steps, each able to refuse to be left and say why.
- **Files.** `FileUpload` and `ImageField` take files from the system's chooser or dropped on them, on desktop and web, and say why they refuse one. `BuildCx::file_picker`, `BuildCx::on_file_drop` and `Ui::set_file_picker` let any view do the same.
- **`TileLayout`** arranges a dashboard of tiles to drag and resize, also from the keyboard. **`CardView`** shows records as cards in as many columns as fit, and builds only those in sight.
- **`DataGrid` row details.** `row_detail` opens a view below a row, with the arrow or the Right key.
- **`DataGrid` to Excel and PDF.** `on_export_file` shows a menu of formats and hands over what the grid shows; the new `ravenrs-grid-export` turns it into a file through the same exporters as reports. `wizard::table_report_with` lets a table report take captions and formats from the caller.
- `DataGrid::filter_builder` puts a `FilterBuilder` in the toolbar, and rows on a server can be edited from the keyboard, with Enter or F2.
- **`ravenrs-app`**, the start of an application framework with no GUI yet. `EntityInfo` declares a kind of record; `generate_model` gives it a list, a list to pick from, a form and a place in the navigation; `ModelLayers` changes that from modules, an administrator and a user, each as a difference in JSON; `ObjectSpace` and `MemoryDatabase` read and write records in a unit of work. `Action` comes in five kinds, each on or off by reasons, by the view it targets, by what is selected and by criteria over it; `ViewController` and `WindowController` bring actions and react to views; `Frame` holds a view and turns them on and off in a fixed order; `Module` and `Application` put it all together and say what is wrong before any screen opens.
- **`ravenrs-app-ui`** turns the model into screens: `AppShell` with navigation, `ListView` over `DataGrid`, `DetailView` over `FormLayout` with a control for each kind of field, `ActionContainer` over `Toolbar`, and `SystemModule` with new, save, cancel, delete, refresh, back and moving between records. `FrameTemplate` rearranges the window, and `EditorRegistry` changes a field's control. `ObjectSpace::insert` refuses a key already in use.
- `BuildCx::report_frame` tells a view the room it got, for those that arrange themselves by it.
- `disabled` and `read_only` on `TextField`, `Select`, `Combobox`, `DatePicker`, `Checkbox`, `Toggle` and `Stepper`. Screen readers hear about read-only controls through `AccessNode::read_only`.

### Changed
- `Ribbon`, `Carousel` and `InputOtp` take their words from `Labels`, so they follow `Localized`.

### Fixed
- A view that builds from the size of its scroll container, as `DataGrid` does, gets a new build in the same frame when that size changes, instead of waiting for the next input.

## [27.1.6] - 2026-11-02

### Added
- **Touchpad pinches on Wayland.** winit 0.30 delivers them only on macOS and iOS, so the new crate `ravenrs-wayland-gestures` listens on winit's own connection. It holds the workspace's only `unsafe` call, and goes away with a winit that delivers them.

### Changed
- `Chart`'s tooltip glides from one place to the next, and follows the pointer along what is free, instead of jumping.

### Fixed
- `DocumentViewer` adds up pinch movements too small to change a whole percent, which a touchpad can send and were being lost.

## [27.1.5] - 2026-11-02

### Added
- **Every pattern of the shadcn/ui chart catalog.** `Chart` draws all 70 of them, in the gallery under their names: `curve`, `stack`, `gradient`, `horizontal`, `vary_colors`, `negative_color`, `active`, `dots`, `dot_icon`, `value_labels`, `category_labels`, `grid`, `center_text`, `concentric`, and more.
- `ChartKind::Radar` and `ChartKind::RadialBar`, in charts and in what reports draw.
- `ChartTooltip` says what is under the pointer or the arrow keys, with indicators, formats, icons and a total. `Chart` takes keyboard focus for it; `hide_tooltip` turns it off.
- `ChartCard` is a chart's card: title, description, what changes it, tabs with totals, and how it trends.
- `Semantic::Chart1` to `Chart5` are series colors for each theme. Charts no longer use status colors, so a series does not look like an error.
- `ravenrs_graphics::plot` lays these charts out, and says where each value landed. `vector::rasterize_layers` adds gradients and dashes to what a picture can have.
- `BuildCx::on_pointer_move` says where the pointer is over an element.
- `Pagination::compact` shows "1 / 12", so its buttons stay in place.
- **`DataGrid` grouped by dragging.** A header dropped on the group panel groups by that column; chips change places by dragging, and leave by dragging off the panel. Expand all and collapse all. Group rows line their summaries up under their columns, and a grouped column shows in its groups.
- `DataGrid` columns move by dragging, stay in place with `GridColumn::pinned`, and have a menu on a right click or from their header. A filter by value lists what is in a column, and a panel says what is being filtered.
- **Scenarios.** `DataGrid::scenarios` saves how a grid is arranged under a name, with `GridScenario` and `GridLayout` as JSON; `DataGrid::layout` keeps the arrangement outside.
- **Summaries with formulas.** A click on a total changes what it sums up, or takes a formula such as `Sum([Seats]) / Count()`, checked as it is typed, over each group's rows. `GridColumn::summary_formula` gives one in code.
- **Pages and rows on a server.** `DataGrid::page_size` shows pages. `DataGrid::remote` with `RemoteRows` asks for what is on the screen once scrolling rests, keeps a limited number of chunks, drops late answers, needs no total, and hands the loader the row before a chunk so that a query can continue from it.
- `examples/grid-poc` shows all of this working.
- `Chat::busy` holds what is sent until an answer arrives.
- **Narration recorded ahead of time.** `Narration` holds recordings of what films say, compressed to about 8 KB a second, and `MoviePlayer::narration` plays them in place of the system's voice, which still says whatever has no recording. `Sound::speak_clip` says a recording that `stop_speaking` can stop.

### Changed
- The legend of a `Chart` is real text under or over it, and numbers on the axis are optional: `show_value_axis`.
- A screen reader hears every series of a `Chart`, not only the first.
- `DataGrid` has a quieter look: lower header and footer, gridlines only between rows, column dividers only in use, and rows that go in under their groups.
- `DocumentViewer` uses the compact pagination, so another document moves nothing in its toolbar.

## [27.1.4] - 2026-11-02

### Added
- **Speech with LFM3 Audio.** `SpeechEngine` runs the 2B model on the CPU in plain Rust, as its model card says: 19 languages and locales, two voices, and the card's sampling values. `LocalVoice` says text a sentence at a time on its own thread, `VoiceManager` is the screen to choose the model, language, voice and fine settings, and `Chat::on_listen` puts a button to hear each answer.
- `Scheduler::card` is the look of a full scheduler, as a card among other cards: a tinted header with the month over the days of the week, a line between the rows' names and the timeline, today's column marked, and bars in a soft tint with a stripe of their color. `Scheduler::event_detail` puts a second line on the bars, and `Scheduler::avatars` the initials beside the rows' names.
- `Scheduler` scrolls by itself while a drag is near an edge, zooms with a pinch or Ctrl and the wheel, scrolls with the wheel over its names, and brings into view the event keyboard focus reaches. `BuildCx::scroll_with_widget` and `shortcut_with_shift` are new.
- `Scheduler::event_menu` and `lane_menu` offer menus on a right click, and with `card` a tooltip shows an event's whole name and where a drag is taking it, and the row under the pointer lights up.

- `HelpTip` is a small "?" that explains what something is and how to use it, under the pointer, on keyboard focus and, after a click, until a click elsewhere.
- `Sidebar::rail` opens and closes it from a strip along its edge, `SidebarBrand` is an app's name that keeps only its mark when reduced to icons, and `SidebarItem::shortcut` shows the keys that do the same.

- `MoviePlayer` plays at three speeds, jumps with the arrow keys, shows where chapters begin on its timeline, which step of how many it is at, the chapters already passed, and at the end offers to watch again or `next`. `Slider::marks` draws the notches.

### Changed
- `FloatingPanel` no longer gets in the way of what is on the page: `BuildCx::floating_overlay` keeps it under every menu and dialog, wherever it is built, and it keeps what is written when they open. Escape closes the last menu or dialog before anything that listens for it.
- `LocalAssistant` stops waiting after a minute, and `too_slow` says so: from then on it answers nothing, at once, so an app can go on without it. A choice also ends as soon as its JSON closes.
- `Recorder::guide` brings the pointer to what a step asks for before stopping there.
- A scrollbar shows only around a scroll and then fades, so it no longer sits over what it scrolls at rest. `ScrollView::scrollbar_outside` puts it in the margin around the view.

### Fixed
- What leaves the screen keeps its stacking: a closing dialog could pass under its own backdrop, depending on the run.
- `Avatar`'s initials can be read in a light theme.
- Dialogs, menus, panels and toasts stay solid while they come and go: fading, they showed the page through their fields, in a gray like metal that stands out in a light theme.

## [27.1.3] - 2026-11-02

### Added
- **`Chat` and `FloatingPanel`.** `Chat` is a conversation over a field to write in, with examples and cards to act on; it does not know who answers. `FloatingPanel` is a round button in the corner of the window that opens a panel over the app, which stays usable behind it. Dragging it or pressing an arrow key sends it to another corner, and it asks for frames only while the pointer or the focus is on it. `Labels` gains `send`.
- **Local AI models.** `ModelPicker` lists models to download, try, use and remove. The new crate `ravenrs-ai`, desktop only, does the work: `ModelManager` offers only the models that fit the machine's memory and disk and tests their speed, `ModelFolder` downloads each when someone asks, checking its hash, and removes it, `LocalEngine` runs one on the CPU with no extra build tools, and `LocalAssistant` keeps it on its own thread, and `tool_rules` and `read_tool_call` let it choose among what the app offers, which is all it can do. Nothing else talks to the network.
- **`MovieChat`.** A `Chat` that answers from the training films: each answer is a card with the moments that open. It looks only at the words the films hold, on this machine, and `MovieChat::writer` can have a local model write the answer about what it found. `ChatMessage::assistant_writing` shows an answer as it arrives, and `MovieAnswers` answers about the films for a chat that also does other things. `MovieAssistant::cue` lets it open a moment on the same player.
- `Alert::action` puts a button beside the message, giving an `Alert<AlertAction>`; an alert without one stays `Send`.
- **LFM3 in `ravenrs-ai`.** The catalog offers LFM3 1.7B, which `LocalEngine` runs with its own implementation, since candle stops at LFM2. It needs 4 GB of memory at the peak and chose the right tool in 27 of 32 requests. Its license allows commercial use only below 10 million dollars of annual revenue.
- `GridColumn::fill` takes the width the columns leave, and `DataGrid::empty_text` says what no data means, apart from filters leaving nothing.
- `Scheduler::fill_rows` shares the height left among the rows.
- `Stat::large` for the one number a screen is about.
- `MovieAssistant::height` and `MoviePlayer::max_film_height` keep a film and its controls in view; the film's choice becomes a select when its names do not fit. A film that has not started shows a play button over its picture.

### Changed
- `FloatingPanel`'s button is smaller and calm: the theme's surface with an accent icon, in line with its panel, which lines up with the page's margin.
- `Chat` sets apart only what the person asked; the answer sits on the panel, and a card takes the whole width.
- `MovieChat` answers in words first, then the moments; it and `MovieAssistant` leave out the steps inside the chapters they found. Answers open over the film instead of pushing it down.
- A film's chapters show how long each one takes, and the current one what it teaches; a player with boxes over its film no longer repeats that under it.
- `Badge` text moves toward the ink, to keep 4.5:1 on its tint.
- `read_tool_call` accepts a tool's name written with accents or capitals, as models do.

## [27.1.2] - 2026-11-01

### Added
- **Sound.** `Sound` plays short clips and speaks with the system's voice, through `BuildCx::sound` and `Ui::set_sound`, on the desktop and in the browser. `SoundClip::click` and `SoundClip::key` are computed, so they add nothing to an app's size. On Linux, compiling now needs ALSA's headers.
- **Films that talk.** `MoviePlayer` plays the clicks and keys of a film, reads what it says aloud, and shows it in a box next to what it talks about, with a switch for each sound. `Recorder::intro` and `Recorder::explain` time their boxes to the text. `Recorder::guide` now also waits for its text to be read, which the player skips while it stops at steps; a film recorded before this plays as it did, with no key sounds.
- **Bug reports from the diagnostics panel.** `AppIdentity::new_issue_url` adds a button that opens a new issue in the browser with the report filled in, cut to a safe length, for the person to review and send. Nothing is sent by the app, and the panel says what the report has before it opens. `ravenrs::native::open_url` opens a web page, and `ravenrs_core::new_issue_url` and `redact_user_paths` are the plain functions behind the button.
- **Performance test.** The diagnostics panel has a "Test performance" button, and `Diagnostics::start_test` starts it from an app's own screen: it redraws the window for 5 seconds and says in plain words whether the app is fluid, how many frames per second it drew, and whether the time goes to drawing or to building the screen. `WindowOptions::diagnostics` shares the panel with the app.
- **Diagnostics on the desktop.** Ctrl+Shift+D (Cmd+Shift+D on a Mac) or `RAVENRS_DIAG=1` opens a panel in any app with the GPU in use, GPUs left out for their drivers, what is wrong and what recent frames cost, with a report to copy into a bug report. `ravenrs::native::diagnose` gives the same GPU report without showing a window. Closed, the panel costs nothing. `WindowOptions::labels` sets the words of the whole UI, this panel included.
- **Expressions in the grid's filter row.** A `DataGrid` filter can be a `ravenrs-criteria` expression over column titles, such as `[Seats] > 10 And [Country] = 'Brazil'`. The short syntax (`>=15`, `5..12`, `sim`, plain text) works as before.
- `ravenrs_criteria::NumberStyle` lays out digits with any separators and affixes; `Culture` and the grid's `NumberFormat` both use it.
- `ravenrs_core::csv::push_field` quotes a CSV field; the grid and report exports both use it.
- **Charts in the UI.** `Chart` draws line, area, bar, pie, doughnut, scatter and sparkline charts with the layout reports use. The shapes become a picture at the screen's density and the labels are real text. Screen readers hear the values.
- **Barcodes in the UI.** `Barcode` shows a QR Code, Code 128, Code 39, EAN-13, EAN-8, UPC-A or Interleaved 2 of 5 with every module on whole pixels. Text a format cannot hold shows why.
- `Paint::Rich` draws with a `PaintCx`, for views that shape text while painting or need the device scale.
- `charts` feature on `ravenrs` and `ravenrs-widgets`, on by default. `ravenrs` also forwards `decode`.
- `ChartKind::SparkArea`, a smoothed line over a filled area, and `ChartStyle` with `chart::draw_styled` for colors, bar corners and the widest bar.
- **`MovieAssistant`.** Someone types a question and the moment of a training film that answers it opens. `MovieLibrary` searches the chapters and steps the films already hold, ignoring accents and case and accepting near forms of a word and typing slips, with no model and no network, on the desktop and in the browser. `MovieCue` opens a moment of a `MoviePlayer` from outside it. `ravenrs_text::fold` is the folding that searches compare with, shared with `DocumentViewer`.
- **Training films (`ravenrs::movie`, crate `ravenrs-movie`).** `Recorder` runs the real app from a script and stores what it drew, with no pixels, no GPU and built-in fonts by reference, so films are small enough to ship inside an app and stay sharp at any zoom. `MoviePlayer` plays them with chapters, marks to click and steps that wait. `MovieEditor` edits them on a timeline in the style of screen-recording editors: cuts, speed, title cards, callouts, captions, spotlight, hiding regions, zoom and pan, fades, pointer highlight and clickable spots. There is no sound yet.
- **`Timeline` and `ToolRail`.** `Timeline` shows blocks along time on named tracks, under a ruler with markers and a playhead, and reports moves and trims. `ToolRail` is a narrow bar of tools with their names under their icons. `IconButton::disabled` makes an icon button inert.
- **Bug reports with a film.** `FlightRecorder` keeps the last minutes on screen in memory, and `BugReporter` is the dialog that lets someone watch them, cover or cut parts, and describe the problem. Nothing leaves without them, and where it goes is up to the app.
- `BuildCx::observe_activity` watches painted frames, the pointer and clicks across the whole UI. `ravenrs_text::builtin_font` and `builtin_font_number` name the built-in fonts across runs.
- `ravenrs::native::OpenDialog` shows the system's dialog for choosing a file to open.
- `ravenrs::native::run_as` opens the window as an `AppIdentity`, whose id ties it to the app's `.desktop` file. Without it, GNOME on Wayland shows no icon for the window, since Wayland windows cannot carry one.
- **`StatusBar`.** One fixed line at the foot of an app window: its sides keep their width, the center takes the rest and cuts what does not fit, and `center_tone` tints only the center. `ButtonSize::Mini`, 22 px, is what fits on it, and `Toaster::bottom_inset` lifts toasts above it.
- **`ReleaseNotes` and `CodeBlock`.** `ReleaseNotes` shows an app's versions, the list on the left and what the chosen one brought on the right, with buttons that walk through them. `CodeBlock` shows a command to copy. `Labels` gains their words.
- **Waking the UI from another thread.** `UiWaker`, from `BuildCx::ui_waker`, asks for a rebuild from any thread, so background work needs no polling. `Ui::set_remote_waker` connects it to a platform.
- `ravenrs::native::SaveDialog` shows the system's dialog for choosing where to save, and `ravenrs::native::open_with_system` opens a file with the system's program.
- **Pinch to zoom.** `Event::Pinch` carries a trackpad pinch, or Ctrl with the wheel, and `BuildCx::on_pinch` receives it. `DocumentViewer` zooms with it.
- **Printing from the viewer.** `DocumentViewer` has a print button, on by default, and `ravenrs_report_viewer::print` prints a document on the desktop and in the browser. The designer uses the same code.
- `Color::faded` makes a color less opaque relative to what it already is, which is what a theme's translucent tokens need.
- `ravenrs::native::run_with_icon` opens the window with an icon, for the title bar and task bar on Windows and Linux.
- **`Scheduler`.** A timeline of resources on the model of Bryntum Scheduler: events as bars that drag along time and between rows, resize by their ends and are drawn on an empty stretch, with `can_drop` to refuse a place, zoom from months to hours, lanes for overlaps, shaded weekends, collapsible groups, keyboard control, and only what is in view built. `TimeAxis` and `TimePoint` describe its time. What is still missing is listed in [docs/PARITY.md](docs/PARITY.md).
- **`EventCalendar`.** A month with its events inside each day.
- **Recurrence.** `RecurrenceRule` says on which days something repeats, with no UI, and reads and writes iCalendar's `RRULE`. `RecurrenceEditor` edits one and says it in words.

### Changed
- `ravenrs-widgets` now depends on `ravenrs-criteria`, without its JSON feature.
- A filter-row text that names a column in brackets and parses, such as `[Seats] > 3`, is now an expression instead of a search for those characters. Other text in brackets, such as `[draft]`, is still a search.

### Fixed
- **GPU choice on Linux.** The native window gives wgpu its display, so the GL backend can present on Wayland when no Vulkan driver is usable, instead of leaving only software rendering. A warning now says when the app renders on the CPU, and `RUST_LOG=ravenrs_platform_native=debug` logs how long each frame's update, acquire, render and present took.
- **The app could draw on the CPU with a real GPU present.** No power preference was asked for, so wgpu took the first adapter it found: on an old Intel GPU on Linux, whose Vulkan driver it hides, that was software Vulkan, at about 11 frames per second, instead of the GPU on GL. It now asks for the GPU that spends the least, and `WGPU_POWER_PREF` overrides that. An adapter that refuses the limits no longer stops the app while another one works.
- The wheel over an open `Dialog` or `Sheet` no longer scrolls the page behind it, as a pinch already did not.
- In the glass theme, the `DataGrid`'s header, footer and group rows and the lines of `DataGrid`, `Ribbon` and `Scheduler` came out far too opaque, with the header text unreadable: `with_alpha` replaced the token's own transparency.
- A `Select`, `DatePicker` or menu inside a `Dialog` or `Sheet` opened in the window's corner, behind the dialog's backdrop. Overlays opened from inside another now come after it: they sit by their anchor and on top, keep focus, and Escape closes only them.

## [27.1.1] - 2026-11-01

### Added
- **Family finance example.** `examples/family-finance` is a desktop app for a family's expenses and bills to pay on SQLite, with reports, expression filters in SQL and updates that compile on the user's machine. It exists to show the framework and to find what is missing in it; see its [README](examples/family-finance/README.md).
- **Components in other languages.** `Labels` holds the words the built-in components use and `Localized` applies them to everything inside, so an app sets its language once. `Labels::pt_br` comes ready.
- **A fuller `Sidebar`.** It closes to a rail of icons or off the screen with `SidebarCollapse`, by Cmd/Ctrl+B or a `SidebarTrigger`, sharing a `SidebarState` that an app can drive with its own binding. It has `header` and `footer` around content that scrolls, `side`, `SidebarVariant::Floating` and `Inset` with `SidebarInset`, and with `narrow_below` small windows show it as a sheet. `SidebarGroup` folds and carries actions; `SidebarItem` gains `text`, `badge`, `disabled`, `action` and submenus with `item`; `SidebarSkeleton` holds the place while items load. On the rail, labels become tooltips, on hover and on keyboard focus. Arrow keys move between rows and fold them. Sidebars that use none of this look as they did.
- `Role::Navigation`, for navigation landmarks, and `Placement::RightCenter` and `LeftCenter`.
- `SplashScreen` covers the window while the app loads, with a logo or icon, the app's name, a status that screen readers announce, and a spinner or progress bar. It leaves with a fade, without the zoom under reduced motion.
- `Footer` ends a page or an app with left, center and right areas and a hairline on top. The center stays centered between uneven sides, and in a narrow window the areas stack. `Role::ContentInfo` is its landmark for screen readers.
- `Align::Fit` keeps a child's own size without overriding a parent that centers its children.
- `Stat::delta_toned` separates a change's direction from whether it is good, for numbers such as spending.
- **Report server.** `ravenrs-report-server` stores layouts with versions, renders them over HTTP, and sends scheduled reports by email or to a folder as PDF, XLSX or CSV. Jobs have cron schedules with time zones, parameters computed by expression, skip if empty, retries that do not send twice, retention, and a history of who received what. See [docs/REPORT-SERVER.md](docs/REPORT-SERVER.md).
- `ReportEngine::count_rows` says how many rows a report would print.
- **Document viewer.** `ravenrs-report-viewer` has `DocumentViewer`, with pages, zoom, fit to width, thumbnails, and search with highlights that ignores case and accents, and `ParameterPanel`, with a control for each parameter kind and apply and reset. The report designer's preview uses both.
- **Charts in reports.** `ControlKind::Chart` draws bar, line, area, pie, doughnut, scatter and sparkline charts from the rows in scope or a nested list, adding up rows with the same argument. PDF and SVG write them as vectors with real text; the screen and XLSX get a picture, with the text still in cells. The designer has a chart button and a "Painel de vendas" template.
- **Barcodes in reports.** `ControlKind::Barcode` prints Code 128, Code 39, EAN-13, EAN-8, UPC-A, Interleaved 2 of 5 and QR Code from fixed text or an expression, adding check digits. PDF and SVG draw them as vectors and XLSX as one picture.
- **Shared drawing logic.** `ravenrs-graphics` holds vector shapes, chart layout and barcodes, and depends on no engine. `Item::Vector` carries its shapes to every output.
- **One way to export.** `ravenrs-report-export` turns a `Document` into a file with its name and media type; the designer and the server both use it.
- `Scope::current_row` and `Scope::current_rows` read what a scope holds.
- **Pictures in reports.** `ControlKind::Picture` shows a PNG or JPEG from the layout or a field, fitted by `PictureMode`. A PDF keeps a JPEG's original file and stores an image once however often it prints; XLSX and SVG show them too. `decode_picture` and `picture_rect` are public, and the designer inserts and chooses images in the browser.
- **Links and outline.** `ControlKind::Link` opens an `https:`, `http:` or `mailto:` address from a PDF, a spreadsheet or SVG; any other scheme is refused and reported. `ControlKind::Bookmark` adds entries, nested by level, to the PDF's outline. `Item` gains `Image`, `Link` and `Bookmark`.

### Fixed
- A control that took keyboard focus while scrolled out of view stayed hidden. Tab, arrow keys and `request_focus` now scroll the containers around it, nested ones included, until it shows; a click still does not scroll.
- A full-window layer with nothing to focus left keyboard focus on the page behind it, where Enter could still press a button.
- `Button`, `Badge`, `DatePicker` and other controls ignored a stack's `Align::Center` and sat at its start.
- `None` and other empty views took a gap in their stack.
- `DocumentViewer` worked out its width from the whole window, so beside a sidebar the page was off center and scrolled sideways.
- A pie or doughnut chart listed categories with no slice in its legend.

### Changed
- The site roadmap, `docs/PARITY.md` and `ARCHITECTURE.md` now reflect what shipped.
- **`BarChart` and `AreaChart` look different.** Their API is the same, but they now draw through `Chart`. `BarChart` gains a numeric axis and has flat bars instead of a gradient. `AreaChart` has a flat fill instead of a gradient, and it no longer reaches the frame's left and right edges. Their boxes are the same size.
- `BarChart`, `AreaChart` and the new components need the `charts` feature. Apps that use the default features need no change. If you set `default-features = false` on `ravenrs-widgets` and use these charts, add `features = ["charts"]`. On `ravenrs`, `default-features = false` now also turns off `decode` and `charts`, which it could not do before.

## [27.1.0] - 2026-11-01

### Migrating from 26.2

This release is ahead of the usual calendar because these changes are breaking, and breaking changes ship only in a `YY.1` release. Most apps need no change.

- `Theme` has a new field, `motion`. A `Theme` written out field by field needs it: add `motion: Theme::raven_dark().motion`, or start from a built-in theme and change what you need.
- `Document` has `culture`, `Page` has `page_header` and `page_footer`, and `TextBlock` has `align`, `value` and `format`. Code that builds these by hand needs the new fields; with `Document` and `Page`, `..Default::default()` covers them.
- `Paint` has a new variant, `Live`, and is now `#[non_exhaustive]`. A `match` on it needs a `_` arm.
- `Card` and `GlassCard` are no longer `Send`, `Sync`, `UnwindSafe` or `RefUnwindSafe`, because they can hold a click handler. Views live on the UI thread, so this matters only to code that required those traits explicitly.

### Added
- **PDF export.** `ravenrs-report-pdf` writes a report `Document` as vector pages with subsets of its fonts embedded and selectable text, in pure Rust, on servers and in the browser.
- **XLSX and CSV export.** `ravenrs-report-xlsx` lays a `Document` out on a grid that follows the page, with real numbers and dates in the cells and report formats translated to Excel's. CSV uses the culture's separator.
- The report designer exports to PDF, Excel and CSV from the ribbon, the File menu and the preview toolbar.
- `Document::culture`, `Page::page_header`, `Page::page_footer`, and `TextBlock::align`, `value` and `format`, for exporters. `TextLayout::glyphs` reports shaped glyphs with the text they stand for.
- **A page for each component on the site.** Every component has a live example and its code. `samples.rs` in the gallery holds the examples, the gallery shows one alone with `?component=Name`, and `scripts/gen-component-pages.py` builds the pages at deploy, failing if a listed component has no example.
- **Component list on the site.** `site/components/` lists every component by category, with what it does, a link to its API, an A to Z index and a filter. `scripts/check-component-list.py` fails the site deploy when a link breaks or a public component is missing.
- **Motion.** The new `ravenrs-animation` crate has closed-form springs, CSS-compatible curves, OKLab color mixing and `Animated<T>`, which changes target with no jump in position or velocity. `BuildCx::motion` and `Motion` declare paint and layout transitions, shared layout, effects, and enter and exit animations. While only animations run, frames repeat the paint phase alone. Also new: `Collapse`, `BuildCx::animated`, `Paint::Live`, `Effect`, `Scene::apply_effect`, `Theme::motion`, `Ui::is_animating` and `Ui::set_reduced_motion`, which the browser now drives from `prefers-reduced-motion`.
- Built-in components animate: buttons, toggles, check boxes, radio buttons, segmented controls, tabs, disclosures, accordions, sidebars, text fields, menus, popovers, tooltips, dialogs, sheets, the command palette, toasts, carousels, progress bars, spinners and skeletons. Switching themes crossfades.
- **Skill and MCP server for AI assistants.** The Claude Code plugin in `plugins/ravenrs` brings a skill on building with RavenRS and `ravenrs-mcp`, a read-only server that works offline. Its tools are `list_components`, `component_api`, `component_examples`, `design_tokens`, `parity_status`, `validate_expression` and `expression_functions`. It is installed from git, not published to crates.io, and nothing in the framework depends on it. See the [README](README.md#ai-assistants).
- `ColorSwatch`, a named color to pick from a palette.
- `Text::truncate` and `BuildCx::text_truncated` keep text on one line that ends with `…`; screen readers still get the whole text. `TextSystem::layout_ellipsized` is the underlying call.
- `Card::on_click` makes a whole card a button, with hover, focus and keyboard support.
- `SidebarItem::compact`, `Dialog::width`, `Dialog::close_label`, `DropdownMenu::size`, `Checkbox::leading` and `Pagination::labels`.
- `Semantic::Scrim` and the `scrim` token, the backdrop behind dialogs, sheets and the command palette.
- `Report::parameter_label`.

### Changed
- `TextField`, `Select`, `Combobox`, `DatePicker`, `Stepper` and `InputOtp` fill with the `bg-300` control token, as the design system specifies.
- Compact text fields are 32 px tall with `radius-md`, the same as small buttons, and the data grid toolbar now has one height throughout.
- The ribbon's application menu is neutral; the accent is kept for the selection.
- Sidebar items truncate long labels instead of wrapping them over their neighbors.
- Dialogs, sheets and the command palette share one backdrop opacity.
- Sample reports and the report wizard use Raven Light colors.
- The report designer is built from RavenRS components and tokens, and follows the Dark, Light and Glass themes; only the paper stays light. The preview and JSON screens drop the ribbon and side panels.

### Fixed
- `Splitter` panes showed nothing: their height was forced to zero.
- The report preview's back button jumped to the first page instead of the previous one.

## [26.2.1] - 2026-11-01

### Fixed
- The documentation of `ravenrs` and `ravenrs-platform-native` builds on docs.rs again. On Linux they depend on `zbus`, which the nightly compiler docs.rs uses cannot compile, so docs.rs now builds them for Windows and macOS. Code and behavior are unchanged, and Linux builds on stable are not affected.

## [26.2.0] - 2026-11-01

The first release under the `YY.R.P` versioning model. It follows 0.1.0 directly; the jump in the number marks the new model, not a long gap.

### Breaking
- Enums that will grow are now `#[non_exhaustive]`: `Event`, `Key`, `PointerButton`, `CursorIcon`, `Brush`, `Primitive`, `Semantic`, `TextStyle`, `Material`, `Axis`, `Align`, `Justify`, `Available`, `DragPhase`, `Placement`, `ButtonStyle`, `ButtonSize`, `Tone`, `SelectionMode`, `Value`, `Summary`, `CellAlign`, `ContentMode`, `RibbonSize`, `Side` and `WebError`. Matches on them need a wildcard arm; future variants will then be additions.

### Added
- **`ravenrs-criteria`**, a criteria and expression language independent of any UI:
  - A public tree that parses from text, can be built in code, and prints back to text that parses to the same tree.
  - Evaluation over any `Record` (JSON, maps, your own rows), with calculated fields, parameters, custom functions and a fixed clock for tests.
  - Over 130 functions and aggregates, in a catalog with signatures and descriptions.
  - `validate` checks fields, parameters and functions against a `Schema`.
  - `to_sql` translates to parameterized SQL for PostgreSQL, MySQL, SQL Server and SQLite.
- **RavenRS Reporting** (`ravenrs-report`, first phase; export comes next):
  - Reports defined as data: JSON layouts (`Report::from_json`) that carry their data sources, or a Rust builder.
  - Bands: report and page headers and footers, nested groups with subtotals, detail, master-detail reports.
  - Controls: label, table, check box, line, shape, panel.
  - Expressions from `ravenrs-criteria`, with aggregates over a group, a page, the report or a nested collection, and `ReportEngine::register_function`.
  - Conditional formatting, named styles, zebra rows, .NET-style formats with pt-BR and en-US cultures.
  - Pagination with growing text, keep-with-next, repeated group headers and page breaks: 10,000 rows in 80 ms natively, about 200 ms in the browser.
  - A wizard that builds a table report from a choice of fields, SVG output per page, and `Document::print_html`, a print-ready page with the fonts embedded.
- **Report Designer** (`examples/report-designer`, [live](https://alexkads.github.io/RavenRS/designer/)): template gallery, wizard, field list with drag and drop, bands, move and resize, property grid, undo and redo, instant preview, printing (the browser's dialog can also save a PDF), JSON view.
- `LayoutStyle::absolute` places a box at an offset outside the flow; `ScrollView::horizontal` scrolls sideways; `TextLayout::lines` reports line ranges and baselines.
- Screen reader support. `Ui::access_tree` describes the UI as roles, names, values and states, and `Ui::perform_action` runs what assistive technology asks for (click, focus, increment, decrement).
  - Desktop: the tree goes to VoiceOver, Narrator, NVDA and Orca through AccessKit. It is built only while a screen reader is listening.
  - Web: the tree is mirrored as invisible ARIA elements over the canvas. Focus is announced through `aria-activedescendant`, toasts through a live region, and screen reader clicks reach the UI. `WebOptions::accessibility(false)` turns it off.
  - Every built-in component describes itself, such as check boxes with their state, sliders with their range, the DataGrid with its rows and keyboard row, and dialogs as modal (the page behind them is hidden).
  - Custom components use `BuildCx::semantics` with `Semantics`.
  - New accessible names: `IconButton::label`, `Button::accessible_name` and `Image::alt`. `label` was added to `Slider`, `Stepper`, `Select`, `Combobox`, `DatePicker`, `RadioGroup` and `SegmentedControl`. `FormField` and `Tooltip` name the control inside them.
- Text selection and the clipboard in `TextField`:
  - Shift with arrows, Home and End selects; Alt or Ctrl moves and deletes by word; Cmd moves and deletes to the line ends.
  - Cmd/Ctrl+A selects all. A press places the caret, a drag selects, Shift-click extends and a double-click selects a word.
  - Cmd/Ctrl+C, X and V copy, cut and paste. Password fields accept a paste but never copy.
  - The clipboard is a runtime service, `BuildCx::clipboard`: the system clipboard on desktop and the browser's on the web.
- DataGrid: Cmd/Ctrl+C copies the selected rows (or the keyboard row) as tab-separated text that pastes into spreadsheets; Cmd/Ctrl+A selects every row.
- `Menubar` follows the WAI-ARIA menubar pattern. Left and Right move between titles, Down, Enter or Space opens a menu, Left and Right inside a menu open the neighbor menu, and Escape returns focus to the title.
- Full keyboard operation following the WAI-ARIA patterns:
  - `RadioGroup`, `SegmentedControl` and `Tabs`: arrow keys move selection and focus together, wrapping; Home and End jump to the ends.
  - `ToggleGroup`: arrow keys move focus; Space toggles.
  - `Stepper`: arrow keys step; Home and End jump to the limits.
  - Menus (`DropdownMenu`, `ContextMenu`, `Menubar`, the `Ribbon` menus) and the `Select` list: the first (or current) item takes focus on open, arrows move, Enter chooses, Escape closes, and focus returns to the trigger. Arrow keys also open a `Select`.
  - `Calendar`: the grid is one Tab stop; arrows move by day and week, Home and End to the ends of the week, Page Up and Page Down by month.
  - `Dialog`, `Sheet` and other full-window layers keep Tab inside them and return focus to where it was when they close.
- `Key::PageUp` and `Key::PageDown`.
- Documentation on every public item, a README per crate, and runnable examples.

### Fixed
- DataGrid: Page Up and Page Down move by 15 rows again; they stopped working when those keys got their own `Key` variants.
- On the web, the canvas now passes keyboard focus to the hidden text input when it is focused with Tab; the listener was never registered.
- DataGrid queries over 100,000 rows now fit in a frame: search 47 → 3.2 ms, range filter 31 → 0.9 ms, text sort 23 → 12 ms, grouping 15 → 11 ms.
- Focus moved by code keeps the focus ring while the user navigates by keyboard.
- `AspectRatio` content with modifiers (for example a styled `Spacer`) no longer collapses to zero height.
- `winit` no longer pulls in `sctk-adwaita`, removing a dependency with a security advisory (RUSTSEC-2026-0192) on Linux.

### Quality
- Accessibility tests for every component family, and an audit that opens every gallery page and fails on any unnamed control.
- GPU golden-image tests (6 scenes in dark and light), property tests, layout and keyboard tests, criterion benchmarks with budgets, and `cargo deny`, MSRV, feature and benchmark checks in `scripts/check.sh`. See [docs/QUALITY.md](docs/QUALITY.md).

## [0.1.0] - 2026-10-31

The first public release.

### Platform
- One declarative UI for native windows (Windows, macOS, Linux via winit) and the browser (WebAssembly + WebGPU on a `<canvas>`).
- GPU renderer on `wgpu` 30: SDF rounded rectangles, borders, gradients, soft shadows, images with mipmaps, glyph atlas text, rectangular clipping.
- Text with Parley and the embedded Inter font; 200 Lucide icons as an embedded icon font.
- Flexbox and grid layout on Taffy.
- Reactive state (`State`, `Binding`, `Derived`) with dependency tracking: only views that read a changed value rebuild, and idle apps draw nothing.
- Runtime services for components: widget-local state, overlays, virtualized scrolling, timers, keyboard shortcuts, drag, double and right click, focus management.
- Web: DPR-aware canvas, pointer, touch scrolling, wheel, keyboard and text input through a hidden textarea (IME commit, dead keys, paste), on-demand frames, device-loss recovery, cleanup.
- Themes: Raven Dark, Raven Light and Raven Glass from semantic design tokens, contrast-checked in tests.

### Components
- Inputs: Button, IconButton, ToggleButton, ToggleGroup, TextField, Select, Combobox, Checkbox, RadioGroup, Toggle, Slider, Stepper, SegmentedControl, InputOtp, Calendar, DatePicker, ThemeSwitcher.
- Forms: FormField with validators, Label.
- Navigation: Ribbon, Menubar, Tabs, Sidebar, Breadcrumb, Pagination, Disclosure, Accordion, CommandPalette.
- Overlays: Dialog, Sheet, Popover, HoverCard, Tooltip, DropdownMenu, ContextMenu, Toaster.
- Data: DataGrid (virtualized, multi-sort, filter row with operators, search, grouping with summaries, column resize and chooser, selection, keyboard navigation, in-place editing, cell templates, CSV export), Table, List, Stat, BarChart, AreaChart.
- Layout: VStack, HStack, Grid, ScrollView, Splitter, Card, GlassCard, Section, AspectRatio, Spacer, `responsive`, and view modifiers.
- Media and feedback: Image, Icon, Carousel, Avatar, Badge, Alert, ProgressBar, Spinner, Skeleton, Kbd, Divider, Canvas.

### Tooling
- `scripts/check.sh` runs formatting, clippy (native and wasm32), the tests and the web builds locally.
- `scripts/deploy-pages.sh` publishes the landing page, the live gallery and the API docs.

[Unreleased]: https://github.com/alexkads/RavenRS/compare/v27.1.10...HEAD
[27.1.10]: https://github.com/alexkads/RavenRS/compare/v27.1.9...v27.1.10
[27.1.3]: https://github.com/alexkads/RavenRS/compare/v27.1.2...v27.1.3
[27.1.2]: https://github.com/alexkads/RavenRS/compare/v27.1.1...v27.1.2
[27.1.1]: https://github.com/alexkads/RavenRS/compare/v27.1.0...v27.1.1
[27.1.0]: https://github.com/alexkads/RavenRS/compare/v26.2.1...v27.1.0
[26.2.1]: https://github.com/alexkads/RavenRS/compare/v26.2.0...v26.2.1
[26.2.0]: https://github.com/alexkads/RavenRS/compare/v0.1.0...v26.2.0
[0.1.0]: https://github.com/alexkads/RavenRS/releases/tag/v0.1.0
