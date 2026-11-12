use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::rc::Rc;

use ravenrs_core::{
    BuildCause, BuildCauses, Color, CursorIcon, Effect, Event, FrameWork, Key, Modifiers, PhaseTimes, Point,
    PointerButton, Primitive, Quad, Rect, Scene, Size, ViewCost,
};
use ravenrs_layout::{Align, Axis, LayoutStyle, LayoutTree};
use ravenrs_reactive::{Observer, State};
use ravenrs_style::{Semantic, Theme};
use ravenrs_text::TextSystem;

use crate::View;
use crate::context::{
    BuildCx, Drag, DragPhase, Element, Interaction, LocalStore, NodeId, Overlay, Paint, PaintCx, Placement,
    QuadPaint, ScrollState, Shortcut, TextLeaf, WidgetId,
};
use crate::motion::{GhostLayer, MotionEntry, MotionSystem, NodePaint, Sampled, flipped};

mod access;

type Waker = Rc<RefCell<Option<Rc<dyn Fn()>>>>;

/// Lines scrolled per wheel notch are converted to pixels by the platform; this
/// is the width of the overlay scrollbar.
const SCROLLBAR_WIDTH: f32 = 4.0;
/// How long a scrollbar stays after the last scroll, and how long it takes to go.
const SCROLLBAR_HOLD: f64 = 0.9;
const SCROLLBAR_FADE: f64 = 0.25;
const SCROLLBAR_ALPHA: f32 = 0.45;

/// Longest time between frames that animations treat as having really passed.
const MAX_FRAME_GAP: f64 = 0.25;

/// How much a longer gap counts for: little enough not to skip the animation,
/// enough that a machine this slow still gets through it.
const STALL_STEP: f64 = 0.05;

/// Time between paints of animated drawing, in seconds.
const LIVE_FRAME: f64 = 1.0 / 60.0;

/// How far past a clip a node still gets painted: what it draws around itself, such as
/// a shadow, can reach into the clip.
const PAINT_MARGIN: f32 = 64.0;

/// Whether anything `frame` draws, with its margin, can land inside `clip`.
fn touches(frame: Rect, clip: Rect) -> bool {
    frame.max_x() + PAINT_MARGIN > clip.x()
        && frame.x() - PAINT_MARGIN < clip.max_x()
        && frame.max_y() + PAINT_MARGIN > clip.y()
        && frame.y() - PAINT_MARGIN < clip.max_y()
}

/// Work a [`Ui`] has done since it was created. These are exact counts, so a test
/// can demand that moving the pointer shapes no text or that an animation builds nothing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct UiStats {
    /// Times the views ran.
    pub builds: u64,
    /// Times sizes and positions were solved.
    pub layouts: u64,
    /// Times every element was given its place.
    pub placements: u64,
    /// Times the scene was painted.
    pub paints: u64,
    /// Times text was shaped.
    pub shapes: u64,
    /// Why the views ran.
    pub causes: BuildCauses,
}

/// How long each type of view takes, measured while it is switched on.
pub(crate) struct ViewProfile {
    clock: Rc<dyn Fn() -> f64>,
    costs: HashMap<&'static str, ViewCost>,
    open: Vec<OpenView>,
}

struct OpenView {
    name: &'static str,
    started: f64,
    elements: usize,
    shapes: u64,
    /// What the views inside took, which is not this view's cost.
    inner_time: f64,
    inner_elements: usize,
    inner_shapes: u64,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl ViewProfile {
    pub(crate) fn enter(&mut self, name: &'static str, elements: usize, shapes: u64) {
        let started = (self.clock)();
        self.open.push(OpenView {
            name,
            started,
            elements,
            shapes,
            inner_time: 0.0,
            inner_elements: 0,
            inner_shapes: 0,
        });
    }

    pub(crate) fn leave(&mut self, elements: usize, shapes: u64) {
        let Some(view) = self.open.pop() else { return };
        let total = ((self.clock)() - view.started).max(0.0);
        let made = elements.saturating_sub(view.elements);
        let shaped = shapes.saturating_sub(view.shapes);
        if let Some(outer) = self.open.last_mut() {
            outer.inner_time += total;
            outer.inner_elements += made;
            outer.inner_shapes += shaped;
        }
        let cost = self
            .costs
            .entry(view.name)
            .or_insert_with(|| ViewCost { name: view.name, ..ViewCost::default() });
        cost.count += 1;
        cost.time += std::time::Duration::from_secs_f64((total - view.inner_time).max(0.0));
        cost.elements += made.saturating_sub(view.inner_elements) as u32;
        cost.shapes += shaped.saturating_sub(view.inner_shapes) as u32;
    }
}

/// The platform-independent UI runtime: owns the root view, reacts to input and
/// state changes, and produces a [`Scene`] for the renderer.
///
/// Platform layers drive it with [`Ui::set_viewport`], [`Ui::handle_event`] and
/// [`Ui::update`], and register a waker with [`Ui::set_waker`] to schedule a
/// frame when state changes outside of event handling (timers, async tasks).
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct Ui {
    root: Box<dyn View>,
    theme: State<Theme>,
    text: TextSystem,
    layout: LayoutTree<TextLeaf>,
    elements: Vec<Element>,
    root_node: Option<NodeId>,
    overlays: Vec<Overlay>,
    /// Where focus returns when the open modal layer closes (`Some(None)`: nowhere).
    modal_return: Option<Option<WidgetId>>,
    /// Absolute frames, indexed like `elements`.
    frames: Vec<Rect>,
    /// The clip in effect where each element is painted.
    clips: Vec<Rect>,
    /// Scrollable content size of each element.
    content: Vec<Size>,
    /// Elements in painting (back-to-front) order; hit testing walks it backwards.
    paint_order: Vec<NodeId>,
    scroll: ScrollState,
    modifiers: Rc<Cell<Modifiers>>,
    /// Parent of each element, for hover-within.
    parents: Vec<Option<NodeId>>,
    /// The hovered element and its ancestors.
    hover_chain: Vec<WidgetId>,
    local: LocalStore,
    observer: Observer,
    state_dirty: Rc<Cell<bool>>,
    ui_waker: crate::UiWaker,
    waker: Waker,
    needs_build: bool,
    /// A view that builds from its own size got a new one in the last layout.
    viewport_moved: bool,
    viewport: Size,
    scale: f32,
    interaction: Interaction,
    /// The next placement scrolls the focused element into view (the keyboard
    /// or code moved focus; a click never asks for this).
    reveal_focus: bool,
    dragging: Option<WidgetId>,
    /// When each scroll container last moved: its scrollbar shows only around that.
    scrolled_at: HashMap<WidgetId, f64>,
    /// A scrollbar was on the last frame, so another one has to follow.
    scrollbar_showing: bool,
    /// Seconds since start, set by the platform.
    time: f64,
    shortcuts: Vec<Shortcut>,
    /// Where the focus was before this build moved it.
    moved_focus_from: Option<Option<WidgetId>>,
    activity: Option<crate::context::ActivityObserver>,
    access_tree_asked: bool,
    /// Earliest time a widget asked to be rebuilt at.
    wake: Option<f64>,
    pointer: Option<Point>,
    cursor: CursorIcon,
    scene: Scene,
    access_map: access::AccessMap,
    clipboard: crate::Clipboard,
    sound: crate::Sound,
    links: crate::Links,
    full_screen: crate::FullScreen,
    /// A scroll gliding to an anchor: which container, from and to, and when it began.
    glide: Option<(WidgetId, f32, f32, f64)>,
    /// Widgets whose `on_appear` already ran.
    appeared: std::collections::HashSet<WidgetId>,
    file_picker: crate::FilePicker,
    files_over: State<bool>,
    appearance: crate::SystemAppearance,
    motion: MotionSystem,
    motions: Vec<MotionEntry>,
    /// What scrolling added to each element's frame.
    shifts: Vec<Point>,
    /// The visible area of the scroll container around each element (the window for
    /// those outside any), for pinned elements and scroll-linked effects.
    views: Vec<Rect>,
    /// The picture's palette, to notice a theme change.
    painted_palette: Option<ravenrs_style::Palette>,
    /// The next build lands colors at once, because the picture crossfades.
    crossfade: bool,
    /// The next layout lands frames at once (the window was resized).
    snap_layout: bool,
    /// The system asked for reduced motion.
    system_reduced_motion: bool,
    /// When animated drawing wants its next paint.
    next_live: Option<f64>,
    /// The latest paint had animated drawing.
    has_live: bool,
    /// A value from `BuildCx::animated` was moving in the latest build.
    value_moving: bool,
    painting_overlays: bool,
    stats: UiStats,
    /// Tells the seconds, when someone wants to know how long the phases take.
    phase_clock: Option<Rc<dyn Fn() -> f64>>,
    phases: PhaseTimes,
    /// Why the next build happens, when it was asked for directly.
    build_cause: Option<BuildCause>,
    frame: FrameWork,
    /// Phase times and shapes when the current frame started.
    frame_start: (PhaseTimes, u64),
    view_profile: Option<Rc<RefCell<ViewProfile>>>,
    hover_readers: Rc<RefCell<std::collections::HashSet<WidgetId>>>,
}

impl fmt::Debug for Ui {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Ui")
            .field("elements", &self.elements.len())
            .field("viewport", &self.viewport)
            .finish_non_exhaustive()
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose(part = "part1"))]
impl Ui {
    /// A runtime for `root` under `theme`, with an 800×600 viewport at scale 1.
    pub fn new(root: impl View + 'static, theme: Theme) -> Self {
        let state_dirty = Rc::new(Cell::new(true));
        let waker: Waker = Rc::new(RefCell::new(None));
        let (dirty, wake) = (state_dirty.clone(), waker.clone());
        let observer = Observer::new(move || {
            dirty.set(true);
            let callback = wake.borrow().clone();
            if let Some(callback) = callback {
                callback();
            }
        });
        Self {
            root: Box::new(root),
            theme: State::new(theme),
            text: TextSystem::new(),
            layout: LayoutTree::new(),
            elements: Vec::new(),
            root_node: None,
            overlays: Vec::new(),
            modal_return: None,
            frames: Vec::new(),
            clips: Vec::new(),
            content: Vec::new(),
            paint_order: Vec::new(),
            scroll: ScrollState::default(),
            modifiers: Rc::new(Cell::new(Modifiers::default())),
            parents: Vec::new(),
            hover_chain: Vec::new(),
            local: LocalStore::default(),
            observer,
            state_dirty,
            ui_waker: crate::UiWaker::default(),
            waker,
            needs_build: true,
            viewport_moved: false,
            viewport: Size::new(800.0, 600.0),
            scale: 1.0,
            interaction: Interaction::default(),
            reveal_focus: false,
            dragging: None,
            scrolled_at: HashMap::new(),
            scrollbar_showing: false,
            time: 0.0,
            shortcuts: Vec::new(),
            moved_focus_from: None,
            activity: None,
            access_tree_asked: false,
            wake: None,
            pointer: None,
            cursor: CursorIcon::Default,
            scene: Scene::new(),
            access_map: access::AccessMap::default(),
            clipboard: crate::Clipboard::default(),
            sound: crate::Sound::default(),
            links: crate::Links::default(),
            full_screen: crate::FullScreen::default(),
            glide: None,
            appeared: std::collections::HashSet::new(),
            file_picker: crate::FilePicker::default(),
            files_over: State::new(false),
            appearance: crate::SystemAppearance::default(),
            motion: MotionSystem::default(),
            motions: Vec::new(),
            shifts: Vec::new(),
            views: Vec::new(),
            painted_palette: None,
            crossfade: false,
            snap_layout: false,
            system_reduced_motion: false,
            next_live: None,
            has_live: false,
            value_moving: false,
            painting_overlays: false,
            stats: UiStats::default(),
            phase_clock: None,
            phases: PhaseTimes::default(),
            build_cause: Some(BuildCause::First),
            frame: FrameWork::default(),
            frame_start: (PhaseTimes::default(), 0),
            view_profile: None,
            hover_readers: Rc::default(),
        }
    }

