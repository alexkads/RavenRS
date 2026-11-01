# Changelog

All notable changes to RavenRS are listed here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/). From 26.2.0 on, versions are `YY.R.P`: breaking changes only in each year's `.1` release, additions in `.2`, fixes in maintenance releases, always valid Cargo SemVer. See [docs/VERSIONING.md](docs/VERSIONING.md).

## [Unreleased]

## [26.2.0] - unreleased

The first release under the `YY.R.P` versioning model. It follows 0.1.0 directly; the jump in the number marks the new model, not a long gap.

### Breaking
- Enums that will grow are now `#[non_exhaustive]`: `Event`, `Key`, `PointerButton`, `CursorIcon`, `Brush`, `Primitive`, `Semantic`, `TextStyle`, `Material`, `Axis`, `Align`, `Justify`, `Available`, `DragPhase`, `Placement`, `ButtonStyle`, `ButtonSize`, `Tone`, `SelectionMode`, `Value`, `Summary`, `CellAlign`, `ContentMode`, `RibbonSize`, `Side` and `WebError`. Matches on them need a wildcard arm; future variants will then be additions.

### Added
- Keyboard support following WAI-ARIA in `RadioGroup` and `SegmentedControl` (arrow keys move selection and focus, wrapping; Home and End) and in `Stepper` (arrow keys step; Home and End jump to the limits).
- Documentation on every public item, a README per crate, and runnable examples.

### Fixed
- DataGrid queries over 100,000 rows now fit in a frame: search 47 → 3.2 ms, range filter 31 → 0.9 ms, text sort 23 → 12 ms, grouping 15 → 11 ms.
- Focus moved by code keeps the focus ring while the user navigates by keyboard.
- `AspectRatio` content with modifiers (for example a styled `Spacer`) no longer collapses to zero height.
- `winit` no longer pulls in `sctk-adwaita`, removing a dependency with a security advisory (RUSTSEC-2026-0192) on Linux.

### Quality
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

[Unreleased]: https://github.com/alexkads/RavenRS/compare/v0.1.0...HEAD
[26.2.0]: https://github.com/alexkads/RavenRS/compare/v0.1.0...v26.2.0
[0.1.0]: https://github.com/alexkads/RavenRS/releases/tag/v0.1.0
