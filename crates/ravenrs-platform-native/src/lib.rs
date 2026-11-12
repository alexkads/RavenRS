#![doc = include_str!("../README.md")]

mod access;
mod diagnostics;
mod files;
mod notice;
mod sound;
mod tray;

/// The system clipboard through `arboard`. When it cannot be opened (no
/// display server, say), copy and paste stay inside the app.
struct SystemClipboard(std::cell::RefCell<arboard::Clipboard>);

impl ravenrs_widgets::ClipboardProvider for SystemClipboard {
    fn read_text(&self) -> Option<String> {
        self.0.borrow_mut().get_text().ok()
    }

    fn write_text(&self, text: &str) {
        if let Err(e) = self.0.borrow_mut().set_text(text) {
            tracing::warn!(error = %e, "could not write to the clipboard");
        }
    }
}

use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use std::rc::Rc;

use ravenrs_core::{
    AccessTree, CursorIcon, Event, FrameSample, ImageData, Key, Modifiers, Point, PointerButton, Size,
};
use ravenrs_render::{GpuContext, GpuError, Renderer, preferred_surface_format, surface_report, wgpu};
use ravenrs_style::Theme;
use ravenrs_widgets::{Diagnostics, DiagnosticsHost, Labels, Localized, Ui, View};
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalSize, PhysicalPosition, PhysicalSize};
use winit::event::StartCause;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy, OwnedDisplayHandle};
use winit::keyboard::{Key as WinitKey, ModifiersState, NamedKey};
use winit::window::{Icon, Window, WindowAttributes, WindowId};

pub use diagnostics::diagnose;
pub use files::{OpenDialog, SaveDialog, open_url, open_with_system};
pub use notice::SystemNotifier;
pub use tray::SystemTray;

/// Wheel pixels that ask for twice the size, with Ctrl held.
const WHEEL_PER_ZOOM: f32 = 400.0;
/// Set to anything, the diagnostics panel is open from the start.
const DIAGNOSTICS_VARIABLE: &str = "RAVENRS_DIAG";

/// What reaches the event loop from outside it.
#[derive(Debug)]
enum UserEvent {
    /// From the screen reader.
    Access(accesskit_winit::Event),
    /// A [`UiWaker`](ravenrs_widgets::UiWaker) fired, on some other thread.
    Wake,
    /// A touchpad pinch on Wayland, which winit does not deliver.
    Pinch(f32),
    /// From the icon beside the clock, or from opening the app a second time.
    Tray(tray::TrayAction),
}

impl From<accesskit_winit::Event> for UserEvent {
    fn from(event: accesskit_winit::Event) -> Self {
        Self::Access(event)
    }
}

/// How the main window opens: its title, size and starting theme.
///
/// Sizes are in logical pixels, which the system scales for the screen, so 800 is the
/// same apparent width on a regular and on a high-density display. The defaults are a
/// 960 × 640 window that the person cannot shrink below 320 × 240, with the default theme.
#[derive(Clone, Debug)]
pub struct WindowOptions {
    /// Window title.
    pub title: String,
    /// Initial inner size in logical pixels.
    pub size: Size,
    /// Smallest inner size the user can resize to, in logical pixels.
    pub min_size: Option<Size>,
    /// Theme the UI starts with. The view can change it later, as with a light and dark
    /// switch; this is only the first frame's.
    pub theme: Theme,
}

impl Default for WindowOptions {
    fn default() -> Self {
        Self {
            title: "RavenRS".into(),
            size: Size::new(960.0, 640.0),
            min_size: Some(Size::new(320.0, 240.0)),
            theme: Theme::default(),
        }
    }
}

impl WindowOptions {
    /// Default options with the given window title.
    ///
    /// ```
    /// use ravenrs_platform_native::WindowOptions;
    /// use ravenrs_style::Theme;
    ///
    /// let options = WindowOptions::new("Notes").size(800.0, 600.0).theme(Theme::raven_light());
    /// assert_eq!(options.size.width, 800.0);
    /// ```
    pub fn new(title: impl Into<String>) -> Self {
        Self { title: title.into(), ..Self::default() }
    }

    /// Sets the initial inner size in logical pixels.
    pub fn size(mut self, width: f32, height: f32) -> Self {
        self.size = Size::new(width, height);
        self
    }

    /// Sets the theme the UI starts with.
    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }
}

/// Why the app could not open its window or stopped with an error.
///
/// Returned by [`run`], [`run_as`] and [`run_with_icon`]. Once the window is on screen,
/// trouble that the app can survive is logged instead: a lost GPU is recreated, and
/// a missing clipboard, icon or voice is done without. When the window will not open,
/// [`diagnose`] says which GPU and drivers the machine has. The crate documentation has
/// a section on what to try for each of these.
#[derive(Debug)]
pub enum PlatformError {
    /// The window system could not be reached, or the event loop stopped with an error.
    /// On Linux this is usually a session with neither `DISPLAY` nor `WAYLAND_DISPLAY`,
    /// as over SSH. It also happens when the app tries to run a second event loop:
    /// a process has one, so call `run` once.
    EventLoop(String),
    /// The window system refused to create the window; the message carries its reason.
    /// It can also come later, when a window closed to the tray on Wayland could not be
    /// created again.
    Window(String),
    /// No GPU adapter, device or drawing surface could be created: an old or missing
    /// driver, or a virtual machine without 3D acceleration. On Linux, `WGPU_BACKEND=gl`
    /// tries OpenGL instead of Vulkan, and `LIBGL_ALWAYS_SOFTWARE=1` with it draws on the CPU.
    Gpu(GpuError),
}

