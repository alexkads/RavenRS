---
name: ravenrs
description: Builds and changes user interfaces with RavenRS, the Rust GUI framework that draws on the GPU on the desktop and in the browser. Use whenever the code depends on the `ravenrs` crates or the user mentions RavenRS, including screens, forms, DataGrid, Ribbon, custom components, themes and design tokens, accessibility, the WebAssembly build, `ravenrs-criteria` expressions, `ravenrs-report`, label and receipt printers, `ravenrs-app` and the beta NF-e and point of sale crates.
---

<!-- ravenrs:skill:begin -->
<!-- cargo-ravenrs 27.1.28 -->

# Building with RavenRS

RavenRS is declarative and reactive. Views are typed builders composed in Rust, `State<T>` drives rebuilds, and everything is drawn with `wgpu`. The same code runs in a native window and on a browser `<canvas>`. There is no DOM, CSS or webview, so web habits do not apply.

## Check, don't guess

Builder method names vary by component, so confirm them before writing a view. If the `ravenrs` MCP server is connected, use it. It answers from the sources of the version it was built from.

| Question | Tool |
|---|---|
| Which component fits? | `list_components` |
| What are its methods and states? | `component_api`, which also accepts `ViewExt`, `State`, `Semantic`, `BuildCx` and any other public item |
| How is it used in real code? | `component_examples` |
| Which color or text style is this? | `design_tokens` |
| Which type does this? I do not know its name. | `search_api`, over every crate |
| How does this part work? What does it not do yet? | `guides`, over the repository's documents |
| Does this feature exist yet? | `parity_status`; for record applications, with `document: "application"` |
| What does a reusable module have to honor, and which issue follows a capability? | `app_contracts` |
| Is this expression valid? Which functions exist? | `validate_expression`, `expression_functions` |
| How do I make a report? | `report_layout_format`, then `create_report_layout` or `report_layout_templates`; confirm with `check_report_layout` and save the JSON as a `.rvreport.json` file |