    fn ask_build(&mut self, cause: BuildCause) {
        self.needs_build = true;
        self.build_cause.get_or_insert(cause);
    }

    /// Why this rebuild is happening, and clears it.
    fn take_build_cause(&mut self) -> BuildCause {
        if let Some(cause) = self.build_cause.take() {
            cause
        } else if self.state_dirty.get() {
            BuildCause::Data
        } else if self.ui_waker.is_dirty() {
            BuildCause::Remote
        } else {
            BuildCause::Clock
        }
    }

    /// What was done since the last call: a platform asks once per frame.
    pub fn take_frame_work(&mut self) -> FrameWork {
        let shapes = self.text.stats().shapes;
        let (phases, shapes_before) = self.frame_start;
        let now = self.phases;
        let mut work = std::mem::take(&mut self.frame);
        work.phases = PhaseTimes {
            build: now.build - phases.build,
            layout: now.layout - phases.layout,
            place: now.place - phases.place,
            paint: now.paint - phases.paint,
        };
        work.shapes = (shapes - shapes_before) as u32;
        work.elements = self.elements.len() as u32;
        self.frame_start = (now, shapes);
        work
    }

    /// Measures every type of view too, on top of the clock of [`Ui::set_phase_clock`].
    /// It costs a little per view, so it stays off unless someone is looking.
    pub fn set_view_profiling(&mut self, on: bool) {
        self.view_profile = match (&self.phase_clock, on) {
            (Some(clock), true) => Some(self.view_profile.take().unwrap_or_else(|| {
                Rc::new(RefCell::new(ViewProfile {
                    clock: clock.clone(),
                    costs: HashMap::new(),
                    open: Vec::new(),
                }))
            })),
            _ => None,
        };
    }

    /// What each type of view cost in the latest build, the most expensive first. Empty
    /// without [`Ui::set_view_profiling`].
    pub fn view_costs(&self) -> Vec<ViewCost> {
        let Some(profile) = &self.view_profile else { return Vec::new() };
        let mut costs: Vec<ViewCost> = profile.borrow().costs.values().cloned().collect();
        costs.sort_by(|a, b| b.time.cmp(&a.time).then(a.name.cmp(b.name)));
        costs
    }

    /// What this runtime has done so far.
    pub fn stats(&self) -> UiStats {
        UiStats { shapes: self.text.stats().shapes, ..self.stats }
    }

    /// Measures the phases with `clock`, which tells the seconds. Without a clock,
    /// which is the default, measuring costs nothing. WebAssembly has no clock of its
    /// own, so it is the platform that passes one in.
    pub fn set_phase_clock(&mut self, clock: Option<Rc<dyn Fn() -> f64>>) {
        self.phase_clock = clock;
    }

    /// Time spent in each phase since the clock was set; see [`Ui::set_phase_clock`].
    pub fn phase_times(&self) -> PhaseTimes {
        self.phases
    }

    fn phase_start(&self) -> Option<f64> {
        self.phase_clock.as_ref().map(|clock| clock())
    }

    fn phase_elapsed(&self, started: Option<f64>) -> std::time::Duration {
        match (started, &self.phase_clock) {
            (Some(started), Some(clock)) => std::time::Duration::from_secs_f64((clock() - started).max(0.0)),
            _ => std::time::Duration::ZERO,
        }
    }

    /// Installs the system clipboard (platform layers do this); the default is
    /// an in-memory clipboard.
    pub fn set_clipboard(&mut self, provider: impl crate::ClipboardProvider + 'static) {
        self.clipboard = crate::Clipboard::new(provider);
        self.ask_build(BuildCause::System);
    }

    /// The clipboard widgets copy to and paste from.
    pub fn clipboard(&self) -> crate::Clipboard {
        self.clipboard.clone()
    }

    /// Installs how addresses open (the platform layers do). Until then they are only
    /// recorded, which tests read through [`crate::Links::opened`].
    pub fn set_links(&mut self, provider: impl crate::LinkOpener + 'static) {
        self.links.set_opener(provider);
    }

    /// The links service views open addresses through.
    pub fn links(&self) -> crate::Links {
        self.links.clone()
    }

    /// Installs how the app enters and leaves full screen (the platform layers do).
    /// Until then the request is only remembered.
    pub fn set_full_screen(&mut self, provider: impl crate::FullScreenProvider + 'static) {
        self.full_screen = crate::FullScreen::new(provider);
        self.ask_build(BuildCause::System);
    }

    /// The full screen service views ask through.
    pub fn full_screen(&self) -> crate::FullScreen {
        self.full_screen.clone()
    }

    /// Installs the system's sound (platform layers do this); the default is silence.
    pub fn set_sound(&mut self, provider: impl crate::SoundProvider + 'static) {
        self.sound = crate::Sound::new(provider);
        self.ask_build(BuildCause::System);
    }

    /// The sound system views play through.
    pub fn sound(&self) -> crate::Sound {
        self.sound.clone()
    }

    /// Installs the system's chooser of files (platform layers do this); the default
    /// chooses nothing.
    pub fn set_file_picker(&mut self, provider: impl crate::FilePickerProvider + 'static) {
        self.file_picker = crate::FilePicker::new(provider);
        self.ask_build(BuildCause::System);
    }

    /// Says whether files are being dragged over the window (platform layers do this).
    pub fn set_files_over(&mut self, over: bool) {
        self.files_over.set_if_changed(over);
    }

    /// Hands dropped files to the place under `position`. When no place is there, and the
    /// screen has only one, it goes there: not every system says where a drop landed.
    /// Whether anything took them.
    pub fn drop_files(&mut self, position: Option<Point>, files: Vec<crate::PickedFile>) -> bool {
        self.files_over.set_if_changed(false);
        let mut node = position.and_then(|p| self.hit_test(p));
        while let Some(n) = node {
            if let Some(handler) = self.elements[n.0 as usize].on_file_drop.clone() {
                handler(files);
                return true;
            }
            node = self.parents.get(n.0 as usize).copied().flatten();
        }
        let mut places = self.elements.iter().filter_map(|e| e.on_file_drop.clone());
        match (places.next(), places.next()) {
            (Some(only), None) => {
                only(files);
                true
            }
            _ => false,
        }
    }

    /// Called whenever reactive state the UI depends on changes.
    pub fn set_waker(&mut self, waker: impl Fn() + 'static) {
        *self.waker.borrow_mut() = Some(Rc::new(waker));
    }

    /// A handle that asks for a rebuild from another thread; see [`UiWaker`](crate::UiWaker).
    pub fn ui_waker(&self) -> crate::UiWaker {
        self.ui_waker.clone()
    }

    /// Tells a platform when a [`UiWaker`](crate::UiWaker) fires, from whichever thread that is,
    /// so it can leave its sleep. Only the first call counts.
    pub fn set_remote_waker(&mut self, notify: impl Fn() + Send + Sync + 'static) {
        self.ui_waker.set_notify(Box::new(notify));
    }

