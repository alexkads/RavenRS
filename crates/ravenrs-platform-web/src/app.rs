use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use ravenrs_core::{CursorIcon, Event, Key, Modifiers, Point, PointerButton, Size};
use ravenrs_render::{GpuContext, GpuError, Renderer, preferred_surface_format, wgpu};
use ravenrs_style::Theme;
use ravenrs_widgets::{Ui, View};
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::Closure;
use web_sys::{
    CompositionEvent, Document, EventTarget, HtmlCanvasElement, HtmlTextAreaElement, InputEvent,
    KeyboardEvent, PointerEvent, ResizeObserver, WheelEvent,
};

use crate::WebError;

/// Where and how the app mounts.
#[derive(Clone, Debug)]
pub struct WebOptions {
    /// Id of an existing `<canvas>` element. It should be sized with CSS.
    pub canvas_id: String,
    /// The theme the app starts with.
    pub theme: Theme,
}

impl WebOptions {
    /// Mounts on the `<canvas>` with this id, in the default theme.
    pub fn new(canvas_id: impl Into<String>) -> Self {
        Self { canvas_id: canvas_id.into(), theme: Theme::default() }
    }

    /// Starts in `theme` instead of Raven Dark.
    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }
}

struct Gpu {
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    context: GpuContext,
    renderer: Renderer,
    lost: Arc<AtomicBool>,
}

struct State {
    ui: Ui,
    canvas: HtmlCanvasElement,
    gpu: Option<Gpu>,
    cursor: CursorIcon,
    recovering: bool,
}

/// Schedules at most one `requestAnimationFrame` at a time. Kept apart from
/// `State` so it can be called while `State` is borrowed (e.g. from a click
/// handler that changes state).
type FrameCallback = RefCell<Option<Closure<dyn FnMut(f64)>>>;

struct Scheduler {
    pending: Cell<bool>,
    /// The pending `setTimeout` for a timed wake-up, and its target time.
    timer: Cell<Option<(i32, f64)>>,
    timer_callback: RefCell<Option<Closure<dyn FnMut()>>>,
    callback: FrameCallback,
}

impl Scheduler {
    /// Schedules a frame at `at` seconds (the `Ui` clock), replacing a later timer.
    fn schedule_at(&self, at: f64) {
        let Some(window) = web_sys::window() else { return };
        if let Some((_, existing)) = self.timer.get()
            && existing <= at
        {
            return;
        }
        if let Some((id, _)) = self.timer.take() {
            window.clear_timeout_with_handle(id);
        }
        let delay = ((at - now_seconds()) * 1000.0).max(0.0).ceil() as i32;
        let cb = self.timer_callback.borrow();
        if let Some(cb) = cb.as_ref()
            && let Ok(id) = window
                .set_timeout_with_callback_and_timeout_and_arguments_0(cb.as_ref().unchecked_ref(), delay)
        {
            self.timer.set(Some((id, at)));
        }
    }

    fn schedule(&self) {
        if self.pending.replace(true) {
            return;
        }
        let requested = web_sys::window().and_then(|w| {
            let cb = self.callback.borrow();
            cb.as_ref().and_then(|cb| w.request_animation_frame(cb.as_ref().unchecked_ref()).ok())
        });
        if requested.is_none() {
            self.pending.set(false);
        }
    }
}

struct Listener {
    target: EventTarget,
    kind: &'static str,
    closure: Closure<dyn FnMut(web_sys::Event)>,
}

/// A running app. Dropping it (or calling [`WebApp::destroy`]) removes every
/// event listener and observer and releases the GPU resources.
pub struct WebApp {
    state: Rc<RefCell<State>>,
    scheduler: Rc<Scheduler>,
    listeners: Vec<Listener>,
    resize_observer: Option<ResizeObserver>,
    _resize_callback: Option<Closure<dyn FnMut(js_sys::Array)>>,
    text_input: Option<HtmlTextAreaElement>,
}

impl std::fmt::Debug for WebApp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WebApp").finish_non_exhaustive()
    }
}

impl WebApp {
    /// Unmounts the app now; the same as dropping it.
    pub fn destroy(self) {}
}