Without the server, use the [API docs](https://alexkads.github.io/RavenRS/api/ravenrs/) and the [gallery source](https://github.com/alexkads/RavenRS/blob/master/examples/component-gallery/src/lib.rs). Check `parity_status` or [docs/PARITY.md](https://github.com/alexkads/RavenRS/blob/master/docs/PARITY.md) before promising a feature, because some are still planned. When a feature is missing, say so and propose composing it from what exists.

## Composing a screen

```rust,no_run
use ravenrs::prelude::*;

struct Settings {
    name: State<String>,
    alerts: State<bool>,
    saved: State<u32>,
}

impl Component for Settings {
    fn body(&self) -> impl View {
        let saved = self.saved.clone();
        VStack::new((
            Text::new("Settings").style(TextStyle::Display),
            Card::new(VStack::new((
                FormField::new("Name", TextField::new(self.name.binding()).placeholder("Your name"))
                    .required()
                    .validate(self.name.binding(), validate::min_length(3)),
                HStack::new((Toggle::new(self.alerts.binding()), Text::new("Send alerts"))),
                Text::bound(&self.saved, |n| format!("Saved {n} times")).foreground(Semantic::InkMuted),
            ))),
            Button::new("Save changes").icon(IconName::Check).on_click(move || saved.update(|n| *n += 1)),
        ))
        .spacing(24.0)
        .padding(32.0)
    }
}

fn main() -> Result<(), ravenrs::native::PlatformError> {
    let app = Settings { name: State::new(String::new()), alerts: State::new(true), saved: State::new(0) };
    ravenrs::native::run(app, ravenrs::native::WindowOptions::new("Settings").size(800.0, 500.0))
}
```

- **State.** Keep `State<T>` in a struct's fields and clone it into closures. Pass `state.binding()` to controls, use `Binding::map` for one field of a struct, and `Derived<T>` for computed values. `body` runs on every rebuild, so state created inside it starts over each time. In a custom `View`, use `cx.use_state`.
- **Events.** Values come in through `Binding<T>` and events go out through closures, such as `on_click`.
- **Layout.** Use `VStack`, `HStack`, `Grid` and `ScrollView`, plus the `ViewExt` modifiers on any view: `.padding()`, `.background()`, `.frame()`, `.grow()`, `.stretch()`, `.centered()`.
- **Custom components.** Use a function returning `impl View` when there is no state, and a `Component` when there is. Implement `View` with `BuildCx` only for your own drawing, input or overlays. Built-in components use the same public API, so read one as a model.
- **Large data.** Use `DataGrid`, which is virtualized for 100,000+ rows, rather than a `ScrollView` full of rows.

## Tokens, not literals

Interfaces must stay correct in Raven Dark, Light and Glass, so never hard-code a color.

- **Color:** use `Semantic` roles: `.foreground(Semantic::InkMuted)`, `.background(Semantic::Control)`, and `cx.theme().color(Semantic::Accent)` in a custom `View`. Do not use literal `Color` values for UI chrome.
- **Text:** use `TextStyle` steps, such as `Display`, `Title`, `Headline`, `Body`, `Callout`, `Caption` and `Metric`, rather than `font_size`.
- **Space and shape:** builders take logical pixels, so stay on the 4px grid: screens pad by 32, cards by 24, stacks by 16. In a custom `View`, read sizes from `cx.theme()`.
- **Emphasis:** use one primary action per view (`ButtonStyle::Primary`) and `Secondary` or `Ghost` for the rest. `success`, `warning` and `danger` always come with a label or icon, never color alone.

Depend on the role names rather than on how a theme is put together. The theme API is still evolving, including support for other design systems. `Semantic` and most other growing enums are `#[non_exhaustive]`, so a `match` on them needs a wildcard arm.

## Motion

Built-in components already animate, so don't add motion to them. In a custom `View`, declare motion with `cx.motion(node, Motion::new()...)` and the runtime plays it, for example `.paint(cx.theme().motion.fast)` for color on hover.

- Use the `theme.motion` tokens instead of your own numbers: `fast` for hover, press and focus color, `base` for small layout and opacity changes, `snappy` for things changing place or size, `gentle` for panels and sheets.
- Exits are shorter than entrances. Scale subtly (0.96 and up), and let popups grow from their anchor with `Motion::anchored`.
- A row in a virtualized list has no enter animation, because scrolling would set it off.

## The states a component owes

A control or screen is done when it meets the [quality bar](https://github.com/alexkads/RavenRS/blob/master/docs/QUALITY.md):

- **States:** rest, hover, pressed, focused (visible ring), disabled, selected where it applies, plus empty, loading and error. Long text, many items and a tiny viewport must not break it.
- **Names:** every control has a name for screen readers, from its text, a `label`, a `FormField` or a `Tooltip`. An `IconButton` always needs one. A custom view describes itself with `cx.semantics(node, Semantics::new(Role::Switch).name("Wi-Fi").toggled(true))`.
- **Keyboard:** reachable with Tab, Enter and Space activate, arrows follow the WAI-ARIA pattern for the widget, Escape closes overlays. A custom interactive view calls `cx.focusable` and `cx.on_key`.
- **Input:** out-of-range values are clamped, never a panic. Hit targets are at least 32px tall, 44px on touch.
- **Copy:** buttons start with a verb. Errors say what happened and how to fix it.

## The lint is the judge

Applications are checked by [`ravenrs-lint`](https://github.com/alexkads/RavenRS/tree/master/crates/ravenrs-lint), which denies what this page asks for: literal colors and font sizes, spacing off the 4px grid, captions without a verb, unnamed icon buttons, more than one primary button, `State::new` and blocking work in `body` or `build`, and files over 600 lines.

- Before saying a task is done, run `cargo ravenrs-lint --gate` (formatting, Clippy with `-D warnings`, the rules and the tests) and make it pass with no warnings.
- When it fails, fix the code as the diagnostic's `help` says. Never silence a rule with `#[allow]`, a lower level in `Cargo.toml` or a `ravenrs-lint: allow(..)` comment just to pass; an exception that is truly right carries its reason and is mentioned to the user.
- A project without it gets it with `cargo ravenrs-lint --init --language en` (or `pt-BR`), which also turns on the strictest rustc and Clippy levels and writes the same rules for every coding agent.

## Testing without a window

`Ui` runs a view without a GPU, so behavior tests drive it directly:

```rust,no_run
use ravenrs::prelude::*;

let mut ui = Ui::new(Text::new("Hello"), Theme::raven_dark());
ui.set_viewport(Size::new(800.0, 600.0), 1.0);
ui.update();
let tree = ui.access_tree(); // What a screen reader sees.
```

Send input with `ui.handle_event(...)`, then call `ui.update()` and check the state, the access tree or the scene. The tests in [`crates/ravenrs-widgets/tests`](https://github.com/alexkads/RavenRS/tree/master/crates/ravenrs-widgets/tests) show the helpers for clicks and keys.

## Running on the web

Put the UI in `src/lib.rs`, shared by both targets, and add `#[wasm_bindgen(start)]`, as [getting started](https://github.com/alexkads/RavenRS/blob/master/docs/GETTING_STARTED.md) shows. `wasm-bindgen` must be pinned to the version RavenRS uses (`=0.2.130`), and the CLI must match it.

```bash
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.130 --locked
cargo build --lib --target wasm32-unknown-unknown --release
wasm-bindgen --target web --no-typescript --out-dir web/pkg target/wasm32-unknown-unknown/release/<crate_name>.wasm
```

Serve the folder over HTTP with a page that has `<canvas id="raven-canvas">`. It needs a browser with WebGPU. Threads and blocking calls are not available there, so keep them behind `cfg(not(target_arch = "wasm32"))`.

## Working inside the RavenRS repository

- `scripts/build-web.sh <example>` builds `hello-world`, `component-gallery` or `report-designer` for the browser, into `examples/<name>/web/pkg`.
- `scripts/check.sh` is the gate, because the project has no hosted CI. It runs formatting, clippy on native and wasm32, all tests, MSRV, `cargo deny`, docs and the web builds. For a quick pass use `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test -p <crate>`.
- A new worktree does not have to compile again what another one compiled. `scripts/check.sh` uses a compiler cache that worktrees share: [Kache](https://github.com/kunobi-ninja/kache) when it is installed, or the repository's own [`ravenrs-xcache`](https://github.com/alexkads/RavenRS/tree/master/crates/ravenrs-xcache) with `RAVENRS_XCACHE=1 scripts/check.sh`. Keep one build directory per worktree; never point two worktrees at the same `CARGO_TARGET_DIR`. `ravenrs-xcache stats` and `ravenrs-xcache explain <crate>` say what came from the cache and why something did not.
- A new component needs rustdoc with an example, semantics, keyboard support, a behavior test through `Ui` and a section in `examples/component-gallery`. Update `docs/PARITY.md` and `CHANGELOG.md` under `[Unreleased]`.
- Use only `Semantic` tokens and existing components in the repository's own modules. If something is missing, add it to the library first.
- Versions are `YY.R.P`: until 28.1.0 any release may hold breaking changes, listed in the CHANGELOG; from then on, only in each year's `.1` release, with additions in `.2` and fixes in maintenance releases. See [docs/VERSIONING.md](https://github.com/alexkads/RavenRS/blob/master/docs/VERSIONING.md). Visual changes need a golden-image update: `RAVENRS_UPDATE_GOLDEN=1 cargo test -p ravenrs --test suite visual::`.

## Reusable application modules

A module meant for more than one application follows [docs/APP-CONTRACTS.md](https://github.com/alexkads/RavenRS/blob/master/docs/APP-CONTRACTS.md); `app_contracts` lists its scenarios and groups with the issue that owns each one.

- Declare what the module needs in `Module::manifest()` (`ModuleManifest`: version, accepted versions of its dependencies, targets, databases, capabilities, migrations, jobs). `ApplicationBuilder::build` refuses what does not fit, and `switch_off` keeps a module's records readable while `Application::guard` refuses changes.
- Talk to anything outside through a `Provider` that declares its capabilities; events go through `EventSink`. Report failures with the `SpaceError` meanings: `Temporary` may work later, `Unknown` may have happened (check before doing it again), `Unavailable` is a missing capability.
- Test on `ravenrs-app-testkit`: organizations T1 and T2 share keys on purpose, roles are an example policy, `Clock` moves only when told, and `FakeSink` accepts, refuses, goes down or loses its answer. `ravenrs-app-requests` and `examples/contracts` show a package used by two applications of different sectors.
- Never put a sector's rules in a generic package: the application gives steps, roles and policies.

## Expressions and reports

`ravenrs-criteria` is the expression language used by filters and reports: `[Price] * [Qty] > ?Minimum`, `Iif([Paid], 'Yes', 'No')`, `[Items][[Qty] > 0].Sum([Qty] * [Price])`. Validate expressions with `validate_expression` before putting them in code. `ravenrs-report` defines reports as JSON or with a builder. See [docs/REPORTING.md](https://github.com/alexkads/RavenRS/blob/master/docs/REPORTING.md), and check `parity_status` with `document: "reporting"` before relying on a feature. Export to PDF and XLSX is still planned.

## Applications and point of sale

`ravenrs-app` turns a Rust struct into an application. See `guides` with `document: "APP-FRAMEWORK"`. For an application used all day from the keyboard, `App::counter` gives a window with a bar of keys made from the actions, `KeyBar` and `ScanField` are the components behind it, and the same guide lists notices, work in the background and numbers.

Labels and receipts are ordinary reports made in the standard designer, and `ravenrs-report-label` sends them to ZPL, EPL, PPLA, TSPL and ESC/POS printers. It has not been tried on a physical printer.

`ravenrs-nfe`, `ravenrs-app-nfe`, `ravenrs-pagamento`, `ravenrs-pos` and `ravenrs-app-pos` are a **beta proof of concept for Brazilian point of sale**: they have never talked to a real SEFAZ, pinpad or payment provider, and production is blocked in the code. Never tell someone they can issue real fiscal documents with them. See `guides` with `document: "AUTOMACAO"`, `crates/ravenrs-nfe/README.md` for the local sandbox of the SEFAZ, and `crates/ravenrs-nfe/REGRAS.md` for where each rule came from.
<!-- ravenrs:skill:end -->