    /// Sets the viewport in logical pixels and the device scale factor.
    pub fn set_viewport(&mut self, size: Size, scale: f32) {
        if size != self.viewport || scale != self.scale {
            self.viewport = size;
            self.scale = scale;
            self.ask_build(BuildCause::Viewport);
            self.snap_layout = true;
        }
    }

    /// The viewport size in logical pixels.
    pub fn viewport(&self) -> Size {
        self.viewport
    }

    /// The device scale factor (physical pixels per logical pixel).
    pub fn scale(&self) -> f32 {
        self.scale
    }

    /// The current theme.
    pub fn theme(&self) -> Theme {
        *self.theme.peek()
    }

    /// Switches the theme; components can do the same through `BuildCx::theme_state`.
    pub fn set_theme(&mut self, theme: Theme) {
        self.theme.set(theme);
    }

    /// The theme as reactive state.
    pub fn theme_state(&self) -> State<Theme> {
        self.theme.clone()
    }

    /// Says whether the system is dark (platform layers do this, at the start and on every
    /// change). While the theme follows the system, it switches to match.
    pub fn set_system_dark(&mut self, dark: bool) {
        self.appearance.report(dark, &self.theme);
    }

    /// Makes the theme follow the system between Raven Dark and Raven Light, or stop.
    pub fn follow_system_theme(&mut self, follow: bool) {
        self.appearance.set_follow(follow, &self.theme);
    }

    /// Whether the system is light or dark, and whether the theme follows it.
    pub fn system_appearance(&self) -> crate::SystemAppearance {
        self.appearance.clone()
    }

    /// Fonts registered here become available to every `Text`.
    pub fn text_system(&mut self) -> &mut TextSystem {
        &mut self.text
    }

    /// True when the next [`Ui::update`] would produce a different scene.
    pub fn needs_redraw(&self) -> bool {
        self.needs_rebuild() || self.motion.active || self.live_is_due()
    }

    fn needs_rebuild(&self) -> bool {
        self.needs_build
            || self.state_dirty.get()
            || self.ui_waker.is_dirty()
            || self.wake.is_some_and(|w| self.time >= w)
    }

    fn live_is_due(&self) -> bool {
        self.next_live.is_some_and(|t| self.time >= t)
    }

    /// True while an animation is running, which means the next frame will
    /// differ from this one even if nothing else happens.
    pub fn is_animating(&self) -> bool {
        self.motion.active || self.value_moving
    }

    /// Sets the clock (seconds since the app started). Platforms call it before
    /// handling events and before each frame. Animations start only once a
    /// platform drives this clock.
    ///
    /// Animations follow their own clock, which skips stalls: after a long pause
    /// (a hidden window, a slow frame) they continue about where they were
    /// instead of jumping to the end.
    pub fn set_time(&mut self, seconds: f64) {
        let elapsed = seconds - self.time;
        self.motion.clock += if (0.0..=MAX_FRAME_GAP).contains(&elapsed) { elapsed } else { STALL_STEP };
        self.motion.enabled = true;
        self.time = seconds;
    }

    /// Tells the runtime that the system asks for reduced motion. Things stop
    /// traveling and springing; colors and opacity may still fade. A theme can
    /// ask for the same through its `reduced_motion` token.
    pub fn set_reduced_motion(&mut self, reduced: bool) {
        if self.system_reduced_motion != reduced {
            self.system_reduced_motion = reduced;
            self.ask_build(BuildCause::System);
        }
    }