impl fmt::Display for PlatformError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EventLoop(e) => write!(f, "event loop error: {e}"),
            Self::Window(e) => write!(f, "window could not be created: {e}"),
            Self::Gpu(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for PlatformError {}

impl From<GpuError> for PlatformError {
    fn from(e: GpuError) -> Self {
        Self::Gpu(e)
    }
}

/// Opens a window showing `root` and runs until the window is closed.
///
/// This takes over the calling thread: it sets up the window, the GPU and the system's
/// services, then runs the event loop, drawing a frame only when something changed.
/// It returns `Ok` when the person closes the window, and an error only when the app
/// could not start or the loop failed. Call it once, from `main`, on the main thread,
/// which macOS requires.
///
/// The window has the system's generic icon, and Linux cannot tie it to a `.desktop`
/// file. For an installed app, use [`run_as`]; for just an icon, [`run_with_icon`].
///
/// ```no_run
/// use ravenrs_platform_native::{WindowOptions, run};
/// use ravenrs_widgets::Text;
///
/// run(Text::new("Hello"), WindowOptions::new("Hello"))?;
/// # Ok::<(), ravenrs_platform_native::PlatformError>(())
/// ```
pub fn run(root: impl View + 'static, options: WindowOptions) -> Result<(), PlatformError> {
    run_window(root, options, AppIdentity::anonymous(None))
}

/// Who the app is to the system, and what it asks of the desktop: given to [`run_as`].
///
/// The id is how the system recognizes the window as belonging to the installed app,
/// to show its icon and group it with its launcher. On top of it, the identity carries
/// what an installed app usually wants: the words of the UI ([`labels`](Self::labels)),
/// a diagnostics panel shared with the app ([`diagnostics`](Self::diagnostics)), a page for
/// bug reports ([`new_issue_url`](Self::new_issue_url)), a window icon ([`icon`](Self::icon))
/// and closing to the tray ([`close_to_tray`](Self::close_to_tray)).
///
/// On Linux the id is what ties the window to the app's `.desktop` file. Wayland
/// has no way for a window to carry an icon: GNOME shows the one in the
/// `.desktop` file whose name, without the extension, is the window's id. On
/// X11 the id must match the file's `StartupWMClass`.
///
/// ```
/// use ravenrs_platform_native::AppIdentity;
///
/// // For a file named `notes.desktop`.
/// let identity = AppIdentity::new("notes");
/// ```
#[derive(Clone, Debug)]
pub struct AppIdentity {
    id: String,
    icon: Option<ImageData>,
    labels: Option<Labels>,
    diagnostics: Option<Diagnostics>,
    new_issue_url: Option<String>,
    tray: Option<SystemTray>,
}

impl AppIdentity {
    /// No name: what [`run`] and [`run_with_icon`] open as.
    fn anonymous(icon: Option<ImageData>) -> Self {
        Self { id: String::new(), icon, labels: None, diagnostics: None, new_issue_url: None, tray: None }
    }

    /// The app known as `id`, which is the name of its `.desktop` file on Linux.
    pub fn new(id: impl Into<String>) -> Self {
        Self { id: id.into(), icon: None, labels: None, diagnostics: None, new_issue_url: None, tray: None }
    }

    /// Sets the words of the whole UI, including the diagnostics panel and the texts the
    /// components show by themselves, such as a dialog's buttons or an empty list's message.
    pub fn labels(mut self, labels: Labels) -> Self {
        self.labels = Some(labels);
        self
    }

    /// Shares `diagnostics` with the app, for a button of its own that opens the
    /// panel or starts a performance test.
    pub fn diagnostics(mut self, diagnostics: Diagnostics) -> Self {
        self.diagnostics = Some(diagnostics);
        self
    }

    /// Sets the page for new issues, so the diagnostics panel can start a bug
    /// report there. The person reviews it and sends it on that page.
    pub fn new_issue_url(mut self, url: impl Into<String>) -> Self {
        self.new_issue_url = Some(url.into());
        self
    }

    /// An icon for the title bar and the task bar on Windows and on Linux with
    /// X11. macOS takes an app's icon from its bundle instead.
    pub fn icon(mut self, icon: ImageData) -> Self {
        self.icon = Some(icon);
        self
    }

    /// Closing the window keeps the app running behind an icon beside the clock, which
    /// shows [`Self::icon`] and needs it. Opening the app again, or clicking its icon in the
    /// Dock on macOS, brings the same window back. See [`SystemTray`] for where the system has no such icon.
    ///
    /// It also makes the app a single copy: launching it while it runs opens the window
    /// of the copy that is running, and the second launch exits. That keeps two copies
    /// from working on the same data. Needs the `tray` feature, and a non-empty id.
    pub fn close_to_tray(mut self, tray: SystemTray) -> Self {
        self.tray = Some(tray);
        self
    }
}

/// Like [`run`], for an installed app that the system should recognize as itself.
///
/// The [`AppIdentity`] ties the window to the app's `.desktop` file on Linux, so its icon
/// and launcher are found, and turns on what an installed app needs: the language of the
/// UI, bug reports from the diagnostics panel and closing to the tray. With
/// [`close_to_tray`](AppIdentity::close_to_tray), a second launch of the app opens the
/// window of the copy already running and returns `Ok` at once.
///
/// ```no_run
/// use ravenrs_platform_native::{AppIdentity, WindowOptions, run_as};
/// use ravenrs_widgets::{Labels, Text};
///
/// let identity = AppIdentity::new("notes").labels(Labels::pt_br());
/// run_as(Text::new("Olá"), WindowOptions::new("Notas"), identity)?;
/// # Ok::<(), ravenrs_platform_native::PlatformError>(())
/// ```
pub fn run_as(
    root: impl View + 'static,
    options: WindowOptions,
    identity: AppIdentity,
) -> Result<(), PlatformError> {
    run_window(root, options, identity)
}

/// Like [`run`], with an icon for the window's title bar and the task bar on
/// Windows and Linux with X11. macOS takes an app's icon from its bundle instead, and
/// Wayland from the app's `.desktop` file, which only [`run_as`] can name.
///
/// For a tool or an example that wants its own icon without being an installed app.
pub fn run_with_icon(
    root: impl View + 'static,
    options: WindowOptions,
    icon: ImageData,
) -> Result<(), PlatformError> {
    run_window(root, options, AppIdentity::anonymous(Some(icon)))
}

fn run_window(
    root: impl View + 'static,
    options: WindowOptions,
    identity: AppIdentity,
) -> Result<(), PlatformError> {
    let AppIdentity { id, icon, labels, diagnostics, new_issue_url, tray } = identity;
    let app_id = (!id.is_empty()).then_some(id);
    let single_copy = app_id.as_deref().filter(|_| tray.is_some());
    if single_copy.is_some_and(tray::ask_running_copy) {
        tracing::info!("the app was already running, and opened its window");
        return Ok(());
    }
    let event_loop = EventLoop::<UserEvent>::with_user_event()
        .build()
        .map_err(|e| PlatformError::EventLoop(e.to_string()))?;
    event_loop.set_control_flow(ControlFlow::Wait);
    let diagnostics = diagnostics.unwrap_or_default();
    diagnostics.set_open(std::env::var_os(DIAGNOSTICS_VARIABLE).is_some());
    if let Some(url) = new_issue_url {
        diagnostics.set_bug_reporter(url, |url| open_url(url).map_err(|e| e.to_string()));
    }
    let root = DiagnosticsHost::new(root, diagnostics.clone());
    let mut ui = match labels {
        Some(labels) => Ui::new(Localized::new(labels, root), options.theme),
        None => Ui::new(root, options.theme),
    };
    let wake = event_loop.create_proxy();
    ui.set_remote_waker(move || {
        // It only fails once the loop is gone, when there is nothing left to wake.
        let _ = wake.send_event(UserEvent::Wake);
    });
    match arboard::Clipboard::new() {
        Ok(clipboard) => ui.set_clipboard(SystemClipboard(std::cell::RefCell::new(clipboard))),
        Err(e) => tracing::warn!(error = %e, "system clipboard unavailable; using an in-app clipboard"),
    }
    ui.set_sound(sound::SystemSound::default());
    ui.set_file_picker(files::SystemFilePicker);
    ui.set_links(SystemLinks);
    let screen_window = ScreenWindow::default();
    ui.set_full_screen(SystemFullScreen(screen_window.clone()));
    let answer = event_loop.create_proxy();
    let tray_notify: tray::Notify = Arc::new(move |action| {
        // It only fails once the loop is gone, when there is nothing left to tell.
        let _ = answer.send_event(UserEvent::Tray(action));
    });
    if let Some(id) = single_copy
        && let Err(e) = tray::listen_for_other_copies(id, tray_notify.clone())
    {
        tracing::warn!(error = %e, "opening the app again will start a second copy");
    }
    let mut app = App {
        display: event_loop.owned_display_handle(),
        diagnostics,
        proxy: event_loop.create_proxy(),
        access: None,
        ui,
        options,
        icon,
        app_id,
        window: None,
        screen_window,
        tray,
        tray_notify,
        in_tray: None,
        away_in_tray: false,
        place: None,
        maximized: false,
        pointer: None,
        modifiers: Modifiers::default(),
        last_click: None,
        started: std::time::Instant::now(),
        looking: false,
        error: None,
    };
    event_loop.run_app(&mut app).map_err(|e| PlatformError::EventLoop(e.to_string()))?;
    match app.error {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

struct WindowState {
    // Field order matters: the surface must drop before the window.
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    gpu: GpuContext,
    renderer: Renderer,
    device_lost: Arc<AtomicBool>,
    cursor: CursorIcon,
    window: Arc<Window>,
}

/// The AccessKit adapter of the window and what it last sent.
struct Access {
    adapter: accesskit_winit::Adapter,
    /// A screen reader asked for the tree; updates are sent while true.
    active: bool,
    last: Option<AccessTree>,
}

struct App {
    /// Lets the GL backend reach the window system; without it, EGL has no
    /// display to present on and only software rendering may be left on Wayland.
    display: OwnedDisplayHandle,
    diagnostics: Diagnostics,
    ui: Ui,
    proxy: EventLoopProxy<UserEvent>,
    access: Option<Access>,
    options: WindowOptions,
    icon: Option<ImageData>,
    app_id: Option<String>,
    window: Option<WindowState>,
    /// The window full screen applies to, once there is one.
    screen_window: ScreenWindow,
    tray: Option<SystemTray>,
    tray_notify: tray::Notify,
    /// The icon on screen, while the window is closed behind it.
    in_tray: Option<tray::Shown>,
    /// The app was seen in the background since it went to the icon, so coming to the
    /// front now is someone asking for it, as with a click on the Dock.
    away_in_tray: bool,
    /// Where the window was when closed behind the icon, for opening it there.
    place: Option<PhysicalPosition<i32>>,
    maximized: bool,
    /// Last pointer position in logical pixels.
    pointer: Option<Point>,
    modifiers: Modifiers,
    /// Time and place of the last primary release, to detect double clicks.
    last_click: Option<(std::time::Instant, Point)>,
    started: std::time::Instant,
    /// The diagnostics panel is open, so the frames are being looked at closely.
    looking: bool,
    error: Option<PlatformError>,
}

/// On macOS a closed window leaves the app in front, and then a click on the Dock would
/// not change anything anyone could see.
#[cfg(all(feature = "tray", target_os = "macos"))]
fn leave_the_front(event_loop: &ActiveEventLoop) {
    use winit::platform::macos::ActiveEventLoopExtMacOS as _;
    event_loop.hide_application();
}

#[cfg(not(all(feature = "tray", target_os = "macos")))]
fn leave_the_front(_event_loop: &ActiveEventLoop) {}

/// The same setting serves Wayland's app id and X11's `WM_CLASS`.
#[cfg(any(
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd"
))]
fn with_app_id(attributes: WindowAttributes, id: &str) -> WindowAttributes {
    use winit::platform::wayland::WindowAttributesExtWayland as _;
    attributes.with_name(id, id)
}

