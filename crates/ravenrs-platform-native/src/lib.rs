//! Desktop backend for RavenRS: one winit window per app, rendered with wgpu
//! (Metal on macOS, DX12/Vulkan on Windows, Vulkan/GL on Linux).
//!
//! Frames are produced on demand: the event loop sleeps until input arrives or
//! reactive state changes, so an idle app uses no CPU or GPU time.
//!
//! Screen readers see the UI through AccessKit: the accessibility tree is
//! built only while one is running.

mod access;
mod files;

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

use ravenrs_core::{AccessTree, CursorIcon, Event, ImageData, Key, Modifiers, Point, PointerButton, Size};
use ravenrs_render::{GpuContext, GpuError, Renderer, preferred_surface_format, wgpu};
use ravenrs_style::Theme;
use ravenrs_widgets::{Ui, View};
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalSize, PhysicalPosition, PhysicalSize};
use winit::event::StartCause;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};
use winit::keyboard::{Key as WinitKey, ModifiersState, NamedKey};
use winit::window::{Icon, Window, WindowAttributes, WindowId};

pub use files::{SaveDialog, open_with_system};

/// Wheel pixels that ask for twice the size, with Ctrl held.
const WHEEL_PER_ZOOM: f32 = 400.0;

/// What reaches the event loop from outside it.
#[derive(Debug)]
enum UserEvent {
    /// From the screen reader.
    Access(accesskit_winit::Event),
    /// A [`UiWaker`](ravenrs_widgets::UiWaker) fired, on some other thread.
    Wake,
}

impl From<accesskit_winit::Event> for UserEvent {
    fn from(event: accesskit_winit::Event) -> Self {
        Self::Access(event)
    }
}

/// How the main window opens.
#[derive(Clone, Debug)]
pub struct WindowOptions {
    /// Window title.
    pub title: String,
    /// Initial inner size in logical pixels.
    pub size: Size,
    /// Smallest inner size the user can resize to, in logical pixels.
    pub min_size: Option<Size>,
    /// Theme the UI starts with.
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

/// Failure to start or run the desktop app.
#[derive(Debug)]
pub enum PlatformError {
    /// The event loop could not be created or exited with an error.
    EventLoop(String),
    /// The window could not be created.
    Window(String),
    /// GPU setup or surface configuration failed.
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

/// Opens a window showing `root` and runs until it is closed.
pub fn run(root: impl View + 'static, options: WindowOptions) -> Result<(), PlatformError> {
    run_window(root, options, None, None)
}

/// Who the app is to the system, for finding its icon and its menu entry.
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
}

impl AppIdentity {
    /// The app known as `id`, which is the name of its `.desktop` file on Linux.
    pub fn new(id: impl Into<String>) -> Self {
        Self { id: id.into(), icon: None }
    }

    /// An icon for the title bar and the task bar on Windows and on Linux with
    /// X11. macOS takes an app's icon from its bundle instead.
    pub fn icon(mut self, icon: ImageData) -> Self {
        self.icon = Some(icon);
        self
    }
}

/// Like [`run`], for an app the system should recognize as itself.
pub fn run_as(
    root: impl View + 'static,
    options: WindowOptions,
    identity: AppIdentity,
) -> Result<(), PlatformError> {
    run_window(root, options, identity.icon, Some(identity.id))
}

/// Like [`run`], with an icon for the window's title bar and the task bar on
/// Windows and Linux. macOS takes an app's icon from its bundle instead.
pub fn run_with_icon(
    root: impl View + 'static,
    options: WindowOptions,
    icon: ImageData,
) -> Result<(), PlatformError> {
    run_window(root, options, Some(icon), None)
}

fn run_window(
    root: impl View + 'static,
    options: WindowOptions,
    icon: Option<ImageData>,
    app_id: Option<String>,
) -> Result<(), PlatformError> {
    let event_loop = EventLoop::<UserEvent>::with_user_event()
        .build()
        .map_err(|e| PlatformError::EventLoop(e.to_string()))?;
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut ui = Ui::new(root, options.theme);
    let wake = event_loop.create_proxy();
    ui.set_remote_waker(move || {
        // It only fails once the loop is gone, when there is nothing left to wake.
        let _ = wake.send_event(UserEvent::Wake);
    });
    match arboard::Clipboard::new() {
        Ok(clipboard) => ui.set_clipboard(SystemClipboard(std::cell::RefCell::new(clipboard))),
        Err(e) => tracing::warn!(error = %e, "system clipboard unavailable; using an in-app clipboard"),
    }
    let mut app = App {
        proxy: event_loop.create_proxy(),
        access: None,
        ui,
        options,
        icon,
        app_id,
        window: None,
        pointer: None,
        modifiers: Modifiers::default(),
        last_click: None,
        started: std::time::Instant::now(),
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
    ui: Ui,
    proxy: EventLoopProxy<UserEvent>,
    access: Option<Access>,
    options: WindowOptions,
    icon: Option<ImageData>,
    app_id: Option<String>,
    window: Option<WindowState>,
    /// Last pointer position in logical pixels.
    pointer: Option<Point>,
    modifiers: Modifiers,
    /// Time and place of the last primary release, to detect double clicks.
    last_click: Option<(std::time::Instant, Point)>,
    started: std::time::Instant,
    error: Option<PlatformError>,
}

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
        // Hidden until the accessibility adapter is attached, as AccessKit requires.
        let mut attributes = Window::default_attributes()
            .with_visible(false)
            .with_title(self.options.title.clone())
            .with_inner_size(LogicalSize::new(self.options.size.width, self.options.size.height));
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
        window.set_visible(true);
        self.init_gpu(window)
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
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
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
        let scene = self.ui.update();
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
        let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());
        let stats = state.renderer.render(scene, &view, (state.config.width, state.config.height), scale);
        tracing::trace!(?stats, "frame rendered");
        state.window.pre_present_notify();
        state.gpu.queue.present(frame);
        if self.ui.needs_redraw() {
            state.window.request_redraw();
        }
        self.update_access();
    }
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
        match self.ui.next_wake() {
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
            Ok(state) => self.window = Some(state),
            Err(e) => {
                tracing::error!(error = %e, "could not start");
                self.error = Some(e);
                event_loop.exit();
            }
        }
    }

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: UserEvent) {
        use accesskit_winit::WindowEvent as A;
        let event = match event {
            UserEvent::Access(event) => event,
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
            WindowEvent::CloseRequested => event_loop.exit(),
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
                    if self.modifiers.ctrl {
                        // How touchpads on Windows and Linux send a pinch, and how a mouse asks to zoom.
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

#[cfg(test)]
mod tests {
    use super::*;

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