impl Drop for WebApp {
    fn drop(&mut self) {
        for l in self.listeners.drain(..) {
            let _ = l.target.remove_event_listener_with_callback(l.kind, l.closure.as_ref().unchecked_ref());
        }
        if let Some(observer) = self.resize_observer.take() {
            observer.disconnect();
        }
        if let Some(input) = self.text_input.take() {
            input.remove();
        }
        if let (Some(window), Some((id, _))) = (web_sys::window(), self.scheduler.timer.take()) {
            window.clear_timeout_with_handle(id);
        }
        self.scheduler.callback.borrow_mut().take();
        self.scheduler.timer_callback.borrow_mut().take();
        if let Ok(mut state) = self.state.try_borrow_mut() {
            state.gpu = None;
        }
    }
}

fn document() -> Result<Document, WebError> {
    web_sys::window().and_then(|w| w.document()).ok_or_else(|| WebError::Js("no window.document".into()))
}

/// Shows `message` in a small panel right after the canvas.
fn show_error(canvas: &HtmlCanvasElement, message: &str) {
    let Ok(doc) = document() else { return };
    let Ok(panel) = doc.create_element("div") else { return };
    panel.set_class_name("raven-error");
    panel.set_attribute("role", "alert").ok();
    panel.set_text_content(Some(message));
    if let Some(parent) = canvas.parent_node() {
        parent.insert_before(&panel, canvas.next_sibling().as_ref()).ok();
    }
}

/// Distance a touch must travel before it scrolls instead of tapping.
const TOUCH_SLOP: f32 = 8.0;

#[derive(Clone, Copy)]
struct TouchScroll {
    start: Point,
    last: Point,
    scrolling: bool,
}

fn is_dragging(state: &Rc<RefCell<State>>) -> bool {
    state.try_borrow().is_ok_and(|s| s.ui.is_dragging())
}

/// An invisible textarea that receives keyboard focus and text for the canvas.
fn create_text_input(canvas: &HtmlCanvasElement) -> Result<HtmlTextAreaElement, WebError> {
    let doc = document()?;
    let input: HtmlTextAreaElement =
        doc.create_element("textarea")?.dyn_into().map_err(|_| WebError::Js("textarea".into()))?;
    input.set_attribute("aria-label", "Text input")?;
    input.set_attribute("autocapitalize", "off")?;
    input.set_attribute("autocomplete", "off")?;
    input.set_attribute("spellcheck", "false")?;
    let style = input.style();
    for (k, v) in [
        ("position", "fixed"),
        ("left", "0"),
        ("top", "0"),
        ("width", "1px"),
        ("height", "1px"),
        ("opacity", "0"),
        ("padding", "0"),
        ("border", "0"),
        ("resize", "none"),
        ("pointer-events", "none"),
        // Font size 16px avoids iOS zooming into the page on focus.
        ("font-size", "16px"),
    ] {
        style.set_property(k, v)?;
    }
    if let Some(parent) = canvas.parent_node() {
        parent.insert_before(&input, canvas.next_sibling().as_ref())?;
    }
    Ok(input)
}

/// Delivers committed text: a lone space as the Space key (so it also activates
/// buttons), anything else as characters.
fn send_text(state: &Rc<RefCell<State>>, scheduler: &Scheduler, text: &str) {
    let modifiers = Modifiers::default();
    for (i, part) in text.split(' ').enumerate() {
        if i > 0 {
            dispatch(state, scheduler, Event::KeyDown { key: Key::Space, modifiers });
        }
        if !part.is_empty() {
            dispatch(state, scheduler, Event::KeyDown { key: Key::Character(part.to_owned()), modifiers });
        }
    }
}

/// Seconds since the page loaded.
fn now_seconds() -> f64 {
    web_sys::window().and_then(|w| w.performance()).map_or(0.0, |p| p.now() / 1000.0)
}

fn webgpu_available() -> bool {
    web_sys::window()
        .map(|w| w.navigator())
        .and_then(|n| js_sys::Reflect::get(&n, &"gpu".into()).ok())
        .is_some_and(|gpu| !gpu.is_undefined() && !gpu.is_null())
}

/// The canvas size in CSS pixels and the device pixel ratio.
fn measure(canvas: &HtmlCanvasElement) -> (Size, f32) {
    let dpr = web_sys::window().map_or(1.0, |w| w.device_pixel_ratio()) as f32;
    let css = Size::new(canvas.client_width().max(1) as f32, canvas.client_height().max(1) as f32);
    (css, dpr.max(0.25))
}

