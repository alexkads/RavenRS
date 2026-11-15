# ravenrs-platform-web

The browser backend of [RavenRS](https://github.com/alexkads/RavenRS). It takes a RavenRS view, mounts it on a `<canvas>` element of your page and keeps it running: it starts the GPU (WebGPU, or WebGL2 where WebGPU is missing), turns the browser's pointer, touch, wheel, keyboard, text-input, clipboard and drag-and-drop events into RavenRS events, draws a frame only when something changed, and mirrors the UI as invisible ARIA elements so screen readers can use it.

It is the web counterpart of `ravenrs-platform-native`, which does the same job with desktop windows. Your UI code (views, components, state) is the same for both; only the few lines that start the app differ. Below this crate sit `ravenrs-widgets` (the `Ui` that builds, lays out and hit-tests the views), `ravenrs-render` (the wgpu renderer) and `ravenrs-text` (shaping, with the fonts embedded).

**Most apps should not depend on this crate directly.** The `ravenrs` facade re-exports all of it as `ravenrs::web` when it is compiled for `wasm32`, next to the prelude with every widget. Depend on `ravenrs-platform-web` only if you assemble the lower crates yourself.

On any target other than `wasm32` this crate compiles to nothing, so a workspace that also builds for the desktop does not need to exclude it.

## Getting started

You need the WebAssembly target and the `wasm-bindgen` command-line tool, at the same version as the `wasm-bindgen` crate in your lock file (the RavenRS workspace pins `=0.2.130`):

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.130
```

The quickest start is `cargo ravenrs new my-app --target web` (or `npx create-ravenrs my-app --target web`), which writes everything below for you. By hand, an app is a library crate with `crate-type = ["cdylib", "rlib"]`:

```toml
[lib]
crate-type = ["cdylib", "rlib"]

[dependencies]
ravenrs = "*" # use the current release

[target.'cfg(target_arch = "wasm32")'.dependencies]
wasm-bindgen = "=0.2.130"
wasm-bindgen-futures = "0.4"
console_error_panic_hook = "0.1"
web-sys = { version = "0.3", features = ["console"] }
```

The Rust side starts the app when the module loads and keeps the returned `WebApp` alive. This is the whole program:

```rust,ignore
use std::cell::RefCell;

use ravenrs::prelude::*;
use ravenrs::web::{WebApp, WebOptions};
use wasm_bindgen::prelude::*;

fn app() -> impl View {
    let clicks = State::new(0_u32);
    let (add, read) = (clicks.clone(), clicks);
    VStack::new((
        Text::dynamic(move || format!("Clicked {} times", read.get())),
        Button::new("Click me").on_click(move || add.update(|n| *n += 1)),
    ))
    .spacing(12.0)
    .padding(32.0)
}

thread_local! {
    // Dropping the WebApp unmounts the app, so it is kept here for the life of the page.
    static APP: RefCell<Option<WebApp>> = const { RefCell::new(None) };
}

#[wasm_bindgen(start)]
pub fn main() {
    // Panics are printed to the browser console instead of a bare "unreachable".
    console_error_panic_hook::set_once();
    wasm_bindgen_futures::spawn_local(async {
        let options = WebOptions::new("raven-canvas").follow_system_theme();
        match ravenrs::web::start(app(), options).await {
            Ok(app) => APP.with(|a| *a.borrow_mut() = Some(app)),
            // The same message is already shown on the page, next to the canvas.
            Err(e) => web_sys::console::error_1(&e.to_string().into()),
        }
    });
}
```

The page gives the canvas its size with CSS and loads the module that `wasm-bindgen` writes:

```html
<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <style>
      html, body { height: 100%; margin: 0; overflow: hidden; }
      #raven-canvas { display: block; width: 100vw; height: 100vh; }
      .raven-error { position: fixed; inset: auto 16px 16px 16px; padding: 16px; }
    </style>
  </head>
  <body>
    <canvas id="raven-canvas" aria-label="My app"></canvas>
    <script type="module">
      import init from "./pkg/my_app.js";
      init();
    </script>
  </body>
</html>
```

Build, bind and serve it over HTTP (opening the file from disk does not load the module):

```sh
cargo build --lib --release --target wasm32-unknown-unknown
wasm-bindgen --target web --no-typescript --out-dir web/pkg \
  target/wasm32-unknown-unknown/release/my_app.wasm
python3 -m http.server -d web 8080
```

Inside the RavenRS repository, `scripts/build-web.sh <example>` does these steps for any example in `examples/`, with the size-tuned `wasm-release` profile and `wasm-opt` when it is installed.

## How it works

`start(root, options)` is `async` because the browser hands out the GPU adapter and device through promises. It never blocks the page's main thread and needs no threads and no `SharedArrayBuffer`, so no special response headers are required. In order it:

1. finds the `<canvas>` with `options.canvas_id` (`WebError::CanvasNotFound` if there is none, or if the element is not a canvas);
2. starts the graphics API chosen by `options.backend` (see [Browsers](#browsers));
3. sizes the canvas backing store to its CSS size times `devicePixelRatio`, capped at the GPU's largest texture, so drawing is sharp on high-density screens;
4. adds, right after the canvas, a hidden `<textarea>` that holds keyboard focus and, when `accessibility` is on, the ARIA mirror and a live region for announcements; then the event listeners and a `ResizeObserver`;
5. draws the first frame and returns a `WebApp`.

Everything then runs on the page's main thread, inside browser callbacks. An input event goes to the `Ui`; if it changed something, one `requestAnimationFrame` is requested, however many events arrive before it. Each frame rebuilds what changed, draws, presents and updates the ARIA mirror. When nothing is animating no frame is requested at all, so an idle page costs nothing; something that wakes later (a blinking caret, a timer) sets one `setTimeout` for that moment. A resize, a browser zoom or moving the window to another screen triggers a frame through the `ResizeObserver`, and the frame reads the new size and pixel ratio.

The `WebApp` owns all of this. Dropping it, or calling `WebApp::destroy`, removes every listener, the observer, the hidden textarea and the mirror, cancels the pending timer and frees the GPU resources; the canvas stays on the page. That is why the example keeps it in a `thread_local`: a `WebApp` dropped at the end of the `async` block unmounts the app right after it appears. To stop the app from JavaScript, export a function that takes it out of that slot.

### Options

`WebOptions::new(canvas_id)` gives the defaults, and each builder method changes one thing:

| Method | Default | Use it when |
|---|---|---|
| `theme(theme)` | Raven Dark | The app should open in another theme, such as `Theme::raven_light()`. |
| `follow_system_theme()` | off | The app should switch between Raven Dark and Raven Light with the system's `prefers-color-scheme`, now and whenever it changes, until the person picks a theme in the app. |
| `accessibility(on)` | on | Only to measure the cost of the screen-reader mirror; turning it off leaves screen-reader users with nothing. |
| `backend(backend)` | `WebBackend::Auto` | A page must use one API, see [Browsers](#browsers). |

The fields are public too, so a struct literal works as well.

### Keeping data: `IndexedDbStorage`

`IndexedDbStorage::open(name).await` opens (or creates) an IndexedDB database of that name and reads everything in it before returning, because the app's `Storage` must answer at once while IndexedDB answers later. After that, reads come from memory, and writes update memory and are sent to the browser in the background, without waiting. A write the browser rejects (a full quota, for instance) does not fail the call; `IndexedDbStorage::problem()` returns the browser's message for the last one. `open` fails with a message when the browser has no IndexedDB, as in some private windows; the app can then fall back to keeping nothing.

## What comes for free

Without a line of app code:

- **Text input that works with every keyboard.** Typing goes through the hidden textarea, not the canvas, so input methods for Chinese, Japanese and Korean, dead keys, emoji pickers and phone keyboards all deliver text. Tab, Enter, the arrows and Backspace stay inside the app instead of moving the page's focus. The textarea uses a 16 px font so iOS does not zoom the page when it gets focus.
- **Paste.** Ctrl+V or Cmd+V pastes plain text from the browser's `paste` event, delivered once.
- **Shortcuts shown the right way.** Menus and tooltips write shortcuts with the Mac keys when the browser runs on a Mac, iPhone or iPad, and with Ctrl elsewhere, whatever system the app was built on.
- **Touch.** A finger that moves more than 8 px scrolls instead of tapping, unless it started on something draggable such as a slider, and the scroll glides on after the finger lifts. The canvas sets `touch-action: none`, so the browser does not pan or zoom the page under the app.
- **Wheel and trackpad.** Line- and page-based wheels are converted to pixels; Shift turns a vertical wheel sideways; a trackpad pinch (which browsers send as a wheel with Ctrl) zooms in the app instead of zooming the page. The page is kept from scrolling only when the app scrolled something, so a RavenRS canvas embedded in a longer page does not trap the wheel.
- **The app's own context menu.** The browser's right-click menu is suppressed on the canvas.
- **System settings followed live.** `prefers-reduced-motion` turns animations down, and with `follow_system_theme` the theme follows `prefers-color-scheme`; both react when the setting changes.
- **Mouse cursor.** The canvas cursor follows what is under the pointer: arrow, hand, text beam, and column or row resize. Other cursor shapes show the arrow.
- **Files dropped from the system** reach the widget under the pointer, up to 1 GiB each.
- **Screen readers.** The UI is mirrored as ARIA elements over the canvas, focus is reported with `aria-activedescendant`, status messages are announced through a polite live region, and a screen reader's "click" on a mirrored element performs the action on that widget.
- **The diagnostics panel.** Ctrl+Shift+D (Cmd+Shift+D on a Mac) opens the same panel as on the desktop: frame times split into update, acquire, render and present, draw counts, the GPU adapter and the backend in use.

## What changes from the desktop

The `Ui` asks the platform for a few services. On the web they are the browser's, with the browser's rules:

| Service | On the web | What the app sees |
|---|---|---|
| Opening files | A hidden `<input type="file">`, with the request's extensions as `accept` | The browser's file chooser. If the person cancels, the browser reports nothing, so the completion is never called; do not leave the UI waiting on it. A file above the request's size limit arrives with its name and size and no bytes, so the app can say why it refused it. |
| Saving files | A download through a temporary link with `download` set | The file goes where the browser puts downloads, possibly without asking; there is no "Save as" with a chosen folder. |
| Copying | `navigator.clipboard.writeText` | Copying works; if the browser refuses (no permission, or a page not served over HTTPS or from `localhost`), nothing is copied and a warning is logged. |
| Pasting from code | Not available | Reading the clipboard is asynchronous and permission-gated in browsers, so an app action such as a "Paste" menu item gets no text. Ctrl+V and Cmd+V still paste, as described above. |
| Opening a link | `location.assign(url)` | The page itself navigates to the address, which leaves the app. |
| Full screen | The Fullscreen API on the whole page | Inside an `<iframe>` it works only when the frame allows full screen; otherwise the widget sees it as unavailable. |
| Sound and speech | Web Audio and the browser's speech synthesis | Browsers only let a page make sound after someone clicked in it; before that nothing is heard and nothing fails. |
| Storage | `IndexedDbStorage` | See above. The data lives in the browser profile, per site. |

**Fonts.** Browsers do not let WebAssembly read the system's fonts, so RavenRS embeds its own: Inter regular and semibold for text, an icon font, and Fira Mono for code when the `mono` feature of `ravenrs-text` is on. The desktop uses the same files, so text looks the same everywhere. A font loaded by the page's CSS is not used by the canvas; a font the app needs has to be registered with RavenRS from its bytes.

**What does not exist on the web.** There are no extra windows, no system tray, no system notifications, no native open and save dialogs and no "open with the system app". Those live in `ravenrs::native`, which is compiled only for non-`wasm32` targets, so code that calls them must sit behind `#[cfg(not(target_arch = "wasm32"))]`; otherwise the web build fails to compile rather than failing at run time. Everything drawn on the canvas (every widget, and the menus and dialogs RavenRS draws itself) works the same.

## Browsers

`WebBackend` decides which graphics API draws the canvas:

- **`Auto`** (the default) checks whether `navigator.gpu` exists. If it does, it starts WebGPU, and if WebGPU fails to give an adapter, a device or a surface, it tries WebGL2. If `navigator.gpu` is missing it goes straight to WebGL2. So WebGPU is used wherever the browser offers it, and WebGL2 covers nearly every other browser and phone.
- **`WebGpu`** uses WebGPU only. A browser without `navigator.gpu` gets `WebError::WebGpuUnavailable`.
- **`WebGl2`** uses WebGL2 only. Choose it for a page that must open everywhere and behave the same, including on machines where WebGPU exists but misbehaves.

A canvas keeps the first kind of context it hands out, so when `Auto` moves on to WebGL2 it replaces the `<canvas>` element with a fresh copy that has the same id and attributes. A reference to the canvas taken by the page's JavaScript before `start` then points at the removed element; look it up again by id after `start` returns.

The diagnostics panel shows which API is running.

## When something goes wrong

**The canvas stays empty.** Check in this order:

1. **The browser console, and the page.** Errors from `start` are also written into the page: a `<div class="raven-error" role="alert">` right after the canvas holds the message. It has no style of its own, so give `.raven-error` one; with a full-window canvas and `overflow: hidden`, an unstyled message may be off screen.
2. **The `WebApp` was dropped.** If the app appears for a moment, or never draws although `start` succeeded, check that the returned `WebApp` is stored somewhere that outlives the `async` block.
3. **The canvas has no size.** The backing store follows the canvas's CSS size; a canvas at `height: 100%` inside a parent with no height measures 0 and is drawn at 1 px. Give it a size in CSS.
4. **The module did not load.** A 404 on the `.js` or `.wasm` file, a page opened with `file://`, or a server that sends `.wasm` without the `application/wasm` type stops it before any RavenRS code runs; the network tab shows it.
5. **A panic.** Without `console_error_panic_hook::set_once()` a panic reads as `RuntimeError: unreachable` with no message. With the hook, the console shows the panic message and where it happened.

**The errors `start` returns** (`WebError`, whose `Display` text is written for the person using the page):

| Error | Meaning | What to try |
|---|---|---|
| `CanvasNotFound(id)` | No element has that id, or it is not a `<canvas>`. | Check the id in `WebOptions::new` against the HTML, and that the script runs after the canvas exists (a `type="module"` script at the end of `<body>` does). |
| `WebGpuUnavailable` | `WebBackend::WebGpu` was asked for and `navigator.gpu` is missing. | Use `Auto` or `WebGl2`, or a browser with WebGPU and hardware acceleration on. |
| `Gpu(GpuError::NoAdapter(_))` | The browser offered no GPU adapter for the API (with `Auto`, for the last one tried). | Hardware acceleration is off, the GPU is blocked by the browser, or WebGL is disabled. Check the browser's GPU status page. |
| `Gpu(GpuError::Device(_))` | The adapter refused to create a device. | Often a driver problem; try `WebGl2`, another browser or updated drivers. |
| `Gpu(GpuError::Surface(_))` | The canvas could not be set up for drawing. | Usually other code on the page already took a context from that canvas; give RavenRS a canvas of its own. |
| `Js(message)` | A browser API call failed; the text is the browser's message. | Read the message; it names the API that failed. |

**The GPU is lost while the app runs** (a driver reset, the system reclaiming the GPU, a tab long in the background). The crate notices, drops the device and starts a new one on the same API at the next frame, which the next input, resize or animation brings; the UI and its state are kept. If the new device does not start, the error panel appears next to the canvas. A frame whose surface is outdated is set up again and redrawn.

**Seeing what the crate reports.** It logs through `tracing`: which API failed before the fallback, a lost GPU and its recovery, refused clipboard writes, sounds, links and full screen, and skipped frames. A WebAssembly app has no subscriber by default, so these messages go nowhere until the app installs a `tracing` subscriber that writes to the browser console.

**In a bug report**, include the browser and its version, the operating system, the console output (with the panic hook and a console subscriber installed), and what the diagnostics panel (Ctrl+Shift+D) says about the adapter and backend. If `Auto` misbehaves, say whether `WebBackend::WebGl2` and `WebBackend::WebGpu` behave differently.

## License

MIT OR Apache-2.0.
