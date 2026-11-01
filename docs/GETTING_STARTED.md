# Getting started with RavenRS

This guide builds a small app step by step: a window, then the same app in the browser, then state, your own components, themes and the data grid.

## 1. A window

```bash
cargo new hello-raven && cd hello-raven
```

```bash
cargo add ravenrs
```

```rust
// src/main.rs
use ravenrs::prelude::*;

fn app() -> impl View {
    VStack::new((
        Text::new("Hello, RavenRS").style(TextStyle::Display),
        Text::new("Drawn on the GPU.").foreground(Semantic::InkMuted),
    ))
    .padding(32.0)
}

fn main() -> Result<(), ravenrs::native::PlatformError> {
    ravenrs::native::run(app(), ravenrs::native::WindowOptions::new("Hello").size(800.0, 500.0))
}
```

```bash
cargo run
```

## 2. The same app in the browser

Put the UI in `src/lib.rs` so both targets share it, and add a web entry point:

```toml
[lib]
crate-type = ["cdylib", "rlib"]

[target.'cfg(target_arch = "wasm32")'.dependencies]
wasm-bindgen = "=0.2.130"
wasm-bindgen-futures = "0.4"
```

```rust
// src/lib.rs
use ravenrs::prelude::*;

pub fn app() -> impl View {
    Text::new("Hello from the browser").style(TextStyle::Display).padding(32.0)
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn start() {
    wasm_bindgen_futures::spawn_local(async {
        // Keep the returned app alive for as long as the page shows it.
        if let Ok(app) = ravenrs::web::start(app(), ravenrs::web::WebOptions::new("raven-canvas")).await {
            std::mem::forget(app);
        }
    });
}
```

A page with a `<canvas id="raven-canvas">` sized by CSS hosts it; [`examples/hello-world/web/index.html`](../examples/hello-world/web/index.html) is a complete one, and [`scripts/build-web.sh`](../scripts/build-web.sh) shows the build (`cargo build --target wasm32-unknown-unknown` followed by `wasm-bindgen --target web`).

## 3. State

`State<T>` holds a value. Views that read it rebuild when it changes; nothing else does.

```rust
let count = State::new(0);
let inc = count.clone();
VStack::new((
    Text::bound(&count, |n| format!("{n} clicks")),
    Button::new("Click").on_click(move || inc.update(|n| *n += 1)),
))
```

Controls take a `Binding<T>`, a read/write view of a state: `Toggle::new(enabled.binding())`, `TextField::new(name.binding())`, `Slider::new(volume.binding())`. `Binding::map` focuses on one field of a struct, and `Derived<T>` caches a value computed from other states.

## 4. Your own components

A `Component` keeps its state in fields and composes views in `body`:

```rust
struct Counter { count: State<i32> }

impl Component for Counter {
    fn body(&self) -> impl View {
        let c = self.count.clone();
        Button::new("Add").on_click(move || c.update(|n| *n += 1))
    }
}
```

For full control, implement `View` and use `BuildCx`. This badge draws itself, reacts to hover and handles clicks:

```rust
struct Pill(&'static str);

impl View for Pill {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let fill = if cx.is_hovered() { theme.color(Semantic::Accent) } else { theme.color(Semantic::Control) };
        let label = cx.child(&Text::new(self.0));
        let node = cx.container(
            ravenrs::layout::LayoutStyle { padding: (6.0, 12.0).into(), ..Default::default() },
            &[label],
            Paint::Quad(QuadPaint { corner_radius: 999.0, ..QuadPaint::new(fill) }),
        );
        cx.on_click(node, std::rc::Rc::new(|| println!("clicked")));
        cx.cursor(node, CursorIcon::Pointer);
        node
    }
}
```

`cx.use_state` gives a view instance its own state (open/closed, scroll position), `cx.overlay` opens a floating layer, `cx.wake_at` schedules a rebuild (animations, timers) and `cx.shortcut` registers app-wide keys.

## 5. Themes

Pick a theme when you start the app, or switch at runtime:

```rust
ravenrs::native::run(app(), ravenrs::native::WindowOptions::new("App").theme(Theme::raven_light()))
```

`ThemeSwitcher` toggles Raven Dark, Light and Glass; any component can do the same with `cx.theme_state().set(...)`. Components use semantic roles (`Semantic::Ink`, `Semantic::Accent`, `Semantic::SurfaceElevated`…) so they follow the theme automatically. The tokens are documented in [docs/design-system](design-system/README.md).

## 6. The data grid

```rust
use ravenrs::widgets::grid::Value;

let data = GridData::new(vec![
    vec![Value::from("Northwind"), Value::from("Brazil"), Value::from(240)],
    vec![Value::from("Aurora"), Value::from("Chile"), Value::from(86)],
]);

DataGrid::new(data, [
    GridColumn::text("Customer").editable(),
    GridColumn::text("Country"),
    GridColumn::number("Seats").summary(Summary::Sum),
])
.selection_mode(SelectionMode::Multiple)
.height(480.0)
```

Sorting, the filter row (`>100`, `10..20`, `yes`, or an expression such as `[Seats] > 10 And [Country] = 'Brazil'`), search, grouping, resizing, the column chooser, editing and CSV export work out of the box; only the visible rows are built, so 100,000+ rows stay smooth.

## Next

- Browse every component in the [live gallery](https://alexkads.github.io/RavenRS/) and its [source](../examples/component-gallery/src/lib.rs).
- Read the [architecture](../ARCHITECTURE.md) to see how build, layout, paint and render fit together.
- The [API documentation](https://alexkads.github.io/RavenRS/api/ravenrs/) lists every type and method.