fn physical(css: Size, dpr: f32, limit: u32) -> (u32, u32) {
    let w = ((css.width * dpr).round() as u32).clamp(1, limit);
    let h = ((css.height * dpr).round() as u32).clamp(1, limit);
    (w, h)
}

async fn create_gpu(canvas: &HtmlCanvasElement, scheduler: &Rc<Scheduler>) -> Result<Gpu, WebError> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::BROWSER_WEBGPU,
        ..wgpu::InstanceDescriptor::new_without_display_handle()
    });
    let surface = instance
        .create_surface(wgpu::SurfaceTarget::Canvas(canvas.clone()))
        .map_err(|e| GpuError::Surface(e.to_string()))?;
    let context = GpuContext::new(&instance, Some(&surface)).await?;
    let caps = surface.get_capabilities(&context.adapter);
    let format = preferred_surface_format(&caps.formats)
        .ok_or_else(|| GpuError::Surface("the canvas reports no supported formats".into()))?;
    let (css, dpr) = measure(canvas);
    let (width, height) = physical(css, dpr, context.device.limits().max_texture_dimension_2d);
    canvas.set_width(width);
    canvas.set_height(height);
    let config = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format,
        width,
        height,
        present_mode: wgpu::PresentMode::Fifo,
        desired_maximum_frame_latency: 2,
        alpha_mode: caps.alpha_modes.first().copied().unwrap_or(wgpu::CompositeAlphaMode::Auto),
        view_formats: vec![],
        color_space: wgpu::SurfaceColorSpace::Auto,
    };
    surface.configure(&context.device, &config);
    let lost = Arc::new(AtomicBool::new(false));
    let flag = lost.clone();
    context.device.set_device_lost_callback(move |reason, message| {
        tracing::error!(?reason, message, "GPU device lost");
        flag.store(true, Ordering::SeqCst);
    });
    let renderer = Renderer::new(&context.device, &context.queue, format);
    scheduler.schedule();
    Ok(Gpu { surface, config, context, renderer, lost })
}

