<div align="center">

# RavenRS

**Beautiful native interfaces, powered by Rust.**

One declarative UI, drawn on the GPU, in a native window on Windows, macOS and Linux, and in the browser through WebAssembly + WebGPU.

[**Live gallery**](https://alexkads.github.io/RavenRS/) · [Getting started](https://github.com/alexkads/RavenRS/blob/master/docs/GETTING_STARTED.md) · [API docs](https://alexkads.github.io/RavenRS/api/ravenrs/) · [Components](#components) · [Parity tracker](https://github.com/alexkads/RavenRS/blob/master/docs/PARITY.md) · [Architecture](https://github.com/alexkads/RavenRS/blob/master/ARCHITECTURE.md) · [Contributing](https://github.com/alexkads/RavenRS/blob/master/CONTRIBUTING.md)

![License: MIT or Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-4c66ee)
![Rust 1.88+](https://img.shields.io/badge/rust-1.88%2B-4c66ee)
![Platforms](https://img.shields.io/badge/platforms-Windows%20%7C%20macOS%20%7C%20Linux%20%7C%20Web-4c66ee)

![The RavenRS component gallery running natively on macOS](https://raw.githubusercontent.com/alexkads/RavenRS/master/docs/media/component-gallery.png)

</div>

RavenRS is a free, open source GUI framework for Rust with a large component library: 60+ components including a virtualized **DataGrid** for 100,000+ rows, an Office-style **Ribbon**, a **command palette**, calendars, menus, dialogs, charts and 200 icons. Everything is drawn by RavenRS itself with `wgpu`, so the same code looks the same on the desktop and on the web.

> **Versions follow the component-vendor model:** `YY.R.P`, with breaking changes only in each year's `.1` release, new features in `.2`, and monthly fixes ([docs/VERSIONING.md](docs/VERSIONING.md)). The current line, 26.x, is young: [docs/QUALITY.md](docs/QUALITY.md) lists what is still missing. [docs/PARITY.md](https://github.com/alexkads/RavenRS/blob/master/docs/PARITY.md) tracks every component family of shadcn/ui, DevExpress and Telerik and where RavenRS stands.

## Why RavenRS

- **One UI, every platform.** Desktop windows and a browser `<canvas>` share the renderer, the layout engine and every component. No webview, no DOM widgets.
- **Declarative and reactive.** Compose views with typed builders. `State<T>` knows who reads it, so a change redraws only when something on screen depends on it, and an idle app uses no CPU or GPU.
- **Beautiful by default.** Three themes (Raven Dark, Raven Light, Raven Glass) built from semantic design tokens, with text contrast checked against WCAG AA in the test suite.
- **Batteries included.** Data grid, ribbon, date pickers, command palette, toasts, context menus, splitters, carousels, forms with validation and more, all keyboard-operable.
- **Built to be extended.** Your components use the same public API as the built-in ones, from a simple `Component` to a fully custom `View` that draws, scrolls, opens overlays and handles input.

## Quick start

Add RavenRS from [crates.io](https://crates.io/crates/ravenrs):

```bash
cargo add ravenrs
```

Write your UI:

```rust,no_run
use ravenrs::prelude::*;

fn app() -> impl View {
    let count = State::new(0);
    let increment = count.clone();

    VStack::new((
        Text::new("Welcome to RavenRS").style(TextStyle::Display),
        Card::new(VStack::new((
            Text::new("REVENUE").style(TextStyle::Caption).foreground(Semantic::InkMuted),
            Text::new("R$ 128.450").style(TextStyle::Metric),
            Text::bound(&count, |n| format!("Clicked {n} times")).foreground(Semantic::InkMuted),
        ))),
        Button::new("Get Started").icon(IconName::Sparkles).on_click(move || increment.update(|n| *n += 1)),
    ))
    .spacing(24.0)
    .padding(32.0)
}

fn main() -> Result<(), ravenrs::native::PlatformError> {
    ravenrs::native::run(app(), ravenrs::native::WindowOptions::new("Hello"))
}
```

The [getting started guide](https://github.com/alexkads/RavenRS/blob/master/docs/GETTING_STARTED.md) covers the browser build, state, components of your own, themes and the data grid.

## Components

| Group | Components |
|---|---|
| Inputs | `Button` (4 styles, 3 sizes, icons), `IconButton`, `ToggleButton`, `ToggleGroup`, `TextField`, `Select`, `Combobox`, `Checkbox`, `RadioGroup`, `Toggle`, `Slider`, `Stepper`, `SegmentedControl`, `InputOtp`, `Calendar`, `DatePicker`, `ThemeSwitcher` |
| Forms | `FormField` with `validate::{required, email, min_length}` and custom rules, `Label` |
| Navigation | `Ribbon`, `Menubar`, `Tabs`, `Sidebar`, `Breadcrumb`, `Pagination`, `Disclosure`, `Accordion`, `CommandPalette` (⌘K) |
| Overlays | `Dialog`, `Sheet`, `Popover`, `HoverCard`, `Tooltip`, `DropdownMenu`, `ContextMenu`, `Toaster` |
| Data | `DataGrid` (virtualized; multi-sort, filter row, search, grouping, summaries, resize, column chooser, selection, editing, templates, CSV export), `Table`, `List`, `Stat`, `BarChart`, `AreaChart` |
| Layout | `VStack`, `HStack`, `Grid`, `ScrollView`, `Splitter`, `Card`, `GlassCard`, `Section`, `AspectRatio`, `Spacer`, `Footer`, `responsive`, and `.padding()`, `.background()`, `.frame()`, `.grow()` on any view |
| Media and feedback | `Image` (PNG/JPEG), `Icon` (200 Lucide icons), `Carousel`, `Avatar`, `Badge`, `Alert`, `ProgressBar`, `Spinner`, `Skeleton`, `Kbd`, `Divider`, `Canvas` |

Every component is in [`examples/component-gallery`](https://github.com/alexkads/RavenRS/blob/master/examples/component-gallery), which runs natively and [in your browser](https://alexkads.github.io/RavenRS/). For a whole app, see [`examples/family-finance`](https://github.com/alexkads/RavenRS/blob/master/examples/family-finance): family expenses and bills to pay on SQLite, with reports.

## Your own components

```rust,no_run
use ravenrs::prelude::*;

/// State lives in the struct; `body` composes views and runs on every rebuild.
struct Counter {
    count: State<i32>,
}

impl Component for Counter {
    fn body(&self) -> impl View {
        let count = self.count.clone();
        HStack::new((
            Text::bound(&self.count, |c| format!("{c}")).style(TextStyle::Title),
            Button::new("+1").on_click(move || count.update(|c| *c += 1)),
        ))
    }
}
```

Need full control? Implement `View` and use `BuildCx`: layout boxes, any drawing through `Paint::Custom`, per-instance state with `cx.use_state`, floating layers with `cx.overlay`, virtualized scrolling, timers with `cx.wake_at`, and input through `on_click`, `on_key`, `on_drag`, `on_context_menu` and `shortcut`. Every built-in component is written this way.

## Run the examples

Requirements: Rust stable 1.88+. For the browser: the `wasm32-unknown-unknown` target and `wasm-bindgen-cli` **0.2.130** (it must match the crate version).

```bash
cargo run -p component-gallery --release
```

```bash
rustup target add wasm32-unknown-unknown
```

```bash
cargo install wasm-bindgen-cli --version 0.2.130 --locked
```

```bash
./scripts/build-web.sh component-gallery && python3 -m http.server -d examples/component-gallery/web 8080
```

Then open http://localhost:8080 in a browser with WebGPU (Chrome, Edge, Safari 18+, or Firefox with WebGPU enabled). Without WebGPU, the page explains what is missing.

## AI assistants

RavenRS ships a skill and an MCP server so coding assistants write against the real API instead of guessing it.

- The **skill** teaches how to compose screens, use tokens instead of literal values, meet the [quality bar](https://github.com/alexkads/RavenRS/blob/master/docs/QUALITY.md) and build for the web.
- The **MCP server** ([`ravenrs-mcp`](https://github.com/alexkads/RavenRS/blob/master/crates/ravenrs-mcp)) is read-only and works offline. It lists components, shows their API and real examples, looks up design tokens, reports parity status and validates expressions.

Install the server, then add the Claude Code plugin, which brings both:

```bash
cargo install --git https://github.com/alexkads/RavenRS --locked ravenrs-mcp
```

```bash
claude plugin marketplace add alexkads/RavenRS
```

```bash
claude plugin install ravenrs@ravenrs
```

For other MCP clients, configure the command `ravenrs-mcp` over stdio. The skill is a plain file: [`plugins/ravenrs/skills/ravenrs/SKILL.md`](https://github.com/alexkads/RavenRS/blob/master/plugins/ravenrs/skills/ravenrs/SKILL.md).

## Crates

| Crate | What it does |
|---|---|
| [`ravenrs`](https://github.com/alexkads/RavenRS/blob/master/crates/ravenrs) | The facade and `prelude`. Start here. |
| [`ravenrs-widgets`](https://github.com/alexkads/RavenRS/blob/master/crates/ravenrs-widgets) | Every component, the `View`/`Component`/`BuildCx` API and the `Ui` runtime. |
| [`ravenrs-reactive`](https://github.com/alexkads/RavenRS/blob/master/crates/ravenrs-reactive) | `State`, `Binding`, `Derived` and dependency-tracking observers. |
| [`ravenrs-style`](https://github.com/alexkads/RavenRS/blob/master/crates/ravenrs-style) | Design tokens and the Raven Dark, Light and Glass themes. |
| [`ravenrs-layout`](https://github.com/alexkads/RavenRS/blob/master/crates/ravenrs-layout) | Flexbox and grid layout on [Taffy](https://github.com/DioxusLabs/taffy). |
| [`ravenrs-text`](https://github.com/alexkads/RavenRS/blob/master/crates/ravenrs-text) | Text with [Parley](https://github.com/linebender/parley); Inter and the icon font embedded. |
| [`ravenrs-render`](https://github.com/alexkads/RavenRS/blob/master/crates/ravenrs-render) | The `wgpu` renderer: SDF shapes, shadows, gradients, images, glyph atlas, clipping. |
| [`ravenrs-core`](https://github.com/alexkads/RavenRS/blob/master/crates/ravenrs-core) | Geometry, colors, input events and the `Scene` display list. |
| [`ravenrs-platform-native`](https://github.com/alexkads/RavenRS/blob/master/crates/ravenrs-platform-native) | Desktop windows with [winit](https://github.com/rust-windowing/winit). |
| [`ravenrs-platform-web`](https://github.com/alexkads/RavenRS/blob/master/crates/ravenrs-platform-web) | `<canvas>` + WebGPU in the browser. |
| [`ravenrs-mcp`](https://github.com/alexkads/RavenRS/blob/master/crates/ravenrs-mcp) | A read-only MCP server for coding assistants. A tool, not a dependency of your app. |

## Roadmap

- **Reporting**: the engine, the [report designer](https://alexkads.github.io/RavenRS/designer/), PDF, XLSX and CSV export, and a [report server](https://github.com/alexkads/RavenRS/blob/master/docs/REPORT-SERVER.md) with scheduled delivery are here. Next: subreports and a table of contents. See [docs/REPORTING.md](https://github.com/alexkads/RavenRS/blob/master/docs/REPORTING.md).
- **DataGrid parity**: frozen columns, drag reordering and drag-to-group, header filters, master-detail.
- **Vector paths** for line and pie charts and gauges, and backdrop blur for Raven Glass.
- **Platform**: IME composition display, right-to-left layout.
- **Motion**: rotation and true group opacity, which need transforms and layers in the renderer.

See [docs/PARITY.md](https://github.com/alexkads/RavenRS/blob/master/docs/PARITY.md) for the full list and [CHANGELOG.md](https://github.com/alexkads/RavenRS/blob/master/CHANGELOG.md) for what changed.

## Contributing

Issues, ideas and pull requests are welcome. [CONTRIBUTING.md](https://github.com/alexkads/RavenRS/blob/master/CONTRIBUTING.md) explains how to build, test and add a component. Please follow the [code of conduct](https://github.com/alexkads/RavenRS/blob/master/CODE_OF_CONDUCT.md).

## License

Licensed under either of [Apache License 2.0](https://github.com/alexkads/RavenRS/blob/master/LICENSE-APACHE) or [MIT](https://github.com/alexkads/RavenRS/blob/master/LICENSE-MIT), at your option. The embedded Inter font is under the [SIL Open Font License 1.1](https://github.com/alexkads/RavenRS/blob/master/crates/ravenrs-text/fonts/Inter-LICENSE.txt); the icons come from [Lucide](https://lucide.dev) under the [ISC License](https://github.com/alexkads/RavenRS/blob/master/crates/ravenrs-text/fonts/RavenIcons-LICENSE.txt).

Unless you state otherwise, any contribution you submit is dual-licensed as above, without additional terms.