    /// When the UI wants its next frame even without input (seconds, same clock
    /// as [`Ui::set_time`]). Platforms schedule a timer for it.
    pub fn next_wake(&self) -> Option<f64> {
        match (self.wake, self.next_live) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        }
    }

    /// True while a pointer drag that started on a draggable element (a slider,
    /// a splitter) is in progress. Touch platforms use it to tell drags from scrolls.
    pub fn is_dragging(&self) -> bool {
        self.dragging.is_some()
    }

    /// The cursor the platform should show.
    pub fn cursor(&self) -> CursorIcon {
        self.cursor
    }

    /// Rebuilds, lays out and paints as needed, then returns the current scene.
    pub fn update(&mut self) -> &Scene {
        let anchor = self.links.take_anchor();
        if self.needs_rebuild() {
            self.rebuild();
            // The tree changed under a stationary pointer: refresh hover once.
            if self.refresh_hover() {
                self.rebuild();
            }
            // Those views were built for their old size. Their new build can move
            // others, but a second round of that waits for the next frame.
            if std::mem::take(&mut self.viewport_moved) {
                self.rebuild();
                self.viewport_moved = false;
            }
        } else if self.glide.is_some() && anchor.is_none() {
            self.advance_glide();
        } else if self.motion.active || self.live_is_due() {
            // Only animations are due: the tree and its layout are still valid.
            let _span = tracing::trace_span!("ravenrs::motion_frame").entered();
            self.paint();
        }
        if let Some(name) = anchor {
            self.start_glide(&name);
        }
        &self.scene
    }

    /// Starts scrolling the container around the anchor `name` so the anchor lands at
    /// its offset below the top. Under reduced motion it jumps.
    fn start_glide(&mut self, name: &str) {
        let Some(i) = self.elements.iter().position(|e| e.anchor.as_ref().is_some_and(|(n, _)| &**n == name))
        else {
            tracing::debug!(name, "no anchor of that name");
            return;
        };
        let offset = self.elements[i].anchor.as_ref().map_or(0.0, |a| a.1);
        let mut node = self.parents.get(i).copied().flatten();
        while let Some(n) = node {
            let k = n.0 as usize;
            if self.elements[k].scroll {
                let current = self.clamped_offset(k);
                let to = (current.y + self.frames[i].y() - self.frames[k].y() - offset)
                    .clamp(0.0, self.max_offset(k).y);
                let widget = self.elements[k].widget;
                if self.reduced_motion() {
                    self.scroll.offsets.insert(widget, Point::new(current.x, to));
                    self.place_and_paint();
                } else {
                    self.glide = Some((widget, current.y, to, self.time));
                    self.next_live = Some(self.time);
                }
                return;
            }
            node = self.parents.get(k).copied().flatten();
        }
    }

    /// One frame of a glide: eases in and out over about half a second.
    fn advance_glide(&mut self) {
        let Some((widget, from, to, start)) = self.glide else { return };
        const GLIDE: f64 = 0.55;
        let t = ((self.time - start) / GLIDE).clamp(0.0, 1.0) as f32;
        let eased = ravenrs_animation::Easing::EaseInOut.sample(t);
        let x = self.scroll.offsets.get(&widget).map_or(0.0, |o| o.x);
        self.scroll.offsets.insert(widget, Point::new(x, from + (to - from) * eased));
        self.scrolled_at.insert(widget, self.time);
        if t >= 1.0 {
            self.glide = None;
        } else {
            self.next_live = Some(self.time + LIVE_FRAME);
        }
        self.place_and_paint();
        self.refresh_hover();
    }

    /// Runs the `on_appear` handlers of elements that show for the first time.
    fn tell_appeared(&mut self) {
        let mut due = Vec::new();
        for (i, e) in self.elements.iter().enumerate() {
            let Some(handler) = &e.on_appear else { continue };
            if self.appeared.contains(&e.widget) {
                continue;
            }
            let (frame, view) = (self.frames[i], self.views[i]);
            if frame.max_y() > view.y()
                && frame.y() < view.max_y()
                && frame.max_x() > view.x()
                && frame.x() < view.max_x()
            {
                due.push((e.widget, handler.clone()));
            }
        }
        for (widget, handler) in due {
            self.appeared.insert(widget);
            handler();
        }
    }

    /// The last painted scene.
    pub fn scene(&self) -> &Scene {
        &self.scene
    }

    /// Number of elements produced by the last build.
    pub fn element_count(&self) -> usize {
        self.elements.len()
    }

    fn tell_activity(&self, activity: crate::context::UiActivity<'_>) {
        if let Some(observer) = self.activity.clone() {
            observer(activity, self.time);
        }
    }

    /// Feeds one input event; returns true when a redraw is needed.
    pub fn handle_event(&mut self, event: &Event) -> bool {
        match event {
            Event::PointerMove { position } => {
                self.pointer = Some(*position);
                self.tell_activity(crate::context::UiActivity::PointerMoved(*position));
                if let Some(widget) = self.dragging {
                    self.drag(widget, DragPhase::Move, *position);
                }
                self.refresh_hover();
                if let Some(node) = self.hit_test(*position) {
                    let i = node.0 as usize;
                    if let Some(handler) = self.elements[i].on_pointer_move.clone() {
                        handler(*position, self.frames[i]);
                    }
                }
            }
            Event::PointerDown { position, button: PointerButton::Primary } => {
                self.pointer = Some(*position);
                self.tell_activity(crate::context::UiActivity::Clicked(*position));
                self.reveal_focus = false;
                self.dismiss_overlays_outside(*position);
                self.refresh_hover();
                let target = self.hit_test(*position);
                let element = target.map(|n| &self.elements[n.0 as usize]);
                let widget = element.map(|e| e.widget);
                let draggable = element.is_some_and(|e| e.on_drag.is_some());
                if self.interaction.pressed != widget {
                    self.interaction.pressed = widget;
                    self.ask_build(BuildCause::Press);
                }
                // Like the DOM, a click focuses the nearest focusable element at or above the target.
                let mut node = target;
                let mut focused = None;
                while let Some(n) = node {
                    let e = &self.elements[n.0 as usize];
                    if e.focus_radius.is_some() {
                        focused = Some(e.widget);
                        break;
                    }
                    node = self.parents.get(n.0 as usize).copied().flatten();
                }
                if self.interaction.focused != focused || self.interaction.focus_visible {
                    self.interaction.focused = focused;
                    self.interaction.focus_visible = false;
                    self.ask_build(BuildCause::Focus);
                }
                if let (true, Some(w)) = (draggable, widget) {
                    self.dragging = Some(w);
                    self.drag(w, DragPhase::Start, *position);
                }
            }
            Event::PointerUp { position, button: PointerButton::Primary } => {
                if let Some(w) = self.dragging.take() {
                    self.drag(w, DragPhase::End, *position);
                }
                let pressed = self.interaction.pressed.take();
                if pressed.is_some() {
                    self.ask_build(BuildCause::Press);
                }
                if let Some(node) = self.hit_test(*position) {
                    let element = &self.elements[node.0 as usize];
                    if pressed == Some(element.widget) {
                        let handler = element.on_click.clone();
                        if let Some(handler) = handler {
                            handler();
                        }
                    }
                }
            }
            Event::PointerLeave => {
                self.pointer = None;
                self.refresh_hover();
                if self.dragging.is_none() && self.interaction.pressed.take().is_some() {
                    self.ask_build(BuildCause::Press);
                }
            }
            Event::Scroll { position, delta } => {
                if self.scroll(*position, *delta) {
                    if !self.needs_build {
                        self.place_and_paint();
                        self.refresh_hover();
                    }
                    return true;
                }
            }
            Event::Pinch { position, delta } => {
                self.pinch(*position, *delta);
            }
            Event::PointerDown { position, button: PointerButton::Secondary } => {
                self.pointer = Some(*position);
                self.dismiss_overlays_outside(*position);
                let mut node = self.hit_test(*position);
                while let Some(n) = node {
                    if let Some(handler) = self.elements[n.0 as usize].on_context_menu.clone() {
                        handler(*position);
                        break;
                    }
                    if let Some(handler) = self.elements[n.0 as usize].on_context_menu_in.clone() {
                        handler(*position, self.frames[n.0 as usize]);
                        break;
                    }
                    node = self.parents.get(n.0 as usize).copied().flatten();
                }
            }
            Event::DoubleClick { position } => {
                if let Some(node) = self.hit_test(*position) {
                    let handler = self.elements[node.0 as usize].on_double_click.clone();
                    if let Some(handler) = handler {
                        handler();
                    }
                }
            }
            Event::KeyUp { modifiers, .. } => self.modifiers.set(*modifiers),
            Event::KeyDown { key, modifiers } => {
                self.modifiers.set(*modifiers);
                if modifiers.alt {
                    let base = modifiers.base_key.map(|c| Key::Character(c.to_string()));
                    let shortcut = self.shortcuts.iter().find(|s| {
                        s.alt && [Some(key), base.as_ref()].into_iter().flatten().any(|k| same_key(&s.key, k))
                    });
                    if let Some(s) = shortcut.map(|s| s.handler.clone()) {
                        s();
                        return self.needs_redraw();
                    }
                }
                let handler = self.focused_element().and_then(|e| e.on_key.clone());
                if let Some(handler) = handler
                    && handler(key, *modifiers)
                {
                    return self.needs_redraw();
                }
                let command = modifiers.ctrl || modifiers.meta;
                // Escape closes what opened last: a menu before anything that listens for it.
                if *key == Key::Escape
                    && let Some(dismiss) = self.overlays.iter().rev().find_map(|o| o.on_dismiss.clone())
                {
                    dismiss();
                    return self.needs_redraw();
                }
                let shortcut = self.shortcuts.iter().find(|s| {
                    !s.alt && s.command == command && (!s.shift || modifiers.shift) && same_key(&s.key, key)
                });
                if let Some(s) = shortcut.map(|s| s.handler.clone()) {
                    s();
                    return self.needs_redraw();
                }
                match key {
                    Key::Tab => self.move_focus(!modifiers.shift),
                    Key::Enter | Key::Space => {
                        let handler = self.focused_element().and_then(|e| e.on_click.clone());
                        if let Some(handler) = handler {
                            handler();
                        }
                    }
                    Key::Escape => {
                        let dismiss = self.overlays.iter().rev().find_map(|o| o.on_dismiss.clone());
                        if let Some(dismiss) = dismiss {
                            dismiss();
                        } else if self.interaction.focused.take().is_some() {
                            self.ask_build(BuildCause::Focus);
                        }
                    }
                    _ => {}
                }
            }
            Event::FocusLost => {
                self.dragging = None;
                if self.interaction.pressed.take().is_some() {
                    self.ask_build(BuildCause::Press);
                }
            }
            _ => {}
        }
        self.needs_redraw()
    }

    fn focused_element(&self) -> Option<&Element> {
        let w = self.interaction.focused?;
        self.elements.iter().find(|e| e.widget == w && e.focus_radius.is_some())
    }

    /// The topmost full-window layer (dialog, sheet, command palette): while it
    /// is open, focus stays inside it.
    fn modal_root(&self) -> Option<NodeId> {
        self.overlays.iter().rev().find(|o| o.placement == Placement::Fill).map(|o| o.root)
    }

    /// A modal layer and what is above it: a menu opened from it counts as inside it.
    fn modal_layers(&self, root: NodeId) -> Vec<NodeId> {
        let from = self.overlays.iter().position(|o| o.root == root).unwrap_or(0);
        self.overlays[from..].iter().map(|o| o.root).collect()
    }

    fn is_inside(&self, mut node: NodeId, root: NodeId) -> bool {
        loop {
            if node == root {
                return true;
            }
            match self.parents.get(node.0 as usize).copied().flatten() {
                Some(parent) => node = parent,
                None => return false,
            }
        }
    }

    /// The first focusable element under `root`, in tree order.
    fn first_focusable(&self, root: NodeId) -> Option<WidgetId> {
        let mut stack = vec![root];
        while let Some(n) = stack.pop() {
            let e = &self.elements[n.0 as usize];
            if e.focus_radius.is_some() {
                return Some(e.widget);
            }
            stack.extend(e.children.iter().rev());
        }
        None
    }

    /// Keeps focus inside an open modal layer, moving it in when the layer
    /// opens and back to where it was when the layer closes.
    fn trap_focus(&mut self) {
        let moved_from = self.moved_focus_from.take();
        match self.modal_root() {
            Some(root) => {
                // A field in the layer that took the focus as it opened: where to go back is where
                // the focus was before that.
                if let Some(before) = moved_from {
                    self.modal_return.get_or_insert(before);
                }
                let focused = self.interaction.focused;
                let layers = self.modal_layers(root);
                let inside = (0..self.elements.len()).any(|i| {
                    let e = &self.elements[i];
                    Some(e.widget) == focused
                        && e.focus_radius.is_some()
                        && layers.iter().any(|layer| self.is_inside(NodeId(i as u32), *layer))
                });
                if !inside {
                    self.modal_return.get_or_insert(focused);
                    // With nothing to focus in the layer, focus still leaves the page behind it.
                    let first = self.first_focusable(root);
                    if self.interaction.focused != first {
                        self.interaction.focused = first;
                        self.ask_build(BuildCause::Focus);
                    }
                }
            }
            None => {
                if let Some(back) = self.modal_return.take() {
                    self.interaction.focused = back;
                    self.ask_build(BuildCause::Focus);
                }
            }
        }
    }

    fn drag(&mut self, widget: WidgetId, phase: DragPhase, position: Point) {
        let found = self
            .elements
            .iter()
            .enumerate()
            .find(|(_, e)| e.widget == widget && e.on_drag.is_some())
            .and_then(|(i, e)| {
                let parent = self.parents.get(i).copied().flatten();
                let container = parent.and_then(|p| self.frames.get(p.0 as usize).copied());
                let frame = self.frames.get(i).copied()?;
                Some((e.on_drag.clone()?, frame, container.unwrap_or(frame)))
            });
        if let Some((handler, frame, container)) = found {
            handler(Drag { phase, position, frame, container });
        }
    }

    fn dismiss_overlays_outside(&mut self, p: Point) {
        let mut handlers = Vec::new();
        for (i, overlay) in self.overlays.iter().enumerate() {
            let Some(dismiss) = &overlay.on_dismiss else { continue };
            let inside = |n: NodeId| self.frames.get(n.0 as usize).is_some_and(|f| f.contains(p));
            let in_anchor = overlay.placement != Placement::Fill && inside(overlay.anchor);
            // A menu opened from this layer may stick out of it, and a click there is not outside.
            let in_popup_above =
                self.overlays[i + 1..].iter().any(|o| o.placement != Placement::Fill && inside(o.root));
            if !inside(overlay.root) && !in_anchor && !in_popup_above {
                handlers.push(dismiss.clone());
            }
        }
        for h in handlers {
            h();
        }
    }

    /// Hands a zoom gesture to the topmost element under `p` that takes it. An
    /// open modal layer keeps it from what is behind.
    fn pinch(&mut self, p: Point, delta: f32) {
        let layers = self.modal_root().map(|root| self.modal_layers(root));
        let handler = self.paint_order.iter().rev().find_map(|&node| {
            let i = node.0 as usize;
            let e = &self.elements[i];
            let reachable = layers.as_ref().is_none_or(|l| l.iter().any(|root| self.is_inside(node, *root)));
            let inside = self.frames[i].intersect(&self.clips[i]).contains(p);
            if reachable && inside { e.on_pinch.clone() } else { None }
        });
        if let Some(handler) = handler {
            handler(delta);
        }
    }

    /// Scrolls the innermost scroll container under `p` that can move in the
    /// direction of `delta`. Returns true if something scrolled. An open modal
    /// layer keeps it from what is behind, even when its own content is at the end.
    fn scroll(&mut self, p: Point, delta: Point) -> bool {
        let layers = self.modal_root().map(|root| self.modal_layers(root));
        for &node in self.paint_order.iter().rev() {
            let i = node.0 as usize;
            let e = &self.elements[i];
            let wheel = e.on_wheel.clone();
            if !(e.scroll || e.scrolls_with_widget || wheel.is_some())
                || !self.frames[i].intersect(&self.clips[i]).contains(p)
            {
                continue;
            }
            if !layers.as_ref().is_none_or(|l| l.iter().any(|root| self.is_inside(node, *root))) {
                continue;
            }
            if let Some(handler) = wheel {
                handler(delta);
                return true;
            }
            // Part of a view that scrolls moves what scrolls, such as frozen row names.
            let i = if e.scroll {
                i
            } else {
                let widget = e.widget;
                match self.elements.iter().position(|other| other.scroll && other.widget == widget) {
                    Some(found) => found,
                    None => continue,
                }
            };
            let e = &self.elements[i];
            let max = self.max_offset(i);
            let current = self.scroll.offsets.get(&e.widget).copied().unwrap_or(Point::ZERO);
            let delta = if e.wheel_sideways && delta.x == 0.0 { Point::new(delta.y, 0.0) } else { delta };
            let next = Point::new(
                if e.scroll_x { (current.x - delta.x).clamp(0.0, max.x) } else { 0.0 },
                (current.y - delta.y).clamp(0.0, max.y),
            );
            if (next.x - current.x).abs() > f32::EPSILON || (next.y - current.y).abs() > f32::EPSILON {
                self.scroll.offsets.insert(e.widget, next);
                self.scrolled_at.insert(e.widget, self.time);
                self.scrollbar_showing = true;
                self.next_live = Some(self.time);
                let e = &self.elements[i];
                if e.rebuild_on_scroll {
                    self.ask_build(BuildCause::Scroll);
                }
                return true;
            }
        }
        false
    }

    fn max_offset(&self, i: usize) -> Point {
        let (content, frame) = (self.content[i], self.frames[i]);
        Point::new((content.width - frame.width()).max(0.0), (content.height - frame.height()).max(0.0))
    }

    fn clamped_offset(&self, i: usize) -> Point {
        let max = self.max_offset(i);
        let o = self.scroll.offsets.get(&self.elements[i].widget).copied().unwrap_or(Point::ZERO);
        Point::new(o.x.clamp(0.0, max.x), o.y.clamp(0.0, max.y))
    }

    fn move_focus(&mut self, forward: bool) {
        let layers = self.modal_root().map(|root| self.modal_layers(root));
        let focusable: Vec<WidgetId> = self
            .paint_order
            .iter()
            .filter(|n| layers.as_ref().is_none_or(|l| l.iter().any(|root| self.is_inside(**n, *root))))
            .map(|n| &self.elements[n.0 as usize])
            .filter(|e| e.focus_radius.is_some())
            .map(|e| e.widget)
            .collect();
        if focusable.is_empty() {
            return;
        }
        let current = self.interaction.focused.and_then(|w| focusable.iter().position(|f| *f == w));
        let next = match (current, forward) {
            (None, true) => 0,
            (None, false) => focusable.len() - 1,
            (Some(i), true) => (i + 1) % focusable.len(),
            (Some(i), false) => (i + focusable.len() - 1) % focusable.len(),
        };
        self.interaction.focused = Some(focusable[next]);
        self.interaction.focus_visible = true;
        self.reveal_focus = true;
        self.ask_build(BuildCause::Focus);
    }

    /// Recomputes the hovered widget and cursor; returns true if hover changed.
    fn refresh_hover(&mut self) -> bool {
        let target = self.pointer.and_then(|p| self.hit_test(p));
        let element = target.map(|n| &self.elements[n.0 as usize]);
        let hovered = element.map(|e| e.widget);
        self.cursor = element.and_then(|e| e.cursor).unwrap_or_default();
        let mut chain = Vec::new();
        let mut node = target;
        while let Some(n) = node {
            chain.push(self.elements[n.0 as usize].widget);
            node = self.parents.get(n.0 as usize).copied().flatten();
        }
        if hovered != self.interaction.hovered || chain != self.hover_chain {
            // Only a view that asked about the pointer can look different now. Those that stay
            // in the chain keep their answer, unless the pointer moved on to or off them.
            let readers = self.hover_readers.borrow();
            let matters = |widget: &WidgetId| readers.contains(widget);
            let affected = chain.iter().filter(|w| !self.hover_chain.contains(w)).any(matters)
                || self.hover_chain.iter().filter(|w| !chain.contains(w)).any(matters)
                || hovered.iter().chain(&self.interaction.hovered).any(matters);
            drop(readers);
            self.interaction.hovered = hovered;
            self.hover_chain = chain;
            if affected {
                self.ask_build(BuildCause::Hover);
            }
            return affected;
        }
        false
    }

    fn hit_test(&self, p: Point) -> Option<NodeId> {
        self.paint_order.iter().rev().copied().find(|n| {
            let i = n.0 as usize;
            self.elements[i].is_interactive()
                && self.frames.get(i).zip(self.clips.get(i)).is_some_and(|(f, c)| f.intersect(c).contains(p))
        })
    }

    fn rebuild(&mut self) {
        let _span = tracing::trace_span!("ravenrs::frame").entered();
        self.needs_build = false;
        let cause = self.take_build_cause();
        self.build_cause = Some(cause);
        self.state_dirty.set(false);
        // Before the build: a wake during it must cause another one.
        self.ui_waker.clear();
        if let Err(e) = self.build() {
            tracing::error!(error = %e, "build failed; keeping the previous frame");
            return;
        }
        if let Err(e) = self.compute_layout() {
            tracing::error!(error = %e, "layout failed; keeping the previous frame");
            return;
        }
        self.place_and_paint();
        self.forget_stale_frames();
    }

    /// Drops the frames of elements the latest build did not ask about. Measured rows come
    /// and go as a list scrolls, and each one would otherwise stay in this map for good;
    /// what the list must remember of a row it keeps in its own track.
    fn forget_stale_frames(&mut self) {
        let asked = |e: &&Element| e.scroll || e.report_frame;
        // Counted first so that an ordinary frame allocates nothing here.
        if self.scroll.viewports.len() <= self.elements.iter().filter(asked).count() {
            return;
        }
        let live: HashSet<WidgetId> = self.elements.iter().filter(asked).map(|e| e.widget).collect();
        self.scroll.viewports.retain(|widget, _| live.contains(widget));
    }

    fn build(&mut self) -> Result<(), ravenrs_layout::LayoutError> {
        let _span = tracing::trace_span!("ravenrs::build").entered();
        self.stats.builds += 1;
        self.frame.builds += 1;
        let cause = self.take_build_cause();
        self.stats.causes.add(cause);
        self.frame.cause.get_or_insert(cause);
        if let Some(profile) = &self.view_profile {
            profile.borrow_mut().costs.clear();
        }
        let started = self.phase_start();
        let result = self.build_views();
        self.phases.build += self.phase_elapsed(started);
        result
    }

    fn build_views(&mut self) -> Result<(), ravenrs_layout::LayoutError> {
        self.layout.clear();
        self.elements.clear();
        self.root_node = None;
        self.overlays.clear();
        let placeholder = self.layout.node(LayoutStyle::leaf(), &[])?;
        let viewport = self.viewport;
        let theme = self.theme.clone();
        self.local.begin_build();
        let modifiers = self.modifiers.clone();
        let clipboard = self.clipboard.clone();
        let sound = self.sound.clone();
        let links = self.links.clone();
        let full_screen = self.full_screen.clone();
        let file_picker = self.file_picker.clone();
        let files_over = self.files_over.clone();
        let appearance = self.appearance.clone();
        let ui_waker = self.ui_waker.clone();
        let view_profile = self.view_profile.clone();
        self.hover_readers.borrow_mut().clear();
        let hover_readers = self.hover_readers.clone();
        let Self { root, text, layout, elements, interaction, observer, hover_chain, local, scroll, .. } =
            self;
        let now = self.time;
        let motion_clock = self.motion.clock;
        let motion_enabled = self.motion.enabled;
        let reduced_motion = self.system_reduced_motion || theme.peek().interaction.reduced_motion;
        self.motion.reduced = reduced_motion;
        let (node, overlays, requests, focus, wake, shortcuts, activity, asked, error, motions, moving) =
            observer.track(|| {
                let mut cx = BuildCx::new(
                    theme,
                    viewport,
                    text,
                    layout,
                    elements,
                    *interaction,
                    hover_chain,
                    scroll,
                    modifiers,
                    local,
                    placeholder,
                    clipboard,
                    ui_waker,
                );
                cx.now = now;
                cx.sound = sound;
                cx.links = links;
                cx.full_screen = full_screen;
                cx.file_picker = file_picker;
                cx.files_over = files_over;
                cx.appearance = appearance;
                cx.motion_clock = motion_clock;
                cx.motion_enabled = motion_enabled;
                cx.reduced_motion = reduced_motion;
                cx.view_profile = view_profile;
                cx.hover_readers = hover_readers;
                let content = cx.child(&**root);
                let style = LayoutStyle {
                    width: Some(viewport.width),
                    height: Some(viewport.height),
                    align: Align::Stretch,
                    overflow_hidden: true,
                    ..LayoutStyle::stack(Axis::Vertical)
                };
                let node = cx.container(style, &[content], Paint::None);
                (
                    node,
                    std::mem::take(&mut cx.overlays),
                    std::mem::take(&mut cx.scroll_requests),
                    cx.focus_request.take(),
                    cx.wake.take(),
                    std::mem::take(&mut cx.shortcuts),
                    cx.activity.take(),
                    cx.access_tree_asked,
                    cx.error.take(),
                    std::mem::take(&mut cx.motions),
                    cx.value_moving,
                )
            });
        self.wake = wake;
        self.value_moving = moving;
        self.shortcuts = shortcuts;
        self.activity = activity;
        self.access_tree_asked = asked;
        if let Some(widget) = focus
            && self.interaction.focused != Some(widget)
        {
            // Focus moved by code keeps its ring only while the user is on the
            // keyboard (arrow keys moving through a radio group, say).
            self.interaction.focus_visible &= self.interaction.focused.is_some();
            self.moved_focus_from = Some(self.interaction.focused);
            self.interaction.focused = Some(widget);
            self.reveal_focus = true;
            // Rebuild once so the newly focused widget can draw its focus state.
            self.ask_build(BuildCause::Focus);
        }
        for (widget, offset) in requests {
            if self.scroll.offsets.insert(widget, offset) != Some(offset) {
                // Virtualized content depends on the offset: build again at the new position.
                self.ask_build(BuildCause::Scroll);
            }
        }
        self.local.end_build();
        if let Some(e) = error {
            return Err(e);
        }
        self.root_node = Some(node);
        let mut overlays = overlays;
        overlays.sort_by_key(|overlay| !overlay.floating);
        self.overlays = overlays;
        self.motions = motions;
        let palette = self.theme.peek().palette;
        self.crossfade = self.motion.enabled && self.painted_palette.is_some_and(|p| p != palette);
        let elements = &self.elements;
        let paints = |n: NodeId| {
            let e = &elements[n.0 as usize];
            NodePaint::of(&e.paint, e.text_color)
        };
        self.motion.sync(&self.motions, paints, self.crossfade);
        self.parents.clear();
        self.parents.resize(self.elements.len(), None);
        for (i, e) in self.elements.iter().enumerate() {
            for c in &e.children {
                self.parents[c.0 as usize] = Some(NodeId(i as u32));
            }
        }
        // Hovering a floating layer counts as hovering its anchor (hover cards stay open).
        for o in &self.overlays {
            if o.placement != Placement::Fill {
                self.parents[o.root.0 as usize] = Some(o.anchor);
            }
        }
        self.trap_focus();
        Ok(())
    }

    fn compute_layout(&mut self) -> Result<(), ravenrs_layout::LayoutError> {
        let _span = tracing::trace_span!("ravenrs::layout").entered();
        self.stats.layouts += 1;
        let started = self.phase_start();
        let result = self.solve_layout();
        self.phases.layout += self.phase_elapsed(started);
        result
    }

    fn solve_layout(&mut self) -> Result<(), ravenrs_layout::LayoutError> {
        let Some(root) = self.root_node else { return Ok(()) };
        let roots = std::iter::once(root).chain(self.overlays.iter().map(|o| o.root)).collect::<Vec<_>>();
        for node in roots {
            let layout_id = self.elements[node.0 as usize].layout;
            self.layout.compute(layout_id, self.viewport, |leaf, input| {
                let limit = input.known_width.or(input.available_width.limit());
                // Truncated text never wraps: it takes what it needs, up to the limit.
                let one_line = leaf.truncate.is_some();
                let size = leaf.layout.measure(if one_line { None } else { limit });
                let width = match limit {
                    Some(limit) if one_line => size.width.ceil().min(limit),
                    _ => size.width.ceil(),
                };
                Size::new(input.known_width.unwrap_or(width), input.known_height.unwrap_or(size.height))
            })?;
        }
        self.content.clear();
        self.content.resize(self.elements.len(), Size::ZERO);
        for (i, e) in self.elements.iter().enumerate() {
            if e.scroll {
                self.content[i] = self.layout.content_size(e.layout)?;
            }
            if e.measure {
                let content = self.layout.content_size(e.layout)?;
                let before = self.scroll.contents.insert(e.widget, content);
                // What is built from the room inside a scroll has to see that room change.
                self.viewport_moved |= e.rebuild_on_scroll && before != Some(content);
            }
        }
        Ok(())
    }

    /// Positions every element (scroll offsets and overlays included) and paints.
    fn place_and_paint(&mut self) {
        if self.root_node.is_none() {
            return;
        }
        self.place_all();
        if self.scroll_focus_into_view() {
            self.place_all();
        }
        self.tell_appeared();
        let snap = std::mem::take(&mut self.snap_layout);
        for entry in &self.motions {
            if let Some(t) = entry.motion.layout {
                let i = entry.node.0 as usize;
                let unscrolled = self.frames[i].translate(Point::ZERO - self.shifts[i]);
                self.motion.sync_frame(entry.key, self.elements[i].widget, unscrolled, t, snap);
            }
        }
        self.paint();
    }

    /// Gives every element its absolute frame, scroll offsets and overlays included.
    fn place_all(&mut self) {
        let Some(root) = self.root_node else { return };
        self.stats.placements += 1;
        let started = self.phase_start();
        self.frames.clear();
        self.frames.resize(self.elements.len(), Rect::ZERO);
        self.shifts.clear();
        self.shifts.resize(self.elements.len(), Point::ZERO);
        self.views.clear();
        self.views.resize(self.elements.len(), Rect::from_origin_size(Point::ZERO, self.viewport));
        self.place(root, Point::ZERO);
        let overlays = std::mem::take(&mut self.overlays);
        for overlay in &overlays {
            let size = self.layout.frame(self.elements[overlay.root.0 as usize].layout).map(|f| f.size);
            let size = size.unwrap_or(Size::ZERO);
            let anchor = self.frames.get(overlay.anchor.0 as usize).copied().unwrap_or(Rect::ZERO);
            let origin = overlay_origin(overlay.placement, anchor, size, overlay.gap, self.viewport);
            self.place(overlay.root, origin);
        }
        self.overlays = overlays;
        self.phases.place += self.phase_elapsed(started);
    }

    /// If focus just moved, scrolls each container around the focused element,
    /// innermost first, just far enough to show it with its focus ring. Returns
    /// true if something scrolled, which makes the frames stale.
    fn scroll_focus_into_view(&mut self) -> bool {
        if !std::mem::take(&mut self.reveal_focus) {
            return false;
        }
        let focused = self.interaction.focused;
        let is_focused = |e: &Element| Some(e.widget) == focused && e.focus_radius.is_some();
        let Some(start) = self.elements.iter().position(is_focused) else { return false };
        let ix = self.theme().interaction;
        let mut target = self.frames[start].inflate(ix.focus_ring_offset + ix.focus_ring_width);
        let mut scrolled = false;
        let mut node = NodeId(start as u32);
        // A floating layer's parent is its anchor, but moving the page behind it shows nothing.
        while !self.overlays.iter().any(|o| o.root == node) {
            let Some(parent) = self.parents.get(node.0 as usize).copied().flatten() else { break };
            node = parent;
            let i = node.0 as usize;
            if !self.elements[i].scroll {
                continue;
            }
            let (frame, max, offset) = (self.frames[i], self.max_offset(i), self.clamped_offset(i));
            let next = Point::new(
                if self.elements[i].scroll_x {
                    reveal_offset(offset.x, (target.x(), target.max_x()), (frame.x(), frame.max_x()), max.x)
                } else {
                    offset.x
                },
                reveal_offset(offset.y, (target.y(), target.max_y()), (frame.y(), frame.max_y()), max.y),
            );
            if next != offset {
                self.scroll.offsets.insert(self.elements[i].widget, next);
                // Virtualized content depends on the offset: build again at the new position.
                if self.elements[i].rebuild_on_scroll {
                    self.ask_build(BuildCause::Scroll);
                }
                target = target.translate(offset - next);
                scrolled = true;
            }
        }
        scrolled
    }

    /// Sets absolute frames for `node` and its subtree; `origin` is where the
    /// node's own frame starts (its layout location is ignored for roots).
    fn place(&mut self, node: NodeId, origin: Point) {
        let window = Rect::from_origin_size(Point::ZERO, self.viewport);
        let mut stack = vec![(node, origin, true, Point::ZERO, window, window)];
        while let Some((node, parent_origin, is_root, mut shift, parent_frame, view)) = stack.pop() {
            let i = node.0 as usize;
            let Ok(local) = self.layout.frame(self.elements[i].layout) else { continue };
            let mut frame = if is_root {
                Rect::from_origin_size(parent_origin, local.size)
            } else {
                local.translate(parent_origin)
            };
            // Sticky: held below the top of the view, and taken away by the parent's bottom.
            if let Some(top) = self.elements[i].pin.filter(|_| !is_root) {
                let lowest = (parent_frame.max_y() - frame.height()).max(frame.y());
                let y = frame.y().max(view.y() + top).min(lowest);
                let moved = Point::new(0.0, y - frame.y());
                frame = frame.translate(moved);
                shift = shift + moved;
            }
            self.frames[i] = frame;
            self.shifts[i] = shift;
            self.views[i] = view;
            let mut child_origin = frame.origin + self.elements[i].translate;
            let mut child_shift = shift;
            let mut child_view = view;
            if self.elements[i].scroll {
                child_view = frame;
                let offset = self.clamped_offset(i);
                child_origin = child_origin - offset;
                child_shift = child_shift - offset;
                let widget = self.elements[i].widget;
                let before = self.scroll.viewports.insert(widget, frame.size);
                self.viewport_moved |= self.elements[i].rebuild_on_scroll && before != Some(frame.size);
                // Store the clamped value so builds see where the content really is.
                self.scroll.offsets.insert(widget, offset);
            } else if self.elements[i].report_frame {
                let before = self.scroll.viewports.insert(self.elements[i].widget, frame.size);
                self.viewport_moved |= before != Some(frame.size);
            }
            let element = &self.elements[i];
            for child in &element.children {
                stack.push((*child, child_origin, false, child_shift, frame, child_view));
            }
        }
    }

    fn paint(&mut self) {
        let _span = tracing::trace_span!("ravenrs::paint").entered();
        self.stats.paints += 1;
        self.frame.paints += 1;
        let started = self.phase_start();
        self.paint_scene();
        self.phases.paint += self.phase_elapsed(started);
    }

    fn paint_scene(&mut self) {
        let theme = self.theme();
        self.scrollbar_showing = false;
        let viewport = Rect::from_origin_size(Point::ZERO, self.viewport);
        if self.crossfade {
            let mut old = Vec::with_capacity(self.scene.len() + 1);
            old.push(Primitive::Quad(Quad::new(viewport, self.scene.clear_color())));
            old.extend_from_slice(self.scene.primitives());
            self.motion.crossfade(old, viewport, theme.motion.base);
            self.crossfade = false;
        }
        self.painted_palette = Some(theme.palette);
        self.scene.clear();
        self.scene.set_clear_color(theme.color(Semantic::Canvas));
        self.paint_order.clear();
        self.clips.clear();
        self.clips.resize(self.elements.len(), Rect::ZERO);
        self.motion.active = false;
        self.has_live = false;
        self.painting_overlays = false;
        if let Some(root) = self.root_node {
            self.paint_node(root, viewport, &theme);
        }
        self.motion.paint_ghosts(GhostLayer::Main, &mut self.scene);
        self.painting_overlays = true;
        for i in 0..self.overlays.len() {
            let root = self.overlays[i].root;
            self.paint_node(root, viewport, &theme);
        }
        self.motion.paint_ghosts(GhostLayer::Overlay, &mut self.scene);
        self.motion.paint_ghosts(GhostLayer::Top, &mut self.scene);
        let live = self.has_live && !self.reduced_motion();
        self.next_live = (live || self.scrollbar_showing).then_some(self.time + LIVE_FRAME);
        self.tell_activity(crate::context::UiActivity::Painted(&self.scene));
        if std::mem::take(&mut self.access_tree_asked) {
            let tree = self.access_tree();
            self.tell_activity(crate::context::UiActivity::Described(&tree));
        }
    }

    /// How far the scroll is along `range` for element `i`, from 0 to 1.
    fn progress_of(&self, i: usize, range: crate::ScrollRange) -> f32 {
        let mut scene = None;
        if range == crate::ScrollRange::Scene {
            let mut node = Some(NodeId(i as u32));
            while let Some(n) = node {
                let k = n.0 as usize;
                if let Some(track) = self.elements[k].scroll_track {
                    scene = Some((k, track));
                    break;
                }
                node = self.parents.get(k).copied().flatten();
            }
        }
        match scene {
            Some((k, track)) => range.progress(self.frames[i], self.views[k], Some((self.frames[k], track))),
            None => range.progress(self.frames[i], self.views[i], None),
        }
    }

    /// The effect element `i`'s scroll link gives at the current scroll position.
    fn scroll_effect(&self, i: usize) -> Effect {
        let Some(link) = &self.elements[i].scroll_link else { return Effect::IDENTITY };
        let effect = link.effect_at(self.progress_of(i, link.scroll_range()));
        // Asked for less motion, the scroll still fades things, but nothing moves or grows.
        if self.reduced_motion() { Effect::IDENTITY.with_opacity(effect.opacity) } else { effect }
    }

    fn reduced_motion(&self) -> bool {
        self.system_reduced_motion || self.theme.peek().interaction.reduced_motion
    }

    /// The node's own shape, at `frame`.
    fn paint_shape(&mut self, i: usize, frame: Rect, quad: Option<QuadPaint>) {
        match &self.elements[i].paint {
            Paint::None => {}
            Paint::Quad(q) => {
                let q = quad.unwrap_or(*q);
                let mut quad = Quad::new(frame, q.fill).radius(q.corner_radius);
                quad.border = q.border;
                quad.shadow = q.shadow;
                self.scene.push_quad(quad);
            }
            Paint::Custom(draw) => {
                let draw = draw.clone();
                draw(frame, &mut self.scene);
            }
            Paint::Live(draw) => {
                let draw = draw.clone();
                let time = if self.reduced_motion() { 0.0 } else { self.time };
                self.has_live = true;
                draw(frame, time, &mut self.scene);
            }
            Paint::Rich(draw) => {
                let draw = draw.clone();
                let mut cx = PaintCx { scene: &mut self.scene, text: &mut self.text, scale: self.scale };
                draw(frame, &mut cx);
            }
        }
    }

    /// The motion values of element `i` for this frame, if it has a motion.
    fn sample_motion(&mut self, i: usize) -> Option<(MotionEntry, Sampled)> {
        let index = self.elements[i].motion?;
        let entry = *self.motions.get(index as usize)?;
        let e = &self.elements[i];
        let paint = NodePaint::of(&e.paint, e.text_color);
        Some((entry, self.motion.sample(entry.key, &paint, self.shifts[i])))
    }

    /// For an overlay root, its anchor's frame.
    fn anchor_of(&self, node: NodeId) -> Option<Rect> {
        let overlay = self.overlays.iter().find(|o| o.root == node)?;
        self.frames.get(overlay.anchor.0 as usize).copied()
    }

    fn paint_node(&mut self, node: NodeId, clip: Rect, theme: &Theme) {
        let i = node.0 as usize;
        self.paint_order.push(node);
        self.clips[i] = clip;
        let frame = self.frames[i];
        let start = self.scene.len();
        let sampled = self.sample_motion(i);
        let motion = sampled.as_ref().map(|(entry, _)| *entry);
        let (quad, text_color, visual, mut effect) = match sampled {
            Some((_, s)) => (s.quad, s.text, s.frame.unwrap_or(frame), s.effect),
            None => (None, None, frame, Effect::IDENTITY),
        };
        if let Some((range, cell)) = self.elements[i].progress_out.clone() {
            cell.set(self.progress_of(i, range));
        }
        let mut linked = self.scroll_effect(i);
        if let Some(link) = self.elements[i].time_link.clone() {
            let time = if self.reduced_motion() { 0.0 } else { self.time };
            self.has_live = true;
            linked = linked.then(link.effect_at(time, frame));
        }
        effect = effect.then(linked);
        // Off screen, its pixels would be thrown away: what it draws goes nowhere, though its
        // children still get their turn, since they can stand outside it. A node under an
        // effect may land elsewhere, one leaving is captured for its way out, and one
        // without a size may draw around itself.
        let shown = effect != Effect::IDENTITY
            || motion.is_some_and(|m| m.motion.exit.is_some())
            || frame.is_empty()
            || touches(visual, clip);
        let underlaid = motion.is_some_and(|m| m.motion.underlay);
        if shown && !underlaid {
            self.paint_shape(i, visual, quad);
        }
        let exit = motion.filter(|m| m.motion.exit.is_some());
        if exit.is_some() {
            self.motion.begin_capture();
        }
        // Everything from here on travels with the node.
        let content_start = self.scene.len();
        if let Some(color) = self.elements[i].text_color.filter(|_| shown) {
            let color = text_color.unwrap_or(color);
            let layout_id = self.elements[i].layout;
            if let Some(leaf) = self.layout.context_mut(layout_id) {
                if let Some((text, attrs)) = &leaf.truncate {
                    let fitted = self.text.layout_ellipsized(text, attrs, frame.width() + 0.5);
                    fitted.paint(frame.origin, color, &mut self.scene);
                } else {
                    // Re-break at the final width; the half pixel absorbs rounding.
                    leaf.layout.measure(Some(frame.width() + 0.5));
                    leaf.layout.paint(frame.origin, color, &mut self.scene);
                }
            }
        }
        let clips_children = self.elements[i].clip;
        let child_clip = if clips_children { clip.intersect(&frame) } else { clip };
        let pushes_clip = clips_children && shown;
        if pushes_clip {
            self.scene.push_clip(frame);
        }
        let children = self.elements[i].children.clone();
        for &child in &children {
            if let Some(m) = self.elements[child.0 as usize].motion
                && self.motions.get(m as usize).is_some_and(|e| e.motion.underlay)
            {
                let c = child.0 as usize;
                if let Some((_, s)) = self.sample_motion(c) {
                    self.paint_shape(c, s.frame.unwrap_or(self.frames[c]), s.quad);
                }
            }
        }
        for child in children {
            self.paint_node(child, child_clip, theme);
        }
        let outside = self.elements[i].scrollbar_outside;
        if shown && self.elements[i].scroll && outside == 0.0 {
            self.paint_scrollbar(i, theme, 0.0);
        }
        if pushes_clip {
            self.scene.pop_clip();
        }
        if shown && self.elements[i].scroll && outside != 0.0 {
            self.paint_scrollbar(i, theme, outside);
        }
        let element = &self.elements[i];
        if let Some(radius) = element.focus_radius
            && shown
            && self.interaction.focus_visible
            && self.interaction.focused == Some(element.widget)
        {
            let ix = theme.interaction;
            let grow = ix.focus_ring_offset + ix.focus_ring_width;
            let ring = Quad::new(frame.inflate(grow), Color::TRANSPARENT)
                .radius(radius + grow)
                .border(ix.focus_ring_width, theme.color(Semantic::FocusRing));
            self.scene.push_quad(ring);
        }
        let Some(entry) = motion else {
            self.scene.apply_effect(start..self.scene.len(), linked, frame.center());
            return;
        };
        let end = self.scene.len();
        if !entry.motion.shape_only && visual.origin != frame.origin {
            self.scene.apply_effect(
                content_start..end,
                {
                    let d = visual.origin - frame.origin;
                    Effect::moved(d.x, d.y)
                },
                Point::ZERO,
            );
        }
        let mut pivot = visual.center();
        let mut flip = false;
        if entry.motion.anchored
            && let Some(anchor) = self.anchor_of(node)
        {
            let target = anchor.center();
            pivot = Point::new(
                target.x.clamp(visual.x(), visual.max_x()),
                target.y.clamp(visual.y(), visual.max_y()),
            );
            flip = visual.center().y < target.y;
            effect = flipped(effect, flip);
        }
        if exit.is_some() {
            let layer = if self.painting_overlays { GhostLayer::Overlay } else { GhostLayer::Main };
            let primitives = &self.scene.primitives()[start..end];
            self.motion.end_capture(entry.key, primitives, start..end, clip, pivot, flip, layer);
        }
        self.scene.apply_effect(start..end, effect, pivot);
    }

    /// Draws the scrollbar around a scroll, and lets it fade. `outside` puts it that far
    /// past the container's edge, in the room around what it scrolls.
    fn paint_scrollbar(&mut self, i: usize, theme: &Theme, outside: f32) {
        let frame = self.frames[i];
        let (content, max) = (self.content[i], self.max_offset(i));
        let offset = self.clamped_offset(i);
        let Some(&moved) = self.scrolled_at.get(&self.elements[i].widget) else { return };
        let age = self.time - moved;
        if age >= SCROLLBAR_HOLD + SCROLLBAR_FADE {
            return;
        }
        self.scrollbar_showing = true;
        let fading = ((age - SCROLLBAR_HOLD) / SCROLLBAR_FADE).clamp(0.0, 1.0) as f32;
        let alpha = if self.reduced_motion() { SCROLLBAR_ALPHA } else { SCROLLBAR_ALPHA * (1.0 - fading) };
        let color = theme.color(Semantic::InkMuted).with_alpha(alpha);
        if max.y > 0.5 && frame.height() > 0.0 {
            let track = frame.height() - 8.0;
            let thumb = (track * frame.height() / content.height).max(24.0).min(track);
            let y = frame.y() + 4.0 + (track - thumb) * (offset.y / max.y);
            let rect = Rect::new(frame.max_x() - SCROLLBAR_WIDTH - 3.0 + outside, y, SCROLLBAR_WIDTH, thumb);
            self.scene.push_quad(Quad::new(rect, color).radius(SCROLLBAR_WIDTH / 2.0));
        }
        if self.elements[i].scroll_x && max.x > 0.5 && frame.width() > 0.0 {
            let track = frame.width() - 8.0;
            let thumb = (track * frame.width() / content.width).max(24.0).min(track);
            let x = frame.x() + 4.0 + (track - thumb) * (offset.x / max.x);
            let rect = Rect::new(x, frame.max_y() - SCROLLBAR_WIDTH - 3.0, thumb, SCROLLBAR_WIDTH);
            self.scene.push_quad(Quad::new(rect, color).radius(SCROLLBAR_WIDTH / 2.0));
        }
    }
}