/// Mounts `root` on the canvas and starts rendering.
///
/// On failure a readable message is also shown on the page, next to the canvas.
pub async fn start(root: impl View + 'static, options: WebOptions) -> Result<WebApp, WebError> {
    let doc = document()?;
    let canvas = doc
        .get_element_by_id(&options.canvas_id)
        .and_then(|e| e.dyn_into::<HtmlCanvasElement>().ok())
        .ok_or_else(|| WebError::CanvasNotFound(options.canvas_id.clone()))?;

    if !webgpu_available() {
        let e = WebError::WebGpuUnavailable;
        show_error(&canvas, &e.to_string());
        return Err(e);
    }

    let scheduler = Rc::new(Scheduler {
        pending: Cell::new(false),
        callback: RefCell::new(None),
        timer: Cell::new(None),
        timer_callback: RefCell::new(None),
    });
    {
        let sched = Rc::downgrade(&scheduler);
        *scheduler.timer_callback.borrow_mut() = Some(Closure::new(move || {
            if let Some(sched) = sched.upgrade() {
                sched.timer.set(None);
                sched.schedule();
            }
        }));
    }
    let gpu = match create_gpu(&canvas, &scheduler).await {
        Ok(gpu) => gpu,
        Err(e) => {
            show_error(&canvas, &e.to_string());
            return Err(e);
        }
    };

    let mut ui = Ui::new(root, options.theme);
    let (css, dpr) = measure(&canvas);
    ui.set_viewport(css, dpr);
    let waker = scheduler.clone();
    ui.set_waker(move || waker.schedule());

    // Focusable, so keyboard input reaches the app; no outline, RavenRS draws focus itself.
    canvas.set_tab_index(0);
    canvas.style().set_property("outline", "none").ok();
    canvas.style().set_property("touch-action", "none").ok();

    let state = Rc::new(RefCell::new(State {
        ui,
        canvas: canvas.clone(),
        gpu: Some(gpu),
        cursor: CursorIcon::Default,
        recovering: false,
    }));

    // The frame callback.
    {
        let (state, sched) = (state.clone(), Rc::downgrade(&scheduler));
        *scheduler.callback.borrow_mut() = Some(Closure::new(move |_time: f64| {
            let Some(sched) = sched.upgrade() else { return };
            sched.pending.set(false);
            frame(&state, &sched);
        }));
    }

    // Keyboard input goes through a hidden <textarea>: unlike a canvas it receives
    // `input` and composition events, so IMEs, dead keys, emoji pickers, mobile
    // keyboards and paste all deliver text.
    let text_input = create_text_input(&canvas)?;
    let mut app = WebApp {
        state: state.clone(),
        scheduler: scheduler.clone(),
        listeners: Vec::new(),
        resize_observer: None,
        _resize_callback: None,
        text_input: Some(text_input.clone()),
    };

    let target: EventTarget = canvas.clone().into();
    // A touch that moves more than a few pixels becomes a scroll gesture, unless
    // it started on something draggable (a slider).
    let touch: Rc<Cell<Option<TouchScroll>>> = Rc::new(Cell::new(None));
    let pointer = |kind: &'static str, make: fn(Point, PointerButton) -> Event| {
        let (state, sched) = (state.clone(), scheduler.clone());
        let canvas = canvas.clone();
        let focus_text_input = text_input.clone();
        let touch = touch.clone();
        Listener {
            target: target.clone(),
            kind,
            closure: Closure::new(move |e: web_sys::Event| {
                let Some(e) = e.dyn_ref::<PointerEvent>() else { return };
                let p = Point::new(e.offset_x() as f32, e.offset_y() as f32);
                let button = match e.button() {
                    0 => PointerButton::Primary,
                    1 => PointerButton::Auxiliary,
                    2 => PointerButton::Secondary,
                    n => PointerButton::Other(n.max(0) as u16),
                };
                let is_touch = e.pointer_type() == "touch";
                match kind {
                    "pointerdown" => {
                        canvas.set_pointer_capture(e.pointer_id()).ok();
                        // Focus after the browser's default focus handling for this click.
                        e.prevent_default();
                        focus_text_input.focus().ok();
                        if is_touch {
                            touch.set(Some(TouchScroll { start: p, last: p, scrolling: false }));
                        }
                    }
                    "pointermove" if is_touch => {
                        if let Some(mut t) = touch.get()
                            && !is_dragging(&state)
                        {
                            let moved = (p.x - t.start.x).hypot(p.y - t.start.y);
                            if t.scrolling || moved > TOUCH_SLOP {
                                if !t.scrolling {
                                    // Cancels the press, so lifting the finger does not click.
                                    dispatch(&state, &sched, Event::PointerLeave);
                                    t.scrolling = true;
                                }
                                let delta = Point::new(p.x - t.last.x, p.y - t.last.y);
                                dispatch(&state, &sched, Event::Scroll { position: t.start, delta });
                                t.last = p;
                                touch.set(Some(t));
                                return;
                            }
                        }
                    }
                    "pointerup" | "pointercancel" => {
                        canvas.release_pointer_capture(e.pointer_id()).ok();
                        let was_scrolling = touch.take().is_some_and(|t| t.scrolling);
                        if was_scrolling {
                            return;
                        }
                    }
                    _ => {}
                }
                dispatch(&state, &sched, make(p, button));
            }),
        }
    };
    app.listeners.push(pointer("pointermove", |position, _| Event::PointerMove { position }));
    app.listeners.push(pointer("pointerdown", |position, button| Event::PointerDown { position, button }));
    app.listeners.push(pointer("pointerup", |position, button| Event::PointerUp { position, button }));
    app.listeners.push(pointer("pointerleave", |_, _| Event::PointerLeave));
    app.listeners.push(pointer("pointercancel", |_, _| Event::PointerLeave));
    // The app draws its own context menus.
    app.listeners.push(Listener {
        target: target.clone(),
        kind: "contextmenu",
        closure: Closure::new(move |e: web_sys::Event| e.prevent_default()),
    });
    {
        let (state, sched) = (state.clone(), scheduler.clone());
        app.listeners.push(Listener {
            target: target.clone(),
            kind: "dblclick",
            closure: Closure::new(move |e: web_sys::Event| {
                if let Some(e) = e.dyn_ref::<web_sys::MouseEvent>() {
                    let position = Point::new(e.offset_x() as f32, e.offset_y() as f32);
                    dispatch(&state, &sched, Event::DoubleClick { position });
                }
            }),
        });
    }

    {
        let (state, sched) = (state.clone(), scheduler.clone());
        app.listeners.push(Listener {
            target: target.clone(),
            kind: "wheel",
            closure: Closure::new(move |e: web_sys::Event| {
                let Some(e) = e.dyn_ref::<WheelEvent>() else { return };
                let unit = match e.delta_mode() {
                    WheelEvent::DOM_DELTA_LINE => 16.0,
                    WheelEvent::DOM_DELTA_PAGE => 600.0,
                    _ => 1.0,
                };
                let position = Point::new(e.offset_x() as f32, e.offset_y() as f32);
                let delta = Point::new((-e.delta_x() * unit) as f32, (-e.delta_y() * unit) as f32);
                // Only keep the page from scrolling when the app scrolled something.
                if dispatch(&state, &sched, Event::Scroll { position, delta }) {
                    e.prevent_default();
                }
            }),
        });
    }
    let input_target: EventTarget = text_input.clone().into();
    let composing = Rc::new(Cell::new(false));
    for kind in ["keydown", "keyup"] {
        let (state, sched, composing) = (state.clone(), scheduler.clone(), composing.clone());
        app.listeners.push(Listener {
            target: input_target.clone(),
            kind,
            closure: Closure::new(move |e: web_sys::Event| {
                let Some(e) = e.dyn_ref::<KeyboardEvent>() else { return };
                if composing.get() || e.is_composing() {
                    return;
                }
                let key = map_key(&e.key());
                let modifiers = Modifiers {
                    shift: e.shift_key(),
                    ctrl: e.ctrl_key(),
                    alt: e.alt_key(),
                    meta: e.meta_key(),
                };
                // Printable text arrives through the `input` event instead, so it is
                // not inserted twice; shortcuts with Ctrl/Cmd still go through here.
                let printable =
                    matches!(key, Key::Character(_) | Key::Space) && !(modifiers.ctrl || modifiers.meta);
                if printable {
                    return;
                }
                // Keep Tab, Enter, arrows and Backspace inside the app.
                e.prevent_default();
                let event = if kind == "keydown" {
                    Event::KeyDown { key, modifiers }
                } else {
                    Event::KeyUp { key, modifiers }
                };
                dispatch(&state, &sched, event);
            }),
        });
    }
    {
        let (state, sched, composing, input) =
            (state.clone(), scheduler.clone(), composing.clone(), text_input.clone());
        app.listeners.push(Listener {
            target: input_target.clone(),
            kind: "input",
            closure: Closure::new(move |e: web_sys::Event| {
                if composing.get() || e.dyn_ref::<InputEvent>().is_some_and(InputEvent::is_composing) {
                    return;
                }
                let text = input.value();
                input.set_value("");
                send_text(&state, &sched, &text);
            }),
        });
    }
    {
        let composing = composing.clone();
        app.listeners.push(Listener {
            target: input_target.clone(),
            kind: "compositionstart",
            closure: Closure::new(move |_e: web_sys::Event| composing.set(true)),
        });
    }
    {
        let (state, sched, composing, input) =
            (state.clone(), scheduler.clone(), composing.clone(), text_input.clone());
        app.listeners.push(Listener {
            target: input_target.clone(),
            kind: "compositionend",
            closure: Closure::new(move |e: web_sys::Event| {
                composing.set(false);
                let text =
                    e.dyn_ref::<CompositionEvent>().and_then(CompositionEvent::data).unwrap_or_default();
                input.set_value("");
                send_text(&state, &sched, &text);
            }),
        });
    }

    {
        let (state, sched) = (state.clone(), scheduler.clone());
        app.listeners.push(Listener {
            target: input_target.clone(),
            kind: "blur",
            closure: Closure::new(move |_e: web_sys::Event| {
                dispatch(&state, &sched, Event::FocusLost);
            }),
        });
    }
    for l in &app.listeners {
        l.target.add_event_listener_with_callback(l.kind, l.closure.as_ref().unchecked_ref())?;
    }

    {
        let input = text_input.clone();
        app.listeners.push(Listener {
            target: target.clone(),
            kind: "focus",
            closure: Closure::new(move |_e: web_sys::Event| {
                input.focus().ok();
            }),
        });
    }
    // Size changes (CSS layout, window resize, zoom) schedule a frame; the frame
    // reads the new size and device pixel ratio.
    let sched = scheduler.clone();
    let resize_callback =
        Closure::<dyn FnMut(js_sys::Array)>::new(move |_entries: js_sys::Array| sched.schedule());
    let observer = ResizeObserver::new(resize_callback.as_ref().unchecked_ref())?;
    observer.observe(&canvas);
    app.resize_observer = Some(observer);
    app._resize_callback = Some(resize_callback);

    scheduler.schedule();
    tracing::info!("RavenRS web app started");
    Ok(app)
}

