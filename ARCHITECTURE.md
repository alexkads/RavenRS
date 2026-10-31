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
raven-core ─────────────┬──────────────┬──────────────┬─────────────┐
 (geometry, color,      │              │              │             │
  events, Scene)        ▼              ▼              ▼             ▼
                  raven-layout    raven-text     raven-style   raven-render
                   (Taffy)        (Parley)       (tokens)      (wgpu, swash)
raven-reactive ───┐      │              │              │             │
 (State, Binding, │      └──────┬───────┴──────────────┘             │
  Derived)        └────► raven-widgets ◄─────────────────────────────┤
                         (View, Component, BuildCx, Ui)              │
                                │                                    │
                ┌───────────────┴───────────────┐                    │
                ▼                               ▼                    │
     raven-platform-native             raven-platform-web ◄──────────┘
       (winit + wgpu)              (canvas + WebGPU, wasm32 only)
                └───────────────┬───────────────┘
                                ▼
                              raven  (facade + prelude)
```

There are no cycles. `raven-widgets` never touches the GPU and `raven-render` knows nothing about widgets; they meet only through `raven_core::Scene`, a plain display list. That boundary keeps widgets testable without a GPU (the widget tests run headless) and keeps the renderer replaceable.

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

`raven-reactive` is single-threaded by design: `Rc`, `Cell`, `RefCell`, no locks. UI state lives on the UI thread on every platform, including the browser main thread.

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

## 7. Rendering

`raven-render` is a small 2D renderer on `wgpu` 30.

* **Quads** are instanced. The WGSL fragment shader evaluates a signed-distance function of a rounded box with per-corner radii, so edges are anti-aliased analytically (one device pixel of coverage ramp) at any scale factor. Borders, two-stop linear gradients and fill share the same shader.
* **Soft shadows** are separate instances drawn under their shape: the coverage is an `erf` approximation over the same SDF, which gives a Gaussian-looking falloff in one pass with no blur texture.
* **Text** is shaped by Parley, rasterized per glyph by `swash` (outline, alpha mask, 4 horizontal sub-pixel positions, no hinting) into an R8 atlas packed with shelves, and drawn as textured quads snapped to the device pixel grid. The atlas grows (×2 up to the device limit) when it fills; at the limit it is cleared.
* **Color.** Tokens are sRGB. The renderer prefers non-sRGB surface formats so blending happens in sRGB space, as in browsers and design tools; on `*Srgb` targets the shaders linearize colors first. Output is premultiplied alpha.
* **Resources** (pipelines, atlas, instance buffers) are created once and reused; instance buffers only grow. The only GPU→CPU readback in the repository is in `crates/raven-render/tests/offscreen.rs`, which renders real scenes on the GPU and checks pixels.

**Why not Vello (yet).** Vello's main renderer requires compute shaders and is still labelled experimental; its hybrid variant is promising but young. RavenRS needs a handful of primitives that an SDF quad pipeline draws in one pass, with a small WASM footprint. Vello remains the candidate for arbitrary vector paths (charts, icons, `Canvas` paths) behind the same `Scene` boundary.

**Glass and blur (planned).** Raven Glass's backdrop blur needs a layer compositor: render the content behind a glass surface to an intermediate texture, blur it (separable Gaussian, downsampled), then composite. Until that lands, glass surfaces render their translucent fill; this is the documented degradation path for platforms where the extra passes are too costly.

## 8. Layout

`raven-layout` exposes a small typed subset of flexbox (`LayoutStyle`: axis, spacing, padding, alignment, justification, sizes, growth) over Taffy. The UI root is a box sized to the viewport; stacks are flex containers; text leaves have zero minimum width so they wrap instead of overflowing. Taffy also offers Grid and Block layout, which `Grid` and `List` will use.

## 9. Text

Parley is used with system fonts disabled and Inter (SIL OFL 1.1) embedded at weights 400 and 600, so text is identical on every platform and works in browsers, which expose no system fonts to WebAssembly. `TextSystem::register_font` adds more fonts. Layout happens in logical pixels; glyphs are rasterized at the device scale factor, so HiDPI text is sharp without reshaping.

## 10. Platforms

**Desktop (`raven-platform-native`).** winit 0.30 (`ApplicationHandler`), one window, `ControlFlow::Wait`. Surface reconfiguration on resize and scale-factor changes, logical-pixel input, cursor icons, keyboard focus traversal, and GPU device-loss recovery (the device, surface and renderer are recreated on the next frame).

**Web (`raven-platform-web`).** Plain `web-sys` on a `<canvas>`:

* async adapter and device creation (`wasm-bindgen-futures`), never blocking the main thread;
* a friendly message next to the canvas when `navigator.gpu` is missing (no WebGL fallback in this phase);
* canvas backing store sized from CSS size × `devicePixelRatio`, re-checked every frame and on `ResizeObserver` callbacks;
* pointer events with pointer capture, keyboard events with Tab/Space/Enter kept inside the app;
* `requestAnimationFrame` scheduled only when the UI is invalidated (at most one pending);
* GPU device-loss recovery: the device is re-requested asynchronously;
* `WebApp` removes every listener and observer and releases GPU resources when dropped.

`winit`'s web backend was not used so that DPR handling, frame scheduling and (later) IME and accessibility bridges stay explicit.

**WASM size (hello-world, `wasm-release` profile + `wasm-opt -Oz`).** ≈2.4 MB raw, ≈1.06 MB gzipped, of which about 830 KB raw are the two embedded Inter faces. Shipping a subsetted font is the first size win available.

## 11. Status

| Area | Implemented | Experimental | Planned |
|---|---|---|---|
| Workspace, CI (desktop + wasm) | ✓ | | |
| `State`, `Binding`, `Derived`, dependency tracking | ✓ | | |
| Full rebuild on tracked change | ✓ | | Incremental reconciliation, per-component observers |
| `View`, `Component`, `BuildCx`, `view_fn`, `Option<V>` | ✓ | | Environment values, `#[component]` macro (`raven-macros`) |
| `VStack`, `HStack`, `Spacer`, `Text`, `Button`, `Card`, `Canvas` | ✓ | | `ZStack`, `Image`, `TextField`, `Toggle`, `Slider`, `ScrollView`, `List`, `Grid`, `NavigationView`, `TabView`, `Popover`, `Tooltip` |
| `GlassCard` | | translucent fill only | backdrop blur |
| Rounded rects, borders, gradients, soft shadows, text | ✓ | | radial gradients, clipping, transforms, images, layers |
| Raven Dark / Light / Glass tokens | ✓ | | runtime theme switch UI, reduced-motion wiring |
| Motion engine (springs, transitions) | | | `raven-animation` |
| Desktop: window, HiDPI, resize, mouse, keyboard, device loss | ✓ | | multiple windows, clipboard |
| Web: canvas, WebGPU, DPR, pointer, keyboard, rAF on demand, cleanup, device loss | ✓ | | clipboard bridge, IME, touch gestures |
| Accessibility | keyboard focus and visible focus ring | | AccessKit tree, screen readers |
| Benchmarks and profiling | `tracing` spans per phase | | criterion benches, GPU timestamps |

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