/// Other systems know the app by its bundle or its executable.
#[cfg(not(any(
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd"
)))]
fn with_app_id(attributes: WindowAttributes, _id: &str) -> WindowAttributes {
    attributes
}

impl App {
    fn create_window(&mut self, event_loop: &ActiveEventLoop) -> Result<WindowState, PlatformError> {
        // Hidden until `show_window`, so nobody waits looking at an empty window.
        // AccessKit also requires it until the adapter is attached.
        let mut attributes = Window::default_attributes()
            .with_visible(false)
            .with_title(self.options.title.clone())
            .with_inner_size(LogicalSize::new(self.options.size.width, self.options.size.height));
        if let Some(place) = self.place {
            attributes = attributes.with_position(place);
        }
        attributes = attributes.with_maximized(self.maximized);
        if let Some(min) = self.options.min_size {
            attributes = attributes.with_min_inner_size(LogicalSize::new(min.width, min.height));
        }
        if let Some(id) = &self.app_id {
            attributes = with_app_id(attributes, id);
        }
        if let Some(image) = &self.icon {
            match Icon::from_rgba(image.pixels().to_vec(), image.width(), image.height()) {
                Ok(icon) => attributes = attributes.with_window_icon(Some(icon)),
                Err(e) => tracing::warn!(error = %e, "could not use the window icon"),
            }
        }
        let window =
            Arc::new(event_loop.create_window(attributes).map_err(|e| PlatformError::Window(e.to_string()))?);
        let adapter =
            accesskit_winit::Adapter::with_event_loop_proxy(event_loop, &window, self.proxy.clone());
        self.access = Some(Access { adapter, active: false, last: None });
        *self.screen_window.borrow_mut() = Arc::downgrade(&window);
        self.init_gpu(window)
    }