/// Feeds an event to the UI; returns true when it asked for a redraw.
fn dispatch(state: &Rc<RefCell<State>>, scheduler: &Scheduler, event: Event) -> bool {
    let Ok(mut s) = state.try_borrow_mut() else { return false };
    s.ui.set_time(now_seconds());
    let redraw = s.ui.handle_event(&event);
    let cursor = s.ui.cursor();
    if cursor != s.cursor {
        let css = match cursor {
            CursorIcon::Default => "default",
            CursorIcon::Pointer => "pointer",
            CursorIcon::Text => "text",
            CursorIcon::ColResize => "col-resize",
            CursorIcon::RowResize => "row-resize",
            _ => "default",
        };
        s.canvas.style().set_property("cursor", css).ok();
        s.cursor = cursor;
    }
    drop(s);
    if redraw {
        scheduler.schedule();
    }
    redraw
}

fn frame(state: &Rc<RefCell<State>>, scheduler: &Rc<Scheduler>) {
    let Ok(mut s) = state.try_borrow_mut() else { return };
    let s = &mut *s;

    if s.gpu.as_ref().is_some_and(|g| g.lost.load(Ordering::SeqCst)) {
        s.gpu = None;
    }
    let Some(gpu) = s.gpu.as_mut() else {
        if !s.recovering {
            s.recovering = true;
            let (state, sched, canvas) = (state.clone(), scheduler.clone(), s.canvas.clone());
            wasm_bindgen_futures::spawn_local(async move {
                let result = create_gpu(&canvas, &sched).await;
                let Ok(mut s) = state.try_borrow_mut() else { return };
                s.recovering = false;
                match result {
                    Ok(gpu) => {
                        tracing::info!("GPU device recreated");
                        s.gpu = Some(gpu);
                    }
                    Err(e) => show_error(&canvas, &e.to_string()),
                }
            });
        }
        return;
    };

    let (css, dpr) = measure(&s.canvas);
    s.ui.set_viewport(css, dpr);
    let (w, h) = physical(css, dpr, gpu.context.device.limits().max_texture_dimension_2d);
    if w != gpu.config.width || h != gpu.config.height {
        s.canvas.set_width(w);
        s.canvas.set_height(h);
        gpu.config.width = w;
        gpu.config.height = h;
        gpu.surface.configure(&gpu.context.device, &gpu.config);
    }

    s.ui.set_time(now_seconds());
    let scene = s.ui.update();
    let frame = match gpu.surface.get_current_texture() {
        wgpu::CurrentSurfaceTexture::Success(f) | wgpu::CurrentSurfaceTexture::Suboptimal(f) => f,
        wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
            gpu.surface.configure(&gpu.context.device, &gpu.config);
            scheduler.schedule();
            return;
        }
        other => {
            tracing::warn!(?other, "skipping frame");
            return;
        }
    };
    let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());
    gpu.renderer.render(scene, &view, (w, h), dpr);
    gpu.context.queue.present(frame);
    if s.ui.needs_redraw() {
        scheduler.schedule();
    } else if let Some(at) = s.ui.next_wake() {
        scheduler.schedule_at(at);
    }
}

fn map_key(key: &str) -> Key {
    match key {
        "Enter" => Key::Enter,
        " " | "Spacebar" => Key::Space,
        "Tab" => Key::Tab,
        "Escape" => Key::Escape,
        "Backspace" => Key::Backspace,
        "Delete" => Key::Delete,
        "Home" => Key::Home,
        "End" => Key::End,
        "PageUp" => Key::PageUp,
        "PageDown" => Key::PageDown,
        "ArrowLeft" => Key::ArrowLeft,
        "ArrowRight" => Key::ArrowRight,
        "ArrowUp" => Key::ArrowUp,
        "ArrowDown" => Key::ArrowDown,
        k if k.chars().count() == 1 => Key::Character(k.to_owned()),
        k => Key::Other(k.to_owned()),
    }
}
