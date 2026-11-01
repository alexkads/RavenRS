# RavenRS Architecture

RavenRS is a declarative GUI framework written in Rust. One UI description runs in a native window (Windows, macOS, Linux) and in the browser (WebAssembly + WebGPU), and RavenRS draws every pixel itself with `wgpu`. This document records the decisions behind the first milestone, why they were made, and what is implemented, experimental or planned.

## 1. Goals and constraints

| Principle | What it means in practice |
|---|---|
| Rust-first | All framework logic is Rust. No code from the original Raven GUI (C) is used. |
| GPU-first | Rendering goes through `wgpu` and WGSL shaders on every platform. |
| WebAssembly-first | Every crate that is not platform glue compiles for `wasm32-unknown-unknown`. No threads, no `SharedArrayBuffer`, no blocking. |
| Canvas-first | On the web the UI is drawn into one `<canvas>`. The DOM hosts the canvas and delivers input, nothing else. |
| Declarative-first | Views are small values composed with builders; state changes trigger rebuilds of what depends on them. |
| Beautiful by default | Components read semantic design tokens; nothing is styled by hand in app code unless the app wants to. |
| Performance-first | Frames are produced only on demand; buffers and allocations are reused; there is no GPU→CPU readback in normal rendering. |

## 2. Ecosystem survey

The survey was done against the official repositories and docs (October 2026). Nothing was copied from these projects; they informed design choices only.