    /// Closes the window behind the icon, keeping the UI and its state. `false` if the
    /// system shows no icon, and only closing the app is left.
    ///
    /// The window is only hidden, so that it comes back at once, with its last frame.
    /// Wayland cannot hide one, so there it is destroyed, and that system shows a new
    /// window only once it has something to show.
    fn close_to_tray(&mut self, event_loop: &ActiveEventLoop) -> bool {
        let (Some(tray), Some(icon), Some(state)) = (&self.tray, &self.icon, &self.window) else {
            return false;
        };
        let id = self.app_id.as_deref().unwrap_or_default();
        match tray::show(tray, id, icon, self.tray_notify.clone()) {
            Ok(shown) => self.in_tray = Some(shown),
            Err(e) => {
                tracing::info!(error = %e, "no icon beside the clock, so closing the window closes the app");
                return false;
            }
        }
        let window = &state.window;
        if !self.on_wayland() {
            window.set_visible(false);
            self.away_in_tray = false;
            leave_the_front(event_loop);
            return true;
        }
        let size = window.inner_size().to_logical::<f32>(window.scale_factor());
        self.options.size = Size::new(size.width, size.height);
        self.place = window.outer_position().ok();
        self.maximized = window.is_maximized();
        // These hold the window, which only closes when the last of them lets go.
        state.gpu.device.set_device_lost_callback(|_, _| {});
        self.ui.set_waker(|| {});
        self.access = None;
        self.pointer = None;
        self.window = None;
        true
    }

    /// Brings the window back, or to the front if it never left.
    fn open_window(&mut self, event_loop: &ActiveEventLoop) {
        self.in_tray = None;
        if let Some(state) = &self.window {
            state.window.set_visible(true);
            state.window.set_minimized(false);
            state.window.focus_window();
            state.window.request_redraw();
            return;
        }
        match self.create_window(event_loop) {
            Ok(state) => self.show_window(state),
            Err(e) => {
                tracing::error!(error = %e, "could not open the window again");
                self.error = Some(e);
                event_loop.exit();
            }
        }
    }