/// Top-left corner of an overlay of `size`, kept inside the viewport.
/// The scroll offset along one axis that shows `target` inside `view`, moving
/// as little as possible. A target larger than the view shows its start.
fn reveal_offset(offset: f32, target: (f32, f32), view: (f32, f32), max: f32) -> f32 {
    let next = if target.0 < view.0 || target.1 - target.0 > view.1 - view.0 {
        offset + target.0 - view.0
    } else if target.1 > view.1 {
        offset + target.1 - view.1
    } else {
        offset
    };
    next.clamp(0.0, max)
}

/// How far a layer in a corner of the window keeps from its edges.
const VIEWPORT_MARGIN: f32 = 16.0;

fn overlay_origin(placement: Placement, anchor: Rect, size: Size, gap: f32, viewport: Size) -> Point {
    let mut p = match placement {
        Placement::Below => Point::new(anchor.x(), anchor.max_y() + gap),
        Placement::Above => Point::new(anchor.x(), anchor.y() - gap - size.height),
        Placement::Right => Point::new(anchor.max_x() + gap, anchor.y()),
        Placement::AboveCenter => {
            Point::new(anchor.center().x - size.width / 2.0, anchor.y() - gap - size.height)
        }
        Placement::RightCenter => Point::new(anchor.max_x() + gap, anchor.center().y - size.height / 2.0),
        Placement::LeftCenter => {
            Point::new(anchor.x() - gap - size.width, anchor.center().y - size.height / 2.0)
        }
        Placement::RightEnd => Point::new(anchor.max_x() + gap, anchor.max_y() - size.height),
        Placement::LeftStart => Point::new(anchor.x() - gap - size.width, anchor.y()),
        Placement::LeftEnd => Point::new(anchor.x() - gap - size.width, anchor.max_y() - size.height),
        Placement::Fill => return Point::ZERO,
        Placement::At(p) => p,
        Placement::BottomRight => Point::new(
            viewport.width - size.width - VIEWPORT_MARGIN,
            viewport.height - size.height - VIEWPORT_MARGIN,
        ),
        Placement::BottomLeft => Point::new(VIEWPORT_MARGIN, viewport.height - size.height - VIEWPORT_MARGIN),
        Placement::BottomCenter => {
            Point::new((viewport.width - size.width) / 2.0, viewport.height - size.height - VIEWPORT_MARGIN)
        }
        Placement::TopLeft => Point::new(VIEWPORT_MARGIN, VIEWPORT_MARGIN),
        Placement::TopCenter => Point::new((viewport.width - size.width) / 2.0, VIEWPORT_MARGIN),
        Placement::TopRight => Point::new(viewport.width - size.width - VIEWPORT_MARGIN, VIEWPORT_MARGIN),
    };
    // A layer at the side changes sides when only the other one has room (a submenu at
    // the window's edge), the same way one below goes above.
    let left = anchor.x() - gap - size.width;
    if placement == Placement::Right && p.x + size.width > viewport.width && left >= 0.0 {
        p.x = left;
    }
    // Flip vertically when there is no room, then clamp.
    if placement == Placement::Below && p.y + size.height > viewport.height {
        p.y = anchor.y() - gap - size.height;
    }
    if matches!(placement, Placement::Above | Placement::AboveCenter) && p.y < 0.0 {
        p.y = anchor.max_y() + gap;
    }
    p.x = p.x.clamp(4.0, (viewport.width - size.width - 4.0).max(4.0));
    p.y = p.y.clamp(4.0, (viewport.height - size.height - 4.0).max(4.0));
    p
}