| Project | License | Rendering | State model | Layout | Text | Web | Lesson for RavenRS |
|---|---|---|---|---|---|---|---|
| [Raven](https://github.com/oscar-soirey/Raven) (C) | Apache-2.0 | Vulkan behind a function table, separate render thread | UI thread + job system | Two-pass measure/arrange | unverified | none | Keep the renderer behind a narrow interface. *This is the closest match to "the original Raven GUI"; the identification is probable, not confirmed.* |
| [GPUI](https://github.com/zed-industries/zed/tree/main/crates/gpui) | Apache-2.0 | Metal; wgpu on Linux | Entities owned by the app, handles + contexts | Taffy | Platform text stacks | Experimental `gpui_web` | Context-passing ownership avoids `Rc<RefCell>` sprawl. |
| [Xilem / Masonry](https://github.com/linebender/xilem) | Apache-2.0 | Vello on wgpu | View tree diffed into a retained widget tree | Masonry's own | Parley | DOM backend (`xilem_web`) | Separating the API layer from a retained widget core lets several API styles share it. |
| [Iced](https://github.com/iced-rs/iced) | MIT | wgpu, tiny-skia fallback | The Elm Architecture | Own | cosmic-text (fork) | Yes, WebGL feature | Bundle a font for WASM; the browser gives you no system fonts. |
| [Slint](https://github.com/slint-ui/slint) | GPL-3.0 / royalty-free / commercial | femtovg, Skia, software, optional wgpu | Compiled DSL with property bindings | Own + Taffy | Parley (optional) | Yes | Trim text features per target to keep WASM small. |
| [Blinc](https://github.com/project-blinc/Blinc) | Apache-2.0 | wgpu, SDF primitives, glass | Signals + state machines | Flexbox builder | Own atlas | WebGPU preview | Closest prior art to RavenRS; validates the SDF + atlas approach. |
| [Vello](https://github.com/linebender/vello) | Apache-2.0 OR MIT | Compute-shader 2D renderer; `vello_cpu`, `vello_gpu` variants | — | — | — | Needs WebGPU compute | Do not make compute-only rendering the only path. |
| [Taffy](https://github.com/DioxusLabs/taffy) | MIT | — | — | Flexbox, Grid, Block | Measure callback | Yes | Wire text into layout through the measure function from day one. |
| [wgpu](https://github.com/gfx-rs/wgpu) | MIT OR Apache-2.0 | Vulkan, Metal, DX12, GL, WebGPU | — | — | — | WebGPU and WebGL2 | Pin the major version; breaking releases arrive roughly quarterly. |
| [Parley](https://github.com/linebender/parley) | Apache-2.0 OR MIT | — | — | — | HarfRust shaping, Skrifa outlines, ICU4X | Yes | Mature rich-text layout without platform dependencies. |

## 3. Crate graph

```
ravenrs-core ─────────────┬──────────────┬──────────────┬─────────────┐
 (geometry, color,      │              │              │             │
  events, Scene)        ▼              ▼              ▼             ▼
                  ravenrs-layout    ravenrs-text     ravenrs-style   ravenrs-render
                   (Taffy)        (Parley)       (tokens)      (wgpu, swash)
ravenrs-reactive ───┐      │              │              │             │
 (State, Binding, │      └──────┬───────┴──────────────┘             │
  Derived)        └────► ravenrs-widgets                               │
                         (View, Component, BuildCx, Ui)              │
                                │                                    │
                ┌───────────────┴───────────────┐                    │
                ▼                               ▼                    │
     ravenrs-platform-native             ravenrs-platform-web ◄──────────┘
       (winit + wgpu)              (canvas + WebGPU, wasm32 only)
                └───────────────┬───────────────┘
                                ▼
                             ravenrs  (facade + prelude)

ravenrs-criteria  (expression language, no RavenRS dependencies)
        ▲
        │
ravenrs-report  ──► ravenrs-core, ravenrs-text  (no GPU, no window)
        ▲
        │
examples/report-designer  ──► ravenrs (the UI)
```

`ravenrs-criteria` (the expression language) depends on nothing else in RavenRS, so any module can use it for filters, rules and validation. `ravenrs-report` (the report engine) depends only on `ravenrs-core`, `ravenrs-text` and `ravenrs-criteria`, so it also runs on servers. The `ravenrs` facade re-exports neither of them; apps depend on them directly. See [§6.3](#63-criteria-language), [§6.4](#64-reporting) and [docs/REPORTING.md](docs/REPORTING.md).

There are no cycles. `ravenrs-render` also uses `ravenrs-text`, and the platform crates are where widgets and the renderer come together. `ravenrs-widgets` never touches the GPU and `ravenrs-render` knows nothing about widgets; they meet only through `ravenrs_core::Scene`, a plain display list. That boundary keeps widgets testable without a GPU (the widget tests run headless) and keeps the renderer replaceable.

## 3.1 Where things live

| Crate | Modules |
|---|---|
| `ravenrs-core` | `geometry`, `color`, `event`, `scene` (the display list), `access` (the accessibility tree) |
| `ravenrs-reactive` | `state`, `binding`, `derived`, `observer` |
| `ravenrs-render` | `renderer` (batching and passes), `gpu` (surface and device), `atlas` (glyphs), `images` |
| `ravenrs-platform-native` | `lib` (winit loop), `access` (AccessKit) |
| `ravenrs-platform-web` | `app` (canvas, events, frames), `access` (ARIA mirror), `error` |
| `ravenrs-criteria` | `ast`, `parse`, `format`, `eval`, `functions`, `validate`, `sql`, `value` |
| `ravenrs-report` | `model`, `engine`, `document`, `paint`, `wizard`, `samples` |

`ravenrs-widgets` has two parts. The runtime is `view`, `context` (`BuildCx`), `ui` and `ui/access`, `modifiers`, `clipboard` and `util`. The components are grouped by family:

| Module | Components |
|---|---|
| `stack`, `card`, `canvas`, `fill`, `text`, `icon`, `icon_set`, `image` | Layout primitives, cards, custom drawing, text, icons, images |
| `button`, `inputs`, `text_field`, `choice`, `calendar`, `date` | Buttons, form controls, text input, `ToggleGroup`, `Pagination`, `Label`, `AspectRatio`, `Calendar`, `DatePicker` |
| `nav`, `ribbon` | `Tabs`, `Sidebar`, `Breadcrumb`, `Disclosure`, `Ribbon` |
| `overlay`, `floating` | `Tooltip`, `Popover`, `DropdownMenu`, `Select`, `Dialog`, then `ContextMenu`, `HoverCard`, `Sheet`, `Toaster`, `Combobox`, `CommandPalette` |
| `display`, `data`, `extras` | Badges and alerts; lists, tables and charts; `Spinner`, `Carousel`, `Splitter`, `Menubar`, `InputOtp`, `FormField` |
| `grid/` | `DataGrid`: `query` (pure pipeline), `model`, `view` |

Three examples use the public API: `hello-world`, `component-gallery` (one demo per component) and `report-designer`. The design tokens are defined in [docs/design-system](docs/design-system), and `ravenrs-style` mirrors them value for value.

## 4. The frame pipeline

```
input event ──► Ui::handle_event ──► hit test ──► handler ──► State::set
                                                                  │
                         Observer marks the UI dirty, calls the waker
                                                                  ▼
platform schedules one frame (winit request_redraw / requestAnimationFrame)
                                                                  ▼
Ui::update:  build (views → elements, tracked)  ──►  layout (Taffy)  ──►  paint (Scene)
                                                                  ▼
Renderer::render:  batch quads/glyphs ──► upload instances ──► one render pass ──► present
```

* **Build** walks the view tree. Each `View::build` creates layout boxes and paint instructions through `BuildCx`. The whole build runs inside a reactive `Observer`, so every `State` read is recorded.
* **Layout** runs Taffy over the boxes. Text leaves are measured by Parley through Taffy's measure callback, so wrapping participates in flexbox sizing.
* **Paint** walks the element tree in order and appends primitives (`Quad`, `GlyphRun`) to the `Scene`. Focus rings are painted after the focused element's children so they sit on top.
* **Render** turns the scene into instanced draws. Consecutive primitives of the same kind are merged into one draw call; painter's order is preserved.

Nothing runs between frames. An idle app sleeps on the desktop (`ControlFlow::Wait`) and schedules no animation frames on the web.

## 5. Reactive system

`ravenrs-reactive` is single-threaded by design: `Rc`, `Cell`, `RefCell`, no locks. UI state lives on the UI thread on every platform, including the browser main thread.

* `State<T>` owns a value. `get`/`with` record a read in the innermost tracking `Observer`; `set`/`update` notify only the observers that read it during their latest run.
* `Binding<T>` is a read/write view of a value owned elsewhere (SwiftUI's `@Binding`). `Binding::map` projects onto a field.
* `Derived<T>` is a lazily recomputed, cached value over other state (a memo). It notifies its own readers when its inputs change.
* `Observer` runs a closure and calls back once when any dependency changes.

**Dependency bookkeeping without back-references.** Each observer run increments an *epoch*. A source stores `(Weak<observer>, epoch)` pairs; on notification, entries from older epochs or dropped observers are discarded. This keeps ownership one-directional (sources never keep observers alive), makes re-tracking O(reads), and makes dropping either side safe at any time.

**What is rebuilt.** In this milestone the `Ui` has one observer for the whole build, so a change to any state the UI read triggers a full rebuild + relayout + repaint, and a change to state the UI never read triggers nothing (covered by `unrelated_state_does_not_invalidate`). Per-component observers and incremental reconciliation (rebuilding only the subtrees whose dependencies changed, reusing their layout nodes) are the next step (Phase 3); the `WidgetId` identity scheme below is already what that needs.

**Identity.** Every view gets a `WidgetId` derived from its parent's id and its position (or an explicit key via `BuildCx::keyed_child`). Hover, press and focus survive rebuilds because they are stored per `WidgetId`, not per element.

## 6. Extensibility: making your own components

Components can be created at three levels, and the built-in widgets use the same public API as apps:

1. **Functions** returning `impl View` for stateless composition.
2. **`Component`**: a struct that holds its state and implements `body() -> impl View`. Every `Component` is a `View` through a blanket implementation.
3. **`View`** implemented directly, with full access to `BuildCx`:
   * `cx.child(&view)`, `cx.children(&seq)`, `cx.keyed_child(key, &view)` to build children;
   * `cx.container(LayoutStyle, &children, Paint)` and `cx.text(...)` to create layout boxes;
   * `Paint::Quad(QuadPaint)` or `Paint::Custom(DrawFn)` to draw anything into the `Scene`;
   * `cx.on_click`, `cx.focusable`, `cx.cursor` for behavior;
   * `cx.is_hovered()`, `cx.is_pressed()`, `cx.is_focused()`, `cx.theme()` to style by state and tokens.

`Canvas` wraps `Paint::Custom` for custom drawing inside normal layout, `view_fn` turns a closure into a reactive view, and `Option<V>` renders conditionally.

## 6.1 Runtime services for components

Rich components need more than layout and paint; the runtime provides them generically, so third-party components get the same powers as built-in ones:

* **Widget-local state.** `cx.use_state(init)` returns a `State<T>` owned by that widget instance, keyed by its `WidgetId` and call order. It survives rebuilds and is dropped when the widget is no longer built. `Keyed::new(key, view)` resets it on purpose.
* **Overlays.** `cx.overlay(anchor, placement, &view, on_dismiss)` builds a floating layer after the main tree. It is laid out as its own Taffy root, placed relative to the anchor's final frame (flipped and clamped to the viewport), painted last, hit-tested first, and dismissed on outside clicks or Escape.
* **Scrolling and clipping.** `cx.scrollable(node)` makes a vertical scroll container (`cx.scroll_horizontally` adds the other axis, which `DataGrid` uses): offsets are kept per `WidgetId`, wheel events move the innermost scrollable under the pointer, children are clipped with scissor rectangles, and a thin scrollbar is drawn. Scrolling only re-places and repaints; it does not rebuild or re-run layout.
* **Input.** `cx.on_key` (focused element first, default handling otherwise), `cx.on_drag` (start/move/end with the element frame), `cx.on_click`, `cx.focusable`, `cx.cursor`, and `cx.is_hover_within()` for hover over a subtree.
* **Clipboard.** `cx.clipboard()` reads and writes the system clipboard: `arboard` on desktop, `navigator.clipboard` on the web (where reading is asynchronous, so a paste arrives through the browser's `paste` event as typed text). `Ui::set_clipboard` swaps the provider; tests use an in-memory one.
* **Theme as state.** The theme is a `State<Theme>`; `ThemeSwitcher` or any component can change it through `cx.theme_state()` and the whole UI rebuilds with the new tokens.
* **Icons.** A subset of Lucide (ISC), about 200 icons in 68 KB, is embedded as a font, so icons are glyphs: sharp at any size, colored like text, and batched with text. `scripts/gen-icons.py` regenerates the subset and the `IconName` enum.

## 6.2 DataGrid

`DataGrid` is split in two layers. `grid::query` is a pure pipeline over rows (filter → search → sort → group → summarize) with no UI dependency: it is unit-tested on its own, including a 100,000-row sort-and-group run. The view caches the query result per (query, data version) and builds only the rows inside the viewport plus a small overscan, with spacers above and below; scrolling rebuilds just that window (`BuildCx::scrollable_virtual`, `scroll_viewport`, `set_scroll_offset`). Data lives in `GridData`, shared copy-on-write so edits invalidate the cache exactly once.

Parity with the DevExpress DataGrid feature set:

| Feature | RavenRS |
|---|---|
| Virtual scrolling (100k+ rows), vertical and horizontal | ✓ |
| Sorting, multi-column (Shift/Ctrl-click), with priority badges | ✓ |
| Filter row with operators (`>`, `>=`, `<`, `<=`, `=`, `<>`, `a..b`), text and yes/no | ✓ |
| Search panel across visible columns | ✓ |
| Grouping, nested, expand/collapse (click or arrows), group panel chips | ✓ (from the Group menu; drag-to-group planned) |
| Group summaries and footer totals (sum, average, min, max, count) | ✓ |
| Column resizing (drag), column chooser (show/hide), min widths | ✓ |
| Selection: single, multiple with check boxes, Shift ranges, Ctrl toggle, select all | ✓ |
| Keyboard navigation (arrows, Home/End, Page Up/Down, Space, Enter/F2) with auto-scroll | ✓ |
| Cell editing (double-click, Enter, F2; Enter commits, Escape cancels), typed parsing | ✓ |
| Cell templates (any view) and custom formats; number/currency formats with separators | ✓ |
| CSV export (RFC 4180, raw numbers) | ✓ |
| Copying rows as tab-separated text (Cmd/Ctrl+C) | ✓ |
| Column reordering by drag, drag-to-group | planned |
| Fixed (frozen) columns, multi-level (banded) headers | planned |
| Header filter pop-ups (value checklists), filter builder | planned |
| Master-detail rows, paging UI, row add/delete UI, validation | planned |
| Excel export, state persistence, context menus, cell range copy | planned |

## 6.3 Criteria language

`ravenrs-criteria` is one expression language for filters, report totals, rules and SQL `WHERE` clauses, modeled on the behavior of the DevExpress Criteria Language. It knows nothing about reports or UI, and nothing specific to a module belongs in it.

* **Tree.** `Criteria` is public: parse it from text (`parse`), build it in code, walk it, and print it back to text that parses to the same tree (`format`).
* **Evaluation.** `Scope` interprets the tree in memory over any `Record`, such as JSON, a map or your own rows. `Environment` holds parameters and custom functions.
* **Function catalog.** One table in `functions` drives parsing, arity checks and expression editors.
* **Validation.** `validate` checks an expression against a `Schema` before anything runs.
* **SQL.** `to_sql` translates to parameterized SQL per `Dialect`.
* **Formatting.** `format_value` and `format_composite` use .NET-style format strings per `Culture`.

`DataGrid` still parses its own filter syntax in `grid::query`; moving it onto this crate is planned.

## 6.4 Reporting

`ravenrs-report` turns a definition plus data into a paginated document. Layout happens once, and every consumer draws the same result, so a preview and an export cannot drift apart.

```
Report (model: JSON or builder) + data ──► ReportEngine (engine) ──► Document (pages in points)
                                                                         │
                                               paint_page ──► Scene ──► on-screen preview
```

Expressions in a report come from `ravenrs-criteria`, and text is measured with the same Parley stack as the UI. `wizard` builds a finished table report from a data source, and `samples` holds the templates. The designer is `examples/report-designer`, built on `Ribbon` and the other public components: `editor` (shared state), `surface` (bands and controls), `props` (property grid), `preview` (document viewer).

Phase R1 is done. Exporters, the viewer widget, the server and scheduled delivery are not built yet; [docs/REPORTING.md](docs/REPORTING.md) has the design and phases, and [docs/REPORTING-PARITY.md](docs/REPORTING-PARITY.md) tracks each feature.

## 7. Rendering

`ravenrs-render` is a small 2D renderer on `wgpu` 30.

* **Quads** are instanced. The WGSL fragment shader evaluates a signed-distance function of a rounded box with per-corner radii, so edges are anti-aliased analytically (one device pixel of coverage ramp) at any scale factor. Borders, two-stop linear gradients and fill share the same shader.
* **Soft shadows** are separate instances drawn under their shape: the coverage is an `erf` approximation over the same SDF, which gives a Gaussian-looking falloff in one pass with no blur texture.
* **Text** is shaped by Parley, rasterized per glyph by `swash` (outline, alpha mask, 4 horizontal sub-pixel positions, no hinting) into an R8 atlas packed with shelves, and drawn as textured quads snapped to the device pixel grid. The atlas grows (×2 up to the device limit) when it fills; at the limit it is cleared.
* **Color.** Tokens are sRGB. The renderer prefers non-sRGB surface formats so blending happens in sRGB space, as in browsers and design tools; on `*Srgb` targets the shaders linearize colors first. Output is premultiplied alpha.
* **Resources** (pipelines, atlas, instance buffers) are created once and reused; instance buffers only grow. GPU→CPU readback exists only in tests: `crates/ravenrs-render/tests/offscreen.rs` checks pixels of real scenes and `crates/ravenrs/tests/visual.rs` compares golden images.

**Why not Vello (yet).** Vello's main renderer requires compute shaders and is still labelled experimental; its hybrid variant is promising but young. RavenRS needs a handful of primitives that an SDF quad pipeline draws in one pass, with a small WASM footprint. Vello remains the candidate for arbitrary vector paths (charts, icons, `Canvas` paths) behind the same `Scene` boundary.

**Glass and blur (planned).** Raven Glass's backdrop blur needs a layer compositor: render the content behind a glass surface to an intermediate texture, blur it (separable Gaussian, downsampled), then composite. Until that lands, glass surfaces render their translucent fill; this is the documented degradation path for platforms where the extra passes are too costly.

## 8. Layout

`ravenrs-layout` exposes a small typed subset of flexbox (`LayoutStyle`: axis, spacing, padding, alignment, justification, sizes, growth) over Taffy. The UI root is a box sized to the viewport; stacks are flex containers; text leaves have zero minimum width so they wrap instead of overflowing. The `Grid` view uses Taffy's Grid layout with equal-width columns (`grid_columns`).

## 9. Text

Parley is used with system fonts disabled and Inter (SIL OFL 1.1) embedded at weights 400 and 600, so text is identical on every platform and works in browsers, which expose no system fonts to WebAssembly. `TextSystem::register_font` adds more fonts. Layout happens in logical pixels; glyphs are rasterized at the device scale factor, so HiDPI text is sharp without reshaping.

## 10. Platforms

**Desktop (`ravenrs-platform-native`).** winit 0.30 (`ApplicationHandler`), one window, `ControlFlow::Wait`. Surface reconfiguration on resize and scale-factor changes, logical-pixel input, cursor icons, keyboard focus traversal, and GPU device-loss recovery (the device, surface and renderer are recreated on the next frame).

**Web (`ravenrs-platform-web`).** Plain `web-sys` on a `<canvas>`:

* async adapter and device creation (`wasm-bindgen-futures`), never blocking the main thread;
* a friendly message next to the canvas when `navigator.gpu` is missing (no WebGL fallback in this phase);
* canvas backing store sized from CSS size × `devicePixelRatio`, re-checked every frame and on `ResizeObserver` callbacks;
* pointer events with pointer capture, keyboard events with Tab/Space/Enter kept inside the app;
* `requestAnimationFrame` scheduled only when the UI is invalidated (at most one pending);
* GPU device-loss recovery: the device is re-requested asynchronously;
* `WebApp` removes every listener and observer and releases GPU resources when dropped.

`winit`'s web backend was not used so that DPR handling, frame scheduling and (later) IME and accessibility bridges stay explicit.

**Accessibility.** After layout, `Ui::access_tree` walks the element tree and builds a platform-neutral `AccessTree` (`ravenrs-core`).
* **Which elements become nodes.** An element becomes a node when a widget described it with `Semantics` or when it is focusable (it then defaults to a button). Text is a label, except inside roles that take their name from content, where it becomes the name ("Save" on a button). Other boxes are transparent.
* **Ids.** Node ids derive from `WidgetId`, so they stay stable across rebuilds.
* **Modal layers.** While one is open, only it is exposed.
* **Actions.** `Ui::perform_action` maps click, focus and increment/decrement back to the element's handlers. Assistive technology therefore drives the same code paths as the mouse and keyboard.
* **Desktop.** The tree goes to AccessKit (`accesskit_winit`) only while a screen reader is listening, and is resent when it changes. The root transform scales logical to physical pixels.
* **Web.** It is mirrored as invisible, absolutely positioned ARIA elements over the canvas. The hidden text input that holds keyboard focus points at the focused node with `aria-activedescendant`. New status and alert nodes are announced through a live region, and clicks on mirror elements become click actions.

**WASM size (hello-world, `wasm-release` profile + `wasm-opt -Oz`).** ≈2.4 MB raw, ≈1.06 MB gzipped, of which about 830 KB raw are the two embedded Inter faces. Shipping a subsetted font is the first size win available.

## 11. Status

| Area | Implemented | Experimental | Planned |
|---|---|---|---|
| Workspace, local checks (`scripts/check.sh`: desktop + wasm) | ✓ | | Hosted CI (intentionally not used for now) |
| `State`, `Binding`, `Derived`, dependency tracking | ✓ | | |
| Full rebuild on tracked change | ✓ | | Incremental reconciliation, per-component observers |
| `View`, `Component`, `BuildCx`, `view_fn`, `responsive`, `Option<V>`, `Keyed`, `ViewExt` modifiers, `use_state` widget-local state | ✓ | | Environment values, `#[component]` macro (`ravenrs-macros`) |
| Layout: `VStack`, `HStack`, `Grid`, `ScrollView`, `Spacer`, `Section`, `Card`, `Splitter`, `AspectRatio` | ✓ | | `ZStack`, horizontal `ScrollView`, virtualized `List` |
| Inputs: `Button` (4 styles, 3 sizes), `IconButton`, `TextField`, `Toggle`, `Checkbox`, `RadioGroup`, `Slider`, `Stepper`, `SegmentedControl`, `Select`, `Combobox`, `ToggleButton`, `ToggleGroup`, `InputOtp`, `Calendar`, `DatePicker`, `ThemeSwitcher`; text selection in `TextField` | ✓ | | multi-line text |
| Navigation: `Tabs`, `Sidebar`, `Breadcrumb`, `Disclosure`, `Accordion`, `Pagination`, `Menubar`, `Ribbon` | ✓ | | `NavigationView`, docking |
| Overlays: `Tooltip`, `Popover`, `HoverCard`, `DropdownMenu`, `ContextMenu`, `Dialog`, `Sheet`, `Toaster`, `CommandPalette` | ✓ | | |
| Data grid: `DataGrid` (see §6.2) | ✓ | | see the parity table in §6.2 |
| Display: `Image` (PNG/JPEG, cover/fit/fill, mipmaps), `Text`, `Icon` (about 200 Lucide icons), `Badge`, `Avatar`, `Kbd`, `Divider`, `ProgressBar`, `Spinner`, `Skeleton`, `Carousel`, `ColorSwatch`, `Alert`, `Stat`, `List`, `Table`, `BarChart`, `AreaChart`, `Canvas` | ✓ | | image loading from URLs, line/pie charts |
| `GlassCard` | | translucent fill only | backdrop blur |
| Rounded rects, borders, gradients, soft shadows, text, images (mipmapped, rounded), rectangular clipping | ✓ | | rounded clipping, radial gradients, transforms, layers |
| Raven Dark / Light / Glass tokens, runtime theme switching, `reduced_motion` token (honored by `Spinner`) | ✓ | | reading the system's reduced-motion setting |
| Motion engine (springs, transitions) | | | `ravenrs-animation` |
| Desktop: window, HiDPI, resize, mouse, wheel/trackpad, keyboard, clipboard, device loss | ✓ | | multiple windows, IME |
| Web: canvas, WebGPU, DPR, pointer, wheel, touch scrolling, keyboard, text input through a hidden textarea (IME commit, dead keys, paste), clipboard copy, rAF on demand, cleanup, device loss | ✓ | IME preedit is not drawn yet | pinch and momentum scrolling |
| Accessibility | keyboard focus and focus ring, WAI-ARIA keys, accessibility tree, AccessKit on desktop, ARIA mirror on the web | | caret and selection for screen readers, right-to-left |
| Criteria language: parsing, evaluation, validation, SQL, formatting (see §6.3) | ✓ | | `DataGrid` filters on this crate |
| Reporting: engine, wizard, designer with preview (phase R1, see §6.4) | ✓ | | PDF and other exports, viewer widget, server and scheduled delivery (phases R2 to R5) |
| Benchmarks and profiling | `tracing` spans per phase, criterion benches (`crates/*/benches`, budgets in [docs/QUALITY.md](docs/QUALITY.md)) | | GPU timestamps |

## 12. Decisions log

| # | Decision | Alternatives considered | Why |
|---|---|---|---|
| 1 | Own SDF quad renderer on wgpu | Vello, tiny-skia | One pass per primitive, small WASM, no compute requirement. |
| 2 | Parley + swash + own atlas | glyphon/cosmic-text | Parley is the most complete pure-Rust rich text layout; owning the atlas keeps text in the same batching and pass as quads. |
| 3 | Embedded Inter, no system fonts | System fonts on desktop | Identical rendering everywhere; the web has no system fonts. |
| 4 | `web-sys` directly on the web | winit web backend | Explicit control of DPR, frame scheduling, IME and accessibility bridges. |
| 5 | Single-threaded reactivity with epoch subscriptions | Signals with back-references, `Arc<Mutex>` | No locks on the UI thread; safe drops; matches the browser model. |
| 6 | Display list (`Scene`) as the widget/renderer boundary | Widgets calling the renderer | Headless widget tests; swappable renderers. |
| 7 | Builders before macros | `VStack!{}` proc macros first | Real, type-checked API first; macros later as sugar over it. |
| 8 | Non-sRGB surface formats preferred | Linear blending | Matches how designers and browsers blend, so tokens look as designed. |