    /// Puts a new window on screen. On macOS its first frame, the slow one with the
    /// shaders and the glyphs, is drawn while hidden, so that nobody sees it black.
    fn show_window(&mut self, state: WindowState) {
        let window = state.window.clone();
        self.window = Some(state);
        if cfg!(target_os = "macos") {
            self.redraw();
        }
        window.set_visible(true);
        window.focus_window();
        // What was drawn while hidden does not count for the system.
        window.request_redraw();
    }

    /// Sends the accessibility tree to the screen reader if one is listening
    /// and the tree changed.
    fn update_access(&mut self) {
        let Some(access) = &mut self.access else { return };
        if !access.active {
            return;
        }
        let tree = self.ui.access_tree();
        if access.last.as_ref() == Some(&tree) {
            return;
        }
        let scale = f64::from(self.ui.scale());
        access.adapter.update_if_active(|| access::tree_update(&tree, scale));
        access.last = Some(tree);
    }

    fn init_gpu(&mut self, window: Arc<Window>) -> Result<WindowState, PlatformError> {
        let instance = diagnostics::new_instance(Some(&self.display), false);
        let surface =
            instance.create_surface(window.clone()).map_err(|e| GpuError::Surface(e.to_string()))?;
        let gpu = pollster::block_on(GpuContext::new(&instance, Some(&surface)))?;
        let caps = surface.get_capabilities(&gpu.adapter);
        let format = preferred_surface_format(&caps.formats)
            .ok_or_else(|| GpuError::Surface("the surface reports no supported formats".into()))?;
        let size = window.inner_size();
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::AutoVsync,
            desired_maximum_frame_latency: 2,
            alpha_mode: caps.alpha_modes.first().copied().unwrap_or(wgpu::CompositeAlphaMode::Auto),
            view_formats: vec![],
            color_space: wgpu::SurfaceColorSpace::Auto,
        };
        surface.configure(&gpu.device, &config);
        let described = surface_report(&caps, format, config.present_mode);
        let (display, adapter) = (self.display.clone(), gpu.adapter.clone());
        self.diagnostics.set_report_source(move || {
            diagnostics::gather(Some(&display), &adapter, Some(described.clone()))
        });
        let device_lost = Arc::new(AtomicBool::new(false));
        let (lost, lost_window) = (device_lost.clone(), window.clone());
        gpu.device.set_device_lost_callback(move |reason, message| {
            tracing::error!(?reason, message, "GPU device lost; recreating it");
            lost.store(true, Ordering::SeqCst);
            lost_window.request_redraw();
        });
        let renderer = Renderer::new(&gpu.device, &gpu.queue, format);

        let redraw = window.clone();
        self.ui.set_waker(move || redraw.request_redraw());
        self.update_viewport(&window, size);
        window.request_redraw();
        tracing::info!(?format, width = size.width, height = size.height, "window ready");
        Ok(WindowState { surface, config, gpu, renderer, device_lost, cursor: CursorIcon::Default, window })
    }

    fn update_viewport(&mut self, window: &Window, size: PhysicalSize<u32>) {
        let scale = window.scale_factor() as f32;
        let logical = Size::new(size.width as f32 / scale, size.height as f32 / scale);
        self.ui.set_viewport(logical, scale);
    }

    fn dispatch(&mut self, event: Event) {
        self.ui.set_time(self.started.elapsed().as_secs_f64());
        let redraw = self.ui.handle_event(&event);
        if let Some(state) = &mut self.window {
            let cursor = self.ui.cursor();
            if cursor != state.cursor {
                state.window.set_cursor(match cursor {
                    CursorIcon::Default => winit::window::CursorIcon::Default,
                    CursorIcon::Pointer => winit::window::CursorIcon::Pointer,
                    CursorIcon::Text => winit::window::CursorIcon::Text,
                    CursorIcon::ColResize => winit::window::CursorIcon::ColResize,
                    CursorIcon::RowResize => winit::window::CursorIcon::RowResize,
                    _ => winit::window::CursorIcon::Default,
                });
                state.cursor = cursor;
            }
            if redraw {
                state.window.request_redraw();
            }
        }
    }

    fn redraw(&mut self) {
        // Some systems still ask for frames of a hidden window. Coming back draws what changed.
        if self.in_tray.is_some() {
            return;
        }
        let lost = self.window.as_ref().is_some_and(|s| s.device_lost.load(Ordering::SeqCst));
        if lost && let Some(old) = self.window.take() {
            let window = old.window.clone();
            drop(old);
            match self.init_gpu(window) {
                Ok(state) => self.window = Some(state),
                Err(e) => tracing::error!(error = %e, "GPU recovery failed"),
            }
        }
        self.ui.set_time(self.started.elapsed().as_secs_f64());
        let Some(state) = &mut self.window else { return };
        let scale = self.ui.scale();
        // Looking costs a little, so it only happens while the panel is open.
        let looking = self.diagnostics.is_open();
        if looking != self.looking {
            self.looking = looking;
            let start = self.started;
            let clock: Option<Rc<dyn Fn() -> f64>> =
                looking.then(|| Rc::new(move || start.elapsed().as_secs_f64()) as Rc<dyn Fn() -> f64>);
            self.ui.set_phase_clock(clock);
            self.ui.set_view_profiling(looking);
            state.renderer.set_gpu_timing(looking);
        }
        let started = std::time::Instant::now();
        let scene = self.ui.update();
        let updated = started.elapsed();
        let frame = match state.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame) => frame,
            wgpu::CurrentSurfaceTexture::Suboptimal(frame) => {
                state.surface.configure(&state.gpu.device, &state.config);
                frame
            }
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                state.surface.configure(&state.gpu.device, &state.config);
                state.window.request_redraw();
                return;
            }
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => return,
            other => {
                tracing::error!(?other, "could not acquire a frame");
                return;
            }
        };
        let acquired = started.elapsed();
        let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());
        let stats = state.renderer.render(scene, &view, (state.config.width, state.config.height), scale);
        let rendered = started.elapsed();
        state.window.pre_present_notify();
        state.gpu.queue.present(frame);
        tracing::debug!(
            update = ?updated,
            acquire = ?acquired - updated,
            render = ?rendered - acquired,
            present = ?started.elapsed() - rendered,
            ?stats,
            "frame rendered"
        );
        let total = started.elapsed();
        if looking {
            self.diagnostics.set_view_costs(self.ui.view_costs());
        }
        self.diagnostics.record(FrameSample {
            work: self.ui.take_frame_work(),
            render_work: stats.work(),
            update: updated,
            acquire: acquired - updated,
            render: rendered - acquired,
            present: total - rendered,
            quads: stats.quads,
            glyphs: stats.glyphs,
            draw_calls: stats.draw_calls,
            size: (state.config.width, state.config.height),
            scale,
        });
        if self.ui.needs_redraw() {
            state.window.request_redraw();
        }
        self.update_access();
    }
}

