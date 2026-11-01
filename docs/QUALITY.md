# Quality bar

RavenRS components are meant to be used in production applications, at the level users expect from commercial component vendors. This page defines what "done" means for a component, which checks enforce it, the performance budgets, and where each component stands today.

## Definition of done for a component

A component is ✅ in [PARITY.md](PARITY.md) only when all of these hold:

1. **API.** Typed builder methods with defaults that look right without configuration. Values come in through `Binding<T>`, events go out through closures. Every public item has rustdoc, and the type has a runnable example. Growable enums are `#[non_exhaustive]`.
2. **States.** Rest, hover, pressed, focused (keyboard ring), disabled, selected or checked where it applies, empty, loading and error where it applies, and overflow (long text, many items, a tiny viewport).
3. **Screen readers.** It sets a role and state with `Semantics`, and every control has a name: from its text, a `label`, a `FormField` or a `Tooltip`. The gallery audit (`examples/component-gallery/tests/accessibility.rs`) fails on any unnamed control.
4. **Keyboard.** Reachable with Tab. Enter and Space activate. Arrow keys, Home and End follow the [WAI-ARIA Authoring Practices](https://www.w3.org/WAI/ARIA/apg/patterns/) pattern for that widget. Escape closes overlays.
5. **Themes.** Built only from semantic tokens, so it is correct in Raven Dark, Light and Glass. Text meets WCAG AA contrast, which `ravenrs-style` tests for every theme.
6. **Edge cases.** Empty data, a single item, huge counts, non-ASCII text, out-of-range values (clamped, never panicking) and a zero-size viewport.
7. **Performance.** Stays inside the budgets below. Large collections are virtualized.
8. **Tests.** At least one behavior test that drives it through `Ui` (pointer and keyboard). Property tests for any pure logic. A golden image for any component whose drawing is non-trivial.
9. **Docs and demo.** A section in the [component gallery](../examples/component-gallery) that shows its variants and states.

## Enforced checks

`scripts/check.sh` runs every check, and nothing ships unless it passes. The project uses no hosted CI, so this script is the gate.

| Check | What it catches |
|---|---|
| `cargo fmt --check` | Formatting drift |
| `cargo clippy -D warnings` on native and `wasm32` | Lints, including `missing_docs` on every public item |
| `cargo test --workspace` | Unit, behavior, accessibility, property ([proptest](https://docs.rs/proptest)) and doc tests |
| GPU golden-image tests (`crates/ravenrs/tests/visual.rs`) | Visual regressions in 6 scenes × dark and light; tolerance 24/255 per channel on at most 0.4% of pixels |
| GPU pixel tests (`crates/ravenrs-render/tests/offscreen.rs`) | Renderer correctness: shapes, clipping, images |
| `cargo doc -D warnings` | Broken intra-doc links |
| `cargo deny check` | Vulnerable, yanked or unlicensed dependencies, and unknown sources |
| `cargo +1.88 check` | The declared minimum Rust version (MSRV) still builds |
| `--no-default-features` | Optional features really are optional |
| `cargo bench --no-run` | Benchmarks keep compiling |
| Web builds | The examples build and bind for WebAssembly |

GPU tests skip themselves, with a message, on machines without a GPU adapter. To accept an intended visual change, regenerate the golden images with `RAVENRS_UPDATE_GOLDEN=1 cargo test -p ravenrs --test visual`, then review the PNG diff in the commit. A failing run writes `*.actual.png` next to the golden image.

Release checks, run by hand before each publish:
- `cargo semver-checks` against the previous release.
- `cargo llvm-cov` for coverage, at 75% line coverage as of 0.1. Coverage is a signal, not a target.

## Performance budgets

Measured with `cargo bench` (criterion) on an Apple M-series laptop, release profile. Budgets assume a 60 Hz frame (16.7 ms).

| Benchmark | Budget | Measured |
|---|---|---|
| Full frame: form with 40 cards (build + layout + paint) | < 8 ms | 1.6 ms |
| Full frame: DataGrid with 100,000 rows | < 8 ms | 0.5 ms |
| Animation frame: the same form, paint phase only | < 2 ms | 0.11 ms |
| DataGrid query over 100k rows: no sort | < 16 ms | 1.0 ms |
| … sort by a number | < 16 ms | 7.9 ms |
| … sort by text, then number | < 16 ms | 12.3 ms |
| … filter row range `100..200` | < 16 ms | 0.9 ms |
| … filter row expression over three conditions | < 16 ms | 8.6 ms |
| … search across columns | < 16 ms | 3.2 ms |
| … group with summaries | < 16 ms | 11.3 ms |
| Shape a label / shape and wrap a paragraph | < 0.1 ms | 0.003 ms / 0.035 ms |
| Report: grouped sales over 10,000 rows, 305 pages | < 300 ms | 80 ms native, about 200 ms in the browser |
| Report: invoice | < 5 ms | 0.05 ms |

Charts are pictures made on the CPU, so their cost is paid when the data or the size changes, and not on other frames: a rebuild that produces the same shapes keeps the same texture. Measured in WebAssembly on Node 24 with the `-Oz` profile, without SIMD, on the same laptop:

| Chart | Median |
|---|---|
| 12 bars, 760x260 at 2x | 4.3 ms |
| Line with 12 points, 760x260 at 2x | 6.1 ms |
| Pie with 8 slices, 360x260 at 2x | 5.3 ms |
| `AreaChart`, 14 values, 760x200 at 2x | 10.8 ms |
| Line with 12 points, 1600x900 at 2x | 22.2 ms |
| Line with 1000 points, 760x260 at 2x | 55.8 ms |
| QR Code, 160 at 2x | 2.1 ms |

Dragging a window's edge redraws every visible chart on each frame, so more than a few at once, or a long series, go over 16.7 ms while it lasts.

The size of a WebAssembly build, after `wasm-opt -Oz`:

| Build | Bytes | Gzipped |
|---|---|---|
| `hello-world`, `charts` off | 2,805,367 | 1,202,999 |
| `hello-world`, `charts` on, not using it | 2,806,628 | 1,203,232 |
| `component-gallery` before charts used it | 3,301,057 | 1,399,745 |
| `component-gallery` using every chart and code | 3,582,964 | 1,504,909 |

Link-time optimization drops what an app does not call, so the feature costs an app without charts about 0.2 KB gzipped. Turning it off saves compile time, not download size. An app that does use charts pays about 105 KB gzipped, its own demos included.

The DataGrid caches the query result and reruns it only when the data or the query changes, so these costs are paid once per user action, not every frame. Version 0.1 broke the budget in three cases (search at 47 ms, filter at 31 ms, text sort at 23 ms). They were fixed by parsing filters once per query, matching text case-insensitively without allocating, computing sort keys only for matching rows, and replacing text comparisons with precomputed ranks.

## Component audit

Status against the definition of done. **Behavior** names the test suites that drive the component. **Visual** marks golden-image coverage. Every component sets screen reader semantics, covered by `accessibility` (widgets) and the gallery audit. Right-to-left layout is missing for every component and is tracked once, in the gaps below.

| Component | Behavior tests | Visual | Keyboard | Notes |
|---|---|---|---|---|
| Button, IconButton | ui, keyboard_and_basics | ✅ | ✅ Enter/Space, tested | |
| ToggleButton, ToggleGroup | batch_parity, keyboard_and_basics | | ✅ arrows move focus, Space toggles, tested | |
| TextField | components, properties, editing_and_clipboard | ✅ | ✅ editing, word and line moves, Shift selection, select all, copy/cut/paste, mouse drag and double-click, tested; IME on web | Passwords paste but never copy |
| Select | components, keyboard_and_basics | | ✅ Enter/Space/arrows open on the current option, arrows, Enter, focus returns, tested | |
| Combobox | batch_parity | | ✅ type, arrows, Enter, tested | |
| Checkbox, Toggle | components, batch_parity, keyboard_and_basics | ✅ | ✅ Space, tested | |
| RadioGroup, SegmentedControl | keyboard_and_basics | ✅ | ✅ arrows, Home/End, wrap, tested | Keyboard added in this pass |
| Slider | components | ✅ | ✅ arrows, Home/End, tested | |
| Stepper | keyboard_and_basics | | ✅ arrows, Home/End, tested | Keyboard added in this pass |
| InputOtp | batch_parity | | ✅ digits and Backspace, tested | |
| FormField, Label | batch_parity, keyboard_and_basics | | n/a | |
| Calendar, DatePicker | batch_parity, keyboard_and_basics | ✅ | ✅ one Tab stop; arrows, Home/End, Page Up/Down, Enter, tested | |
| EventCalendar | event_calendar | ✅ | ✅ one Tab stop, shared with Calendar; arrows, Home/End, Page Up/Down, Enter, tested | A day's name and description read out its events, tested |
| Scheduler | scheduler | ✅ | ✅ events are Tab stops; arrows move, Shift resizes, Enter opens, Delete removes, tested | Grid, rows and events named, with dates, tested. Pointer: drag, resize, draw, tested |
| RecurrenceEditor | event_calendar | ✅ | ✅ made of tested controls | Says the rule and its next days in words, tested |
| Ribbon | ribbon | ✅ | 🟡 Tab + Enter; its menus are fully keyboard-operable | Key tips planned |
| DropdownMenu, ContextMenu, Menubar | keyboard_and_basics, components, batch_parity | | ✅ first item focused on open, arrows, Enter, Escape, focus returns; Menubar Left/Right between titles and open menus, tested | |
| Tabs | components, layout_and_content, keyboard_and_basics | | ✅ arrows open tabs, Home/End, tested | |
| Sidebar | sidebar, layout_and_content, components | | ✅ Cmd/Ctrl+B, Tab + Enter, Up/Down/Home/End between rows, Left/Right fold submenus and groups, Escape closes the sheet, tested | `navigation` landmark; rail keeps names and shows tooltips on hover and on keyboard focus, tested. Width eases and is instant under reduced motion. Sheet goes by the window width, not its container's |
| Breadcrumb, Pagination, Accordion, Disclosure | components, layout_and_content, batch_parity | | 🟡 Tab + Enter | Links and buttons by pattern; untested |
| CommandPalette | batch_parity | | ✅ ⌘K, type, arrows, Enter, tested | |
| Dialog, Sheet, Popover, HoverCard, Tooltip, Toaster | components, batch_parity, keyboard_and_basics | | ✅ Escape; Dialog and Sheet trap focus and return it, tested | |
| SplashScreen | splash | | ✅ no focus stop: takes no input, focus leaves the page and returns on close, tested | Status and progress announced, tested |
| DataGrid | datagrid, properties, editing_and_clipboard | ✅ | ✅ row navigation, Page Up/Down, Enter edits, Cmd/Ctrl+C copies rows as tab-separated text, Cmd/Ctrl+A, tested | See the DataGrid parity table in ARCHITECTURE.md |
| Table, List, Stat | components | ✅ partial | n/a | |
| Chart, BarChart, AreaChart, Barcode | charts, components | ✅ a chart reads its values; a code reads its text or `alt` | n/a | The layout's own tests are in `ravenrs-graphics` |
| Layout: stacks, Grid, ScrollView, Splitter, AspectRatio, Card, Section | layout_and_content, components, keyboard_and_basics | ✅ partial | 🟡 Splitter arrows, untested | AspectRatio content now fills its box |
| Footer | footer | | ✅ no focus stop of its own; its links are buttons (Tab + Enter), tested | `contentinfo` landmark, tested. Stacks by window width, not by its container's |
| StatusBar | status_bar | | ✅ no focus stop of its own; its actions are buttons (Tab + Enter), tested | `contentinfo` landmark; the center is a `status` |
| Timeline, ToolRail | timeline | | 🟡 blocks, markers and tools work with Tab + Enter; moving and trimming need the pointer, so apps offer them in fields too | Blocks and markers are named buttons |
| ReleaseNotes, CodeBlock | release_notes | | ✅ versions and buttons work with Tab + Enter, tested | Lists its versions as buttons; the copy button says when it copied |
| MoviePlayer, MovieEditor, BugReporter | ravenrs-movie | | ✅ controls, chapters, marks and blocks work with Tab + Enter; dragging on the canvas and the timeline has no keyboard way yet: 🟡 | Marks say what they teach; a film is an image with a name |
| Media and feedback: Image, Icon, Carousel, Avatar, Badge, Alert, ProgressBar, Spinner, Skeleton, Kbd, Divider | layout_and_content, components, batch_parity | ✅ partial | 🟡 Carousel arrows, untested | |

✅ means the WAI-ARIA keys for that widget work and a test covers them. 🟡 means it is reachable and activatable but the full pattern is missing or untested.

Defects found and fixed during this audit:
- RadioGroup, SegmentedControl and Stepper had no keyboard support beyond Tab. Tabs, ToggleGroup, every menu, the Select list and the Calendar grid lacked their arrow-key patterns, and Calendar days could not take focus at all.
- Tab could leave an open Dialog or Sheet for the page behind it; focus now stays inside and returns to the opener on close.
- Focus moved by code hid the focus ring even when the user was navigating by keyboard.
- `AspectRatio` content with modifiers (for example `Spacer::new().background(...)`) collapsed to zero height.
- The DataGrid query exceeded the frame budget on 100k rows (see above).

## Known gaps

These block a "commercial-grade" label and are prioritized ahead of new components of the same family:

1. **Right-to-left layout.** Parley shapes bidirectional text, but layouts do not mirror.
2. **Remaining keyboard patterns** in the 🟡 rows above: Ribbon key tips.
3. **Visual coverage.** Golden images exist for 6 scenes. Overlays, navigation and charts have none yet.
4. **Screen reader depth.** Text fields do not expose caret position or selection, charts expose only their labels, and the web mirror is rebuilt whole when the tree changes. Tested with macOS VoiceOver's API (System Events) and the browser's DOM; not yet with NVDA, Narrator or Orca.
