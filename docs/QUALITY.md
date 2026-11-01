# Quality bar

RavenRS components are meant to be used in production applications, at the level users expect from commercial component vendors. This page defines what "done" means for a component, which checks enforce it, the performance budgets, and where each component stands today.

## Definition of done for a component

A component is ✅ in [PARITY.md](PARITY.md) only when all of these hold:

1. **API.** Typed builder methods with defaults that look right without configuration. Values come in through `Binding<T>`, events go out through closures. Every public item has rustdoc, and the type has a runnable example. Growable enums are `#[non_exhaustive]`.
2. **States.** Rest, hover, pressed, focused (keyboard ring), disabled, selected or checked where it applies, empty, loading and error where it applies, and overflow (long text, many items, a tiny viewport).
3. **Keyboard.** Reachable with Tab. Enter and Space activate. Arrow keys, Home and End follow the [WAI-ARIA Authoring Practices](https://www.w3.org/WAI/ARIA/apg/patterns/) pattern for that widget. Escape closes overlays.
4. **Themes.** Built only from semantic tokens, so it is correct in Raven Dark, Light and Glass. Text meets WCAG AA contrast, which `ravenrs-style` tests for every theme.
5. **Edge cases.** Empty data, a single item, huge counts, non-ASCII text, out-of-range values (clamped, never panicking) and a zero-size viewport.
6. **Performance.** Stays inside the budgets below. Large collections are virtualized.
7. **Tests.** At least one behavior test that drives it through `Ui` (pointer and keyboard). Property tests for any pure logic. A golden image for any component whose drawing is non-trivial.
8. **Docs and demo.** A section in the [component gallery](../examples/component-gallery) that shows its variants and states.

## Enforced checks

`scripts/check.sh` runs every check, and nothing ships unless it passes. The project uses no hosted CI, so this script is the gate.

| Check | What it catches |
|---|---|
| `cargo fmt --check` | Formatting drift |
| `cargo clippy -D warnings` on native and `wasm32` | Lints, including `missing_docs` on every public item |
| `cargo test --workspace` | Unit, behavior, property ([proptest](https://docs.rs/proptest)) and doc tests |
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
| DataGrid query over 100k rows: no sort | < 16 ms | 1.0 ms |
| … sort by a number | < 16 ms | 7.9 ms |
| … sort by text, then number | < 16 ms | 12.3 ms |
| … filter row range `100..200` | < 16 ms | 0.9 ms |
| … search across columns | < 16 ms | 3.2 ms |
| … group with summaries | < 16 ms | 11.3 ms |
| Shape a label / shape and wrap a paragraph | < 0.1 ms | 0.003 ms / 0.035 ms |

The DataGrid caches the query result and reruns it only when the data or the query changes, so these costs are paid once per user action, not every frame. Version 0.1 broke the budget in three cases (search at 47 ms, filter at 31 ms, text sort at 23 ms). They were fixed by parsing filters once per query, matching text case-insensitively without allocating, computing sort keys only for matching rows, and replacing text comparisons with precomputed ranks.

## Component audit

Status against the definition of done. **Behavior** names the test suites that drive the component. **Visual** marks golden-image coverage. Accessibility for screen readers (AccessKit) and right-to-left layout are missing for every component and are tracked once, in the gaps below.

| Component | Behavior tests | Visual | Keyboard | Notes |
|---|---|---|---|---|
| Button, IconButton | ui, keyboard_and_basics | ✅ | ✅ Enter/Space, tested | |
| ToggleButton, ToggleGroup | batch_parity | | 🟡 Tab + Enter/Space | Arrow keys inside the group planned |
| TextField | components, properties | ✅ | ✅ editing keys, tested; IME on web | Clipboard planned |
| Select | components | | 🟡 Tab + Enter opens | Arrow keys in the list planned |
| Combobox | batch_parity | | ✅ type, arrows, Enter, tested | |
| Checkbox, Toggle | components, batch_parity | ✅ | 🟡 Tab + Enter/Space | |
| RadioGroup, SegmentedControl | keyboard_and_basics | ✅ | ✅ arrows, Home/End, wrap, tested | Keyboard added in this pass |
| Slider | components | ✅ | ✅ arrows, Home/End, tested | |
| Stepper | keyboard_and_basics | | ✅ arrows, Home/End, tested | Keyboard added in this pass |
| InputOtp | batch_parity | | ✅ digits and Backspace, tested | |
| FormField, Label | batch_parity, keyboard_and_basics | | n/a | |
| Calendar, DatePicker | batch_parity | ✅ | 🟡 Tab + Enter per day | Arrow-key grid navigation planned |
| Ribbon | ribbon | ✅ | 🟡 Tab + Enter | Key tips planned |
| Menubar, DropdownMenu, ContextMenu | keyboard_and_basics, components, batch_parity | | 🟡 Tab + Enter, Escape | Arrow keys inside menus planned |
| Tabs, Sidebar, Breadcrumb, Pagination, Accordion, Disclosure | components, layout_and_content, batch_parity | | 🟡 Tab + Enter | Arrow keys between tabs planned |
| CommandPalette | batch_parity | | ✅ ⌘K, type, arrows, Enter, tested | |
| Dialog, Sheet, Popover, HoverCard, Tooltip, Toaster | components, batch_parity, keyboard_and_basics | | ✅ Escape, tested | Focus trap in Dialog planned |
| DataGrid | datagrid, properties | ✅ | ✅ row navigation, Enter edits, tested | See the DataGrid parity table in ARCHITECTURE.md |
| Table, List, Stat, BarChart, AreaChart | components | ✅ partial | n/a | |
| Layout: stacks, Grid, ScrollView, Splitter, AspectRatio, Card, Section | layout_and_content, components, keyboard_and_basics | ✅ partial | 🟡 Splitter arrows, untested | AspectRatio content now fills its box |
| Media and feedback: Image, Icon, Carousel, Avatar, Badge, Alert, ProgressBar, Spinner, Skeleton, Kbd, Divider | layout_and_content, components, batch_parity | ✅ partial | 🟡 Carousel arrows, untested | |

✅ means the WAI-ARIA keys for that widget work and a test covers them. 🟡 means it is reachable and activatable but the full pattern is missing or untested.

Defects found and fixed during this audit:
- RadioGroup, SegmentedControl and Stepper had no keyboard support beyond Tab.
- Focus moved by code hid the focus ring even when the user was navigating by keyboard.
- `AspectRatio` content with modifiers (for example `Spacer::new().background(...)`) collapsed to zero height.
- The DataGrid query exceeded the frame budget on 100k rows (see above).

## Known gaps

These block a "commercial-grade" label and are prioritized ahead of new components of the same family:

1. **Screen readers.** No AccessKit tree yet, so no component exposes roles or names to assistive technology.
2. **Right-to-left layout.** Parley shapes bidirectional text, but layouts do not mirror.
3. **Full keyboard patterns** for the 🟡 rows above: arrow keys in menus, the Select list, tabs, toggle groups and the calendar grid; a focus trap inside dialogs.
4. **Clipboard** in `TextField` and the DataGrid.
5. **Visual coverage.** Golden images exist for 6 scenes. Overlays, navigation and charts have none yet.