/// Ctrl+Shift+D, or Cmd+Shift+D on a Mac, opens the diagnostics panel.
fn is_diagnostics_shortcut(key: &WinitKey, modifiers: Modifiers) -> bool {
    (modifiers.ctrl || modifiers.meta)
        && modifiers.shift
        && matches!(key, WinitKey::Character(c) if c.eq_ignore_ascii_case("d"))
}

fn map_key(key: &WinitKey) -> Key {
    match key {
        WinitKey::Named(NamedKey::Enter) => Key::Enter,
        WinitKey::Named(NamedKey::Space) => Key::Space,
        WinitKey::Named(NamedKey::Tab) => Key::Tab,
        WinitKey::Named(NamedKey::Escape) => Key::Escape,
        WinitKey::Named(NamedKey::Backspace) => Key::Backspace,
        WinitKey::Named(NamedKey::Delete) => Key::Delete,
        WinitKey::Named(NamedKey::Home) => Key::Home,
        WinitKey::Named(NamedKey::End) => Key::End,
        WinitKey::Named(NamedKey::PageUp) => Key::PageUp,
        WinitKey::Named(NamedKey::PageDown) => Key::PageDown,
        WinitKey::Named(NamedKey::ArrowLeft) => Key::ArrowLeft,
        WinitKey::Named(NamedKey::ArrowRight) => Key::ArrowRight,
        WinitKey::Named(NamedKey::ArrowUp) => Key::ArrowUp,
        WinitKey::Named(NamedKey::ArrowDown) => Key::ArrowDown,
        WinitKey::Character(c) if c.as_str() == " " => Key::Space,
        WinitKey::Character(c) => Key::Character(c.to_string()),
        other => Key::Other(format!("{other:?}")),
    }
}

fn map_button(button: MouseButton) -> PointerButton {
    match button {
        MouseButton::Left => PointerButton::Primary,
        MouseButton::Right => PointerButton::Secondary,
        MouseButton::Middle => PointerButton::Auxiliary,
        MouseButton::Back => PointerButton::Other(3),
        MouseButton::Forward => PointerButton::Other(4),
        MouseButton::Other(n) => PointerButton::Other(n),
    }
}

impl App {
    /// On Wayland, winit does not deliver a touchpad's pinch, so it is heard apart.
    fn on_wayland(&self) -> bool {
        use winit::raw_window_handle::{HasDisplayHandle, RawDisplayHandle};
        self.display
            .display_handle()
            .is_ok_and(|handle| matches!(handle.as_raw(), RawDisplayHandle::Wayland(_)))
    }

    fn listen_for_wayland_pinches(&self) {
        use winit::raw_window_handle::{HasDisplayHandle, RawDisplayHandle};
        let Ok(handle) = self.display.display_handle() else { return };
        let RawDisplayHandle::Wayland(wayland) = handle.as_raw() else { return };
        let proxy = self.proxy.clone();
        let heard = ravenrs_wayland_gestures::listen(wayland.display, move |delta| {
            let _ = proxy.send_event(UserEvent::Pinch(delta));
        });
        if let Err(e) = heard {
            tracing::info!(error = %e, "no touchpad pinches on this Wayland");
        }
    }
}

impl ApplicationHandler<UserEvent> for App {
    fn new_events(&mut self, _event_loop: &ActiveEventLoop, cause: StartCause) {
        if let StartCause::ResumeTimeReached { .. } = cause
            && let Some(state) = &self.window
        {
            state.window.request_redraw();
        }
    }