/// Whether a shortcut's key is the key pressed, whatever the case of a letter.
fn same_key(wanted: &Key, pressed: &Key) -> bool {
    match (wanted, pressed) {
        (Key::Character(a), Key::Character(b)) => a.eq_ignore_ascii_case(b),
        (a, b) => a == b,
    }
}

#[cfg(test)]
mod frame_memory_tests {
    use ravenrs_core::Event;

    use super::*;
    use crate::{Text, VirtualList};

    #[test]
    fn a_measured_list_does_not_remember_every_row_it_ever_showed() {
        let list = VirtualList::new(5_000, |_| Text::new("row")).height(300.0).key(|i| i as u64);
        let mut ui = Ui::new(list, Theme::raven_dark());
        ui.set_viewport(Size::new(400.0, 400.0), 1.0);
        ui.update();
        ui.update();
        let at_rest = ui.scroll.viewports.len();
        assert!(at_rest > 1, "a measured row reports its frame, or this test proves nothing");
        for _ in 0..100 {
            ui.handle_event(&Event::Scroll {
                position: Point::new(100.0, 100.0),
                delta: Point::new(0.0, -600.0),
            });
            ui.update();
            ui.update();
        }
        assert!(ui.scroll.offsets.values().any(|o| o.y > 10_000.0), "the list did scroll");
        let scrolled = ui.scroll.viewports.len();
        assert!(scrolled <= at_rest + 4, "{scrolled} frames kept, {at_rest} at rest");
    }
}
