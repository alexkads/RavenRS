# ravenrs-platform-native

The part of [RavenRS](https://github.com/alexkads/RavenRS) that runs an app on Windows, macOS and Linux. You give it a view; it opens the window, draws it on the GPU, turns the mouse, keyboard and touchpad into RavenRS events, connects the UI to the operating system (clipboard, files, links, sound, notifications, the icon beside the clock, screen readers) and keeps doing all of that until the window closes.

Your view never talks to the system directly. It asks the runtime of `ravenrs-widgets` for a file, a sound or the clipboard, and this crate is what answers on the desktop. `ravenrs-platform-web` answers the same questions in the browser, which is why the same view runs in both places.

```text
your View ──► ravenrs-widgets (Ui: build, layout, events, Scene)
                    ▲      │
          events    │      ▼ Scene
ravenrs-platform-native ──► ravenrs-render (wgpu) ──► the screen
   │  winit: window, input        AccessKit: screen readers
   └─ the system: clipboard, dialogs, links, sound, notifications, tray
```

Most apps reach it through the [`ravenrs`](https://crates.io/crates/ravenrs) facade, as `ravenrs::native`. Depend on this crate directly only when you want the desktop backend without the rest of the facade.

## Opening a window

```rust,no_run
use ravenrs_platform_native::{PlatformError, WindowOptions, run};
use ravenrs_widgets::Text;

fn main() -> Result<(), PlatformError> {
    let options = WindowOptions::new("Notes").size(800.0, 600.0);
    run(Text::new("Hello"), options)
}
```

`run` takes over the thread that calls it and returns only when the window closes, or with the error that kept it from opening. Call it once, from `main`: a process has a single event loop, and macOS requires it on the main thread.

### Which function to start with

| Function | Use it when | What you lose with the plainer one |
|---|---|---|
| `run` | Examples, tools, anything that does not need to be recognized as an installed app. | — |
| `run_with_icon` | The window should show your icon in its title bar and the task bar. | `run` shows the system's generic icon. |
| `run_as` with an `AppIdentity` | A real, installed app. | Without the id, Linux cannot tie the window to your `.desktop` file, so GNOME shows a generic icon and the dock does not group the window with your launcher. You also give up the translated UI (`labels`), a shared diagnostics panel, bug reports, and closing to the tray. |

```rust,no_run
use ravenrs_platform_native::{AppIdentity, SystemTray, WindowOptions, run_as};
use ravenrs_widgets::{Labels, Text};

let identity = AppIdentity::new("notes") // the name of `notes.desktop` on Linux
    .labels(Labels::pt_br())
    .new_issue_url("https://github.com/example/notes/issues/new")
    .close_to_tray(SystemTray::new("Notes").open("Open Notes").quit("Quit"));
run_as(Text::new("Hello"), WindowOptions::new("Notes"), identity)?;
# Ok::<(), ravenrs_platform_native::PlatformError>(())
```

## How a frame happens

The app draws only when something changed. Between frames the event loop sleeps, so an idle window uses no CPU or GPU.

1. **Something happens:** input arrives, a `State` the view reads changes, an animation needs its next step, or another thread wakes the UI.
2. **The UI rebuilds** what changed and produces a `Scene`. Asking for the next frame is the UI's decision. This crate only asks it, through `Ui::needs_redraw` and `Ui::next_wake`.
3. **`ravenrs-render` draws the scene** into the window's surface, and the frame is presented in step with the display (vsync).
4. **If an animation is running,** the loop sleeps only until the moment of its next frame. Otherwise it sleeps until the next event.

What follows from that:

- **A view never polls.** A timer or a loop that asks "did anything change?" keeps the app awake. Change a `State` and the frame follows.
- **Work in another thread wakes the UI with a `UiWaker`.** Get it from `BuildCx::ui_waker`, move it to the thread, send the result through a channel and call `wake`. The next build runs on the UI thread and reads the channel. Everything else in the UI belongs to the UI thread. The `UiWaker` documentation in `ravenrs-widgets` has the complete example.
- **If the GPU is lost,** for example after a driver update, a GPU reset or waking from sleep, the device and the surface are recreated on the next frame and the app carries on. The log says `GPU device lost; recreating it`.
- **On macOS a new window's first frame is drawn while the window is still hidden.** That frame is the slow one, because it compiles the shaders and rasterizes the glyphs. Nobody sees a black window.

## What users get without a line of your code

| Behavior | Detail |
|---|---|
| Clipboard | The system clipboard. Without a display server, as over SSH, copy and paste still work inside the app. |
| Scrolling | One wheel notch scrolls 48 px, three 16 px lines, as browsers do. Shift turns vertical scrolling sideways. Touchpads scroll by their exact pixels. |
| Zoom and pinch | Ctrl + wheel zooms, at 400 px of wheel for twice the size. That is also how touchpads on Windows send a pinch. Touchpad pinches arrive on macOS through winit and on Wayland through `ravenrs-wayland-gestures`, because winit does not deliver them there. |
| Double click | Two primary clicks less than 450 ms and 5 px apart. |
| Drag and drop of files | Files dropped on the window go to the view under the pointer. A file over 1 GiB arrives with its name and size but no bytes, so the view can say why it refused it. |
| Links | Only `http` and `https` addresses open, in the system's browser. Anything else is refused and logged. |
| Full screen | A borderless window over the monitor the window is on. |
| Sound and voice | Sounds play through the default output, which closes after 2 s of silence. Speech uses `say` on macOS, the built-in Windows voice through PowerShell, and `espeak-ng` on Linux. Without `espeak-ng` there is no voice. |
| Screen readers | VoiceOver, Narrator, NVDA and Orca read the UI through AccessKit. The accessibility tree is built and sent only while a screen reader is listening, so it costs nothing otherwise. |
| Diagnostics panel | Ctrl+Shift+D, or Cmd+Shift+D on a Mac, opens it in any app. See [When something goes wrong](#when-something-goes-wrong). |

## Services you call

These run when the view asks for them, usually from an event handler such as `on_click`.

- **`SaveDialog` and `OpenDialog`:** the system's own file dialogs. They **block** the UI thread until the person answers, which is fine because the person is looking at the dialog. They must run on the UI thread, where event handlers already run. The save dialog asks before replacing a file, so the path it returns can be written. Views that use the file inputs of `ravenrs-widgets` get these dialogs without calling them.
- **`open_with_system`:** opens a file with the program the system uses for it, such as a PDF in its viewer. It does not wait for that program.
- **`open_url`:** opens a web page in the browser. It refuses anything that is not `http` or `https`, so text from a document cannot run a program or open a local file.
- **`SystemNotifier`:** a notification in the system's place: the Notification Center on macOS, a toast on Windows, the notification server on Linux. `send` **waits** for the system to answer, so call it from another thread. `Ok` means the system took the notification, not that the person saw it, because they may have notifications turned off.
- **`SystemTray`**, given to `AppIdentity::close_to_tray`:
  - Closing the window keeps the app running behind an icon beside the clock. The icon's menu opens the window again or really quits.
  - Opening the app a second time, from the launcher or the Dock, brings the same window back instead of starting a second copy over the same data. Copies find each other on a local port derived from the app's id and the user's name, so two people on one computer each keep their own app.
  - It needs `AppIdentity::icon`, because the tray has nothing else to show.

## Features

| Feature | Turns on | Pulls in |
|---|---|---|
| `tray` | The icon beside the clock for `close_to_tray`. | `tray-icon` on macOS and Windows, and `objc2` to notice clicks on the Dock. `ksni` on Linux, a StatusNotifierItem in pure Rust over D-Bus, so there is no GTK to build. |
| `notifications` | `SystemNotifier` actually showing something. | `notify-rust`, which speaks D-Bus in pure Rust on Linux. |

`SystemTray` and `SystemNotifier` exist with the features off, so the same app code compiles either way. Without `tray`, closing the window closes the app. Without `notifications`, `send` returns `Err("this build has no system notifications")`.

## Platform behavior

| | macOS | Windows | Linux, X11 | Linux, Wayland |
|---|---|---|---|---|
| GPU API | Metal | DX12 or Vulkan | Vulkan or GL | Vulkan or GL |
| Window icon | From the app bundle. `icon` is ignored. | `icon` | `icon` | From the `.desktop` file named by the `AppIdentity` id. A window cannot carry its own. |
| App id | The bundle | The executable | `WM_CLASS`, which must match `StartupWMClass` in the `.desktop` file | The app id, which must be the `.desktop` file's name without the extension |
| Tray icon | Yes | Yes | Where something hosts a StatusNotifierItem: KDE, or GNOME with the AppIndicator extension | The same. **Plain GNOME has no tray:** there, closing the window closes the app, because an app hidden behind an icon nobody draws could never be closed. |
| Closing to the tray | The window is hidden and the app leaves the front, so a click on the Dock brings it back. | The window is hidden and comes back with its last frame. | The window is hidden and comes back with its last frame. | Wayland cannot hide a window, so it is destroyed. Its size, position and maximized state are kept, and a new window opens in the same place. The UI and its state are kept. |
| Notification click | Not reported by the system | Not reported by the system | With `SystemNotifier::opens_window`, a click brings the app's window back | The same as X11 |

**Building on Linux** needs the ALSA headers for sound (`libasound2-dev` on Debian and Ubuntu, `alsa-lib-devel` on Fedora) and `pkg-config`. At run time, the file dialogs go through the XDG desktop portal (`xdg-desktop-portal` and a backend for your desktop), and notifications and the tray need a D-Bus session. Without a portal, the dialogs do not open.

## When something goes wrong

**`run` returned an error.** `PlatformError` says which step failed:

- `EventLoop`: winit could not reach the window system. On Linux, check that `DISPLAY` or `WAYLAND_DISPLAY` is set. Over SSH there is no window system at all.
- `Window`: the window system refused to create the window. The message carries its reason.
- `Gpu`: no GPU adapter, device or surface could be created. This is the common one: an old or missing driver, or a virtual machine without 3D acceleration. On Linux, `WGPU_BACKEND=gl` tries OpenGL instead of Vulkan. Adding `LIBGL_ALWAYS_SOFTWARE=1` draws on the CPU: slowly, but the window opens.

**The app opens but misbehaves.** Open the diagnostics panel with Ctrl+Shift+D (Cmd+Shift+D on a Mac). To have it open from the first frame, set the `RAVENRS_DIAG` environment variable to any value. The panel shows:

- the GPU and the API in use, with warnings when drawing fell back to the CPU, when no GPU was found, or when the build is not optimized (a debug build is much slower);
- the time of each frame, phase by phase (update, acquire, render, present), and a timeline;
- what each view costs, the draw batches and the memory;
- a performance test that says whether the app is smooth and whether the GPU or the interface is to blame;
- the machine's report and a trace to copy.

With `AppIdentity::new_issue_url`, the panel starts a bug report on that page, filled with the report. The person reviews it and sends it there. `AppIdentity::diagnostics` shares the panel with your own UI, for a button that opens it or starts a performance test.

**The window does not open at all.** Call `diagnose()` instead of `run`, for example behind a `--diagnostics` option, and print the result. It finds the GPU the way a window would, without showing anything. It answers over SSH too, where it still lists the adapters. Its report has:

- the GPU;
- the system and, on Linux, the session type, the desktop and the Vulkan drivers installed;
- the build;
- the variables that change which GPU is used (`WGPU_BACKEND`, `WGPU_ADAPTER_NAME`, `WGPU_POWER_PREF`, `VK_ICD_FILENAMES`, `LIBGL_ALWAYS_SOFTWARE` and others). Paths in those variables are cut to their file names, because the report is made to be shared.

```rust,no_run
if std::env::args().any(|a| a == "--diagnostics") {
    print!("{}", ravenrs_platform_native::diagnose());
}
```

**Logs.** Everything above logs through `tracing`. Install a subscriber in `main`, such as `tracing-subscriber` with `RUST_LOG=info,wgpu=warn`, to see why a clipboard, an icon or a voice was not available.

## More

- [API documentation](https://docs.rs/ravenrs-platform-native)
- [Getting started](https://github.com/alexkads/RavenRS/blob/master/docs/GETTING_STARTED.md)
- `examples/family-finance/src/main.rs` in the repository: a real app that uses `run_as`, the tray, the diagnostics panel and `diagnose`.

## License

MIT OR Apache-2.0.