    /// Sleeps until input arrives or until the UI's next timed wake-up.
    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.in_tray.is_some()
            && let Some(active) = tray::app_is_active()
        {
            if !active {
                self.away_in_tray = true;
            } else if self.away_in_tray {
                self.open_window(event_loop);
            }
        }
        match self.ui.next_wake().filter(|_| self.in_tray.is_none()) {
            Some(at) => {
                let at = self.started + std::time::Duration::from_secs_f64(at.max(0.0));
                event_loop.set_control_flow(ControlFlow::WaitUntil(at));
            }
            None => event_loop.set_control_flow(ControlFlow::Wait),
        }
    }

    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        match self.create_window(event_loop) {
            Ok(state) => {
                self.show_window(state);
                self.listen_for_wayland_pinches();
            }
            Err(e) => {
                tracing::error!(error = %e, "could not start");
                self.error = Some(e);
                event_loop.exit();
            }
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: UserEvent) {
        use accesskit_winit::WindowEvent as A;
        let event = match event {
            UserEvent::Access(event) => event,
            UserEvent::Tray(tray::TrayAction::Open) => return self.open_window(event_loop),
            UserEvent::Tray(tray::TrayAction::Quit) => return event_loop.exit(),
            UserEvent::Pinch(delta) => {
                if let Some(position) = self.pointer {
                    self.dispatch(Event::Pinch { position, delta });
                }
                return;
            }
            UserEvent::Wake => {
                tracing::trace!("woken from another thread");
                if let Some(state) = &self.window {
                    state.window.request_redraw();
                }
                return;
            }
        };
        match event.window_event {
            A::InitialTreeRequested => {
                if let Some(access) = &mut self.access {
                    access.active = true;
                    access.last = None;
                }
                self.ui.update();
                self.update_access();
            }
            A::ActionRequested(request) => {
                let Some(action) = access::action(request.action) else { return };
                self.ui.set_time(self.started.elapsed().as_secs_f64());
                if self.ui.perform_action(request.target_node.0, action)
                    && let Some(state) = &self.window
                {
                    state.window.request_redraw();
                }
            }
            A::AccessibilityDeactivated => {
                if let Some(access) = &mut self.access {
                    access.active = false;
                    access.last = None;
                }
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        if let (Some(access), Some(state)) = (&mut self.access, &self.window) {
            access.adapter.process_event(&state.window, &event);
        }
        match event {
            WindowEvent::CloseRequested => {
                if !self.close_to_tray(event_loop) {
                    event_loop.exit();
                }
            }
            WindowEvent::Resized(size) => {
                if let Some(state) = &mut self.window {
                    state.config.width = size.width.max(1);
                    state.config.height = size.height.max(1);
                    state.surface.configure(&state.gpu.device, &state.config);
                    let window = state.window.clone();
                    self.update_viewport(&window, size);
                    window.request_redraw();
                }
            }
            WindowEvent::ScaleFactorChanged { .. } => {
                if let Some(state) = &self.window {
                    let window = state.window.clone();
                    self.update_viewport(&window, window.inner_size());
                    window.request_redraw();
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                let scale = self.ui.scale().max(f32::EPSILON) as f64;
                let PhysicalPosition { x, y } = position;
                let p = Point::new((x / scale) as f32, (y / scale) as f32);
                self.pointer = Some(p);
                self.dispatch(Event::PointerMove { position: p });
            }
            WindowEvent::HoveredFile(_) => self.ui.set_files_over(true),
            WindowEvent::HoveredFileCancelled => self.ui.set_files_over(false),
            WindowEvent::DroppedFile(path) => {
                // Nothing here knows how large a place will take; this keeps a drop
                // from filling the memory with something no place would keep.
                if let Some(file) = files::read_picked(&path, Some(files::MOST_DROPPED_BYTES)) {
                    self.ui.drop_files(self.pointer, vec![file]);
                }
            }
            WindowEvent::CursorLeft { .. } => {
                self.pointer = None;
                self.dispatch(Event::PointerLeave);
            }
            WindowEvent::MouseInput { state, button, .. } => {
                if let Some(position) = self.pointer {
                    let button = map_button(button);
                    let event = match state {
                        ElementState::Pressed => Event::PointerDown { position, button },
                        ElementState::Released => Event::PointerUp { position, button },
                    };
                    self.dispatch(event);
                    if state == ElementState::Released && button == PointerButton::Primary {
                        let now = std::time::Instant::now();
                        let double = self.last_click.is_some_and(|(t, p)| {
                            now.duration_since(t).as_millis() < 450
                                && (p.x - position.x).hypot(p.y - position.y) < 5.0
                        });
                        if double {
                            self.last_click = None;
                            self.dispatch(Event::DoubleClick { position });
                        } else {
                            self.last_click = Some((now, position));
                        }
                    }
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                if let Some(position) = self.pointer {
                    let delta = match delta {
                        // One notch scrolls three 16px lines, as browsers do.
                        MouseScrollDelta::LineDelta(x, y) => Point::new(x * 48.0, y * 48.0),
                        MouseScrollDelta::PixelDelta(p) => {
                            let scale = self.ui.scale().max(f32::EPSILON) as f64;
                            Point::new((p.x / scale) as f32, (p.y / scale) as f32)
                        }
                    };
                    // Shift turns a wheel that only goes up and down sideways, as browsers do.
                    let delta =
                        if self.modifiers.shift && delta.x == 0.0 { Point::new(delta.y, 0.0) } else { delta };
                    if self.modifiers.ctrl {
                        // How touchpads on Windows send a pinch, and how a mouse asks to zoom.
                        self.dispatch(Event::Pinch { position, delta: delta.y / WHEEL_PER_ZOOM });
                    } else {
                        self.dispatch(Event::Scroll { position, delta });
                    }
                }
            }
            WindowEvent::PinchGesture { delta, .. } => {
                if let Some(position) = self.pointer {
                    self.dispatch(Event::Pinch { position, delta: delta as f32 });
                }
            }
            WindowEvent::ModifiersChanged(m) => {
                let s: ModifiersState = m.state();
                self.modifiers = Modifiers {
                    shift: s.shift_key(),
                    ctrl: s.control_key(),
                    alt: s.alt_key(),
                    meta: s.super_key(),
                };
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if is_diagnostics_shortcut(&event.logical_key, self.modifiers) {
                    if event.state == ElementState::Pressed && !event.repeat {
                        self.diagnostics.toggle();
                    }
                    return;
                }
                let key = map_key(&event.logical_key);
                let modifiers = self.modifiers;
                self.dispatch(match event.state {
                    ElementState::Pressed => Event::KeyDown { key, modifiers },
                    ElementState::Released => Event::KeyUp { key, modifiers },
                });
            }
            WindowEvent::Focused(false) => self.dispatch(Event::FocusLost),
            WindowEvent::RedrawRequested => self.redraw(),
            _ => {}
        }
    }
}

/// The window full screen applies to. Weak, so a window closed behind the icon still goes.
type ScreenWindow = std::rc::Rc<std::cell::RefCell<std::sync::Weak<Window>>>;

/// Full screen as a borderless window over the whole monitor it is on.
struct SystemFullScreen(ScreenWindow);

impl ravenrs_widgets::FullScreenProvider for SystemFullScreen {
    fn is_on(&self) -> bool {
        self.0.borrow().upgrade().is_some_and(|window| window.fullscreen().is_some())
    }

    fn set(&self, on: bool) {
        if let Some(window) = self.0.borrow().upgrade() {
            window.set_fullscreen(on.then_some(winit::window::Fullscreen::Borderless(None)));
        }
    }
}

/// Opens web addresses in the system's browser. Other addresses (relative paths, other
/// schemes) are refused there, so they are only logged.
struct SystemLinks;

impl ravenrs_widgets::LinkOpener for SystemLinks {
    fn open(&self, url: &str) {
        if let Err(e) = files::open_url(url) {
            tracing::warn!(url, error = %e, "the address did not open");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_diagnostics_shortcut_needs_both_modifiers() {
        let key = WinitKey::Character("D".into());
        let held = |ctrl, shift, meta| Modifiers { ctrl, shift, meta, alt: false };
        assert!(is_diagnostics_shortcut(&key, held(true, true, false)));
        assert!(is_diagnostics_shortcut(&key, held(false, true, true)));
        assert!(!is_diagnostics_shortcut(&key, held(true, false, false)));
        assert!(!is_diagnostics_shortcut(&key, held(false, true, false)));
        assert!(!is_diagnostics_shortcut(&WinitKey::Character("e".into()), held(true, true, false)));
    }

    #[test]
    fn named_keys_map_to_raven_keys() {
        let cases = [
            (NamedKey::Enter, Key::Enter),
            (NamedKey::Space, Key::Space),
            (NamedKey::Tab, Key::Tab),
            (NamedKey::Escape, Key::Escape),
            (NamedKey::Backspace, Key::Backspace),
            (NamedKey::Delete, Key::Delete),
            (NamedKey::Home, Key::Home),
            (NamedKey::End, Key::End),
            (NamedKey::PageUp, Key::PageUp),
            (NamedKey::PageDown, Key::PageDown),
            (NamedKey::ArrowLeft, Key::ArrowLeft),
            (NamedKey::ArrowRight, Key::ArrowRight),
            (NamedKey::ArrowUp, Key::ArrowUp),
            (NamedKey::ArrowDown, Key::ArrowDown),
        ];
        for (named, expected) in cases {
            assert_eq!(map_key(&WinitKey::Named(named)), expected, "{named:?}");
        }
    }

    #[test]
    fn characters_and_unknown_keys() {
        assert_eq!(map_key(&WinitKey::Character(" ".into())), Key::Space, "a typed space is the Space key");
        assert_eq!(map_key(&WinitKey::Character("ç".into())), Key::Character("ç".into()));
        assert!(matches!(map_key(&WinitKey::Named(NamedKey::F5)), Key::Other(name) if name.contains("F5")));
    }

    #[test]
    fn mouse_buttons_map_to_pointer_buttons() {
        assert_eq!(map_button(MouseButton::Left), PointerButton::Primary);
        assert_eq!(map_button(MouseButton::Right), PointerButton::Secondary);
        assert_eq!(map_button(MouseButton::Middle), PointerButton::Auxiliary);
        assert_eq!(map_button(MouseButton::Back), PointerButton::Other(3));
        assert_eq!(map_button(MouseButton::Forward), PointerButton::Other(4));
        assert_eq!(map_button(MouseButton::Other(9)), PointerButton::Other(9));
    }
}
