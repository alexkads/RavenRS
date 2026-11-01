# Changelog

All notable changes to RavenRS are listed here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/). From 26.2.0 on, versions are `YY.R.P`: breaking changes only in each year's `.1` release, additions in `.2`, fixes in maintenance releases, always valid Cargo SemVer. See [docs/VERSIONING.md](docs/VERSIONING.md).

## [Unreleased]

### Added
- **Bug reports from the diagnostics panel.** `WindowOptions::new_issue_url` adds a button that opens a new issue in the browser with the report filled in, cut to a safe length, for the person to review and send. Nothing is sent by the app, and the panel says what the report has before it opens. `ravenrs::native::open_url` opens a web page, and `ravenrs_core::new_issue_url` and `redact_user_paths` are the plain functions behind the button.
- **Diagnostics on the desktop.** Ctrl+Shift+D (Cmd+Shift+D on a Mac) or `RAVENRS_DIAG=1` opens a panel in any app with the GPU in use, GPUs left out for their drivers, what is wrong and what recent frames cost, with a report to copy into a bug report. `ravenrs::native::diagnose` gives the same GPU report without showing a window. Closed, the panel costs nothing. `WindowOptions::labels` sets the words of the whole UI, this panel included.
- **Expressions in the grid's filter row.** A `DataGrid` filter can be a `ravenrs-criteria` expression over column titles, such as `[Seats] > 10 And [Country] = 'Brazil'`. The short syntax (`>=15`, `5..12`, `sim`, plain text) works as before.
- `ravenrs_criteria::NumberStyle` lays out digits with any separators and affixes; `Culture` and the grid's `NumberFormat` both use it.
- `ravenrs_core::csv::push_field` quotes a CSV field; the grid and report exports both use it.
- **Charts in the UI.** `Chart` draws line, area, bar, pie, doughnut, scatter and sparkline charts with the layout reports use. The shapes become a picture at the screen's density and the labels are real text. Screen readers hear the values.
- **Barcodes in the UI.** `Barcode` shows a QR Code, Code 128, Code 39, EAN-13, EAN-8, UPC-A or Interleaved 2 of 5 with every module on whole pixels. Text a format cannot hold shows why.
- `Paint::Rich` draws with a `PaintCx`, for views that shape text while painting or need the device scale.
- `charts` feature on `ravenrs` and `ravenrs-widgets`, on by default. `ravenrs` also forwards `decode`.
- `ChartKind::SparkArea`, a smoothed line over a filled area, and `ChartOptions` fields for colors, bar corners and the widest bar.
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

[Unreleased]: https://github.com/alexkads/RavenRS/compare/v27.1.1...HEAD
[27.1.1]: https://github.com/alexkads/RavenRS/compare/v27.1.0...v27.1.1
[27.1.0]: https://github.com/alexkads/RavenRS/compare/v26.2.1...v27.1.0
[26.2.1]: https://github.com/alexkads/RavenRS/compare/v26.2.0...v26.2.1
[26.2.0]: https://github.com/alexkads/RavenRS/compare/v0.1.0...v26.2.0
[0.1.0]: https://github.com/alexkads/RavenRS/releases/tag/v0.1.0
