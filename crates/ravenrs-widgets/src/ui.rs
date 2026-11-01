use std::cell::{Cell, RefCell};
use std::fmt;
use std::rc::Rc;

use ravenrs_core::{
    Color, CursorIcon, Effect, Event, Key, Modifiers, Point, PointerButton, Primitive, Quad, Rect, Scene,
    Size,
};
use ravenrs_layout::{Align, Axis, LayoutStyle, LayoutTree};
use ravenrs_reactive::{Observer, State};
use ravenrs_style::{Semantic, Theme};
use ravenrs_text::TextSystem;

use crate::View;
use crate::context::{
    BuildCx, Drag, DragPhase, Element, Interaction, LocalStore, NodeId, Overlay, Paint, Placement, QuadPaint,
    ScrollState, Shortcut, TextLeaf, WidgetId,
};
use crate::motion::{GhostLayer, MotionEntry, MotionSystem, NodePaint, Sampled, flipped};

mod access;

type Waker = Rc<RefCell<Option<Rc<dyn Fn()>>>>;

/// Lines scrolled per wheel notch are converted to pixels by the platform; this
/// is the width of the overlay scrollbar.
const SCROLLBAR_WIDTH: f32 = 4.0;

/// Longest time between frames that animations treat as having really passed.
const MAX_FRAME_GAP: f64 = 0.25;

/// How much a longer gap counts for: little enough not to skip the animation,
/// enough that a machine this slow still gets through it.
const STALL_STEP: f64 = 0.05;

/// Time between paints of animated drawing, in seconds.
const LIVE_FRAME: f64 = 1.0 / 60.0;

/// The platform-independent UI runtime: owns the root view, reacts to input and
/// state changes, and produces a [`Scene`] for the renderer.
///
/// Platform layers drive it with [`Ui::set_viewport`], [`Ui::handle_event`] and
/// [`Ui::update`], and register a waker with [`Ui::set_waker`] to schedule a
/// frame when state changes outside of event handling (timers, async tasks).
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
    viewport: Size,
    scale: f32,
    interaction: Interaction,
    /// The next placement scrolls the focused element into view (the keyboard
    /// or code moved focus; a click never asks for this).
    reveal_focus: bool,
    dragging: Option<WidgetId>,
    /// Seconds since start, set by the platform.
    time: f64,
    shortcuts: Vec<Shortcut>,
    activity: Option<crate::context::ActivityObserver>,
    /// Earliest time a widget asked to be rebuilt at.
    wake: Option<f64>,
    pointer: Option<Point>,
    cursor: CursorIcon,
    scene: Scene,
    access_map: access::AccessMap,
    clipboard: crate::Clipboard,
    motion: MotionSystem,
    motions: Vec<MotionEntry>,
    /// What scrolling added to each element's frame.
    shifts: Vec<Point>,
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
}

impl fmt::Debug for Ui {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Ui")
            .field("elements", &self.elements.len())
            .field("viewport", &self.viewport)
            .finish_non_exhaustive()
    }
}

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
            viewport: Size::new(800.0, 600.0),
            scale: 1.0,
            interaction: Interaction::default(),
            reveal_focus: false,
            dragging: None,
            time: 0.0,
            shortcuts: Vec::new(),
            activity: None,
            wake: None,
            pointer: None,
            cursor: CursorIcon::Default,
            scene: Scene::new(),
            access_map: access::AccessMap::default(),
            clipboard: crate::Clipboard::default(),
            motion: MotionSystem::default(),
            motions: Vec::new(),
            shifts: Vec::new(),
            painted_palette: None,
            crossfade: false,
            snap_layout: false,
            system_reduced_motion: false,
            next_live: None,
            has_live: false,
            value_moving: false,
            painting_overlays: false,
        }
    }

    /// Installs the system clipboard (platform layers do this); the default is
    /// an in-memory clipboard.
    pub fn set_clipboard(&mut self, provider: impl crate::ClipboardProvider + 'static) {
        self.clipboard = crate::Clipboard::new(provider);
        self.needs_build = true;
    }

    /// The clipboard widgets copy to and paste from.
    pub fn clipboard(&self) -> crate::Clipboard {
        self.clipboard.clone()
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
            self.needs_build = true;
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
            self.needs_build = true;
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
        if self.needs_rebuild() {
            self.rebuild();
            // The tree changed under a stationary pointer: refresh hover once.
            if self.refresh_hover() {
                self.rebuild();
            }
        } else if self.motion.active || self.live_is_due() {
            // Only animations are due: the tree and its layout are still valid.
            let _span = tracing::trace_span!("ravenrs::motion_frame").entered();
            self.paint();
        }
        &self.scene
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
                    self.needs_build = true;
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
                    self.needs_build = true;
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
                    self.needs_build = true;
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
                    self.needs_build = true;
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
                let handler = self.focused_element().and_then(|e| e.on_key.clone());
                if let Some(handler) = handler
                    && handler(key, *modifiers)
                {
                    return self.needs_redraw();
                }
                let command = modifiers.ctrl || modifiers.meta;
                let shortcut = self.shortcuts.iter().find(|s| {
                    s.command == command
                        && match (&s.key, key) {
                            (Key::Character(a), Key::Character(b)) => a.eq_ignore_ascii_case(b),
                            (a, b) => a == b,
                        }
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
                            self.needs_build = true;
                        }
                    }
                    _ => {}
                }
            }
            Event::FocusLost => {
                self.dragging = None;
                if self.interaction.pressed.take().is_some() {
                    self.needs_build = true;
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
        match self.modal_root() {
            Some(root) => {
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
                        self.needs_build = true;
                    }
                }
            }
            None => {
                if let Some(back) = self.modal_return.take() {
                    self.interaction.focused = back;
                    self.needs_build = true;
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
            if !e.scroll || !self.frames[i].intersect(&self.clips[i]).contains(p) {
                continue;
            }
            if !layers.as_ref().is_none_or(|l| l.iter().any(|root| self.is_inside(node, *root))) {
                continue;
            }
            let max = self.max_offset(i);
            let current = self.scroll.offsets.get(&e.widget).copied().unwrap_or(Point::ZERO);
            let next = Point::new(
                if e.scroll_x { (current.x - delta.x).clamp(0.0, max.x) } else { 0.0 },
                (current.y - delta.y).clamp(0.0, max.y),
            );
            if (next.x - current.x).abs() > f32::EPSILON || (next.y - current.y).abs() > f32::EPSILON {
                self.scroll.offsets.insert(e.widget, next);
                if e.rebuild_on_scroll {
                    self.needs_build = true;
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
        self.needs_build = true;
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
            self.interaction.hovered = hovered;
            self.hover_chain = chain;
            self.needs_build = true;
            return true;
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
    }

    fn build(&mut self) -> Result<(), ravenrs_layout::LayoutError> {
        let _span = tracing::trace_span!("ravenrs::build").entered();
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
        let ui_waker = self.ui_waker.clone();
        let Self { root, text, layout, elements, interaction, observer, hover_chain, local, scroll, .. } =
            self;
        let now = self.time;
        let motion_clock = self.motion.clock;
        let motion_enabled = self.motion.enabled;
        let reduced_motion = self.system_reduced_motion || theme.peek().interaction.reduced_motion;
        self.motion.reduced = reduced_motion;
        let (node, overlays, requests, focus, wake, shortcuts, activity, error, motions, moving) = observer
            .track(|| {
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
                cx.motion_clock = motion_clock;
                cx.motion_enabled = motion_enabled;
                cx.reduced_motion = reduced_motion;
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
                    cx.error.take(),
                    std::mem::take(&mut cx.motions),
                    cx.value_moving,
                )
            });
        self.wake = wake;
        self.value_moving = moving;
        self.shortcuts = shortcuts;
        self.activity = activity;
        if let Some(widget) = focus
            && self.interaction.focused != Some(widget)
        {
            // Focus moved by code keeps its ring only while the user is on the
            // keyboard (arrow keys moving through a radio group, say).
            self.interaction.focus_visible &= self.interaction.focused.is_some();
            self.interaction.focused = Some(widget);
            self.reveal_focus = true;
            // Rebuild once so the newly focused widget can draw its focus state.
            self.needs_build = true;
        }
        for (widget, offset) in requests {
            if self.scroll.offsets.insert(widget, offset) != Some(offset) {
                // Virtualized content depends on the offset: build again at the new position.
                self.needs_build = true;
            }
        }
        self.local.end_build();
        if let Some(e) = error {
            return Err(e);
        }
        self.root_node = Some(node);
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
                self.scroll.contents.insert(e.widget, self.layout.content_size(e.layout)?);
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
        let snap = std::mem::take(&mut self.snap_layout);
        for entry in &self.motions {
            if let Some(t) = entry.motion.layout {
                let i = entry.node.0 as usize;
                let unscrolled = self.frames[i].translate(Point::ZERO - self.shifts[i]);
                self.motion.sync_frame(entry.key, unscrolled, t, snap);
            }
        }
        self.paint();
    }

    /// Gives every element its absolute frame, scroll offsets and overlays included.
    fn place_all(&mut self) {
        let Some(root) = self.root_node else { return };
        self.frames.clear();
        self.frames.resize(self.elements.len(), Rect::ZERO);
        self.shifts.clear();
        self.shifts.resize(self.elements.len(), Point::ZERO);
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
                self.needs_build |= self.elements[i].rebuild_on_scroll;
                target = target.translate(offset - next);
                scrolled = true;
            }
        }
        scrolled
    }

    /// Sets absolute frames for `node` and its subtree; `origin` is where the
    /// node's own frame starts (its layout location is ignored for roots).
    fn place(&mut self, node: NodeId, origin: Point) {
        let mut stack = vec![(node, origin, true, Point::ZERO)];
        while let Some((node, parent_origin, is_root, shift)) = stack.pop() {
            let i = node.0 as usize;
            let Ok(local) = self.layout.frame(self.elements[i].layout) else { continue };
            let frame = if is_root {
                Rect::from_origin_size(parent_origin, local.size)
            } else {
                local.translate(parent_origin)
            };
            self.frames[i] = frame;
            self.shifts[i] = shift;
            let mut child_origin = frame.origin + self.elements[i].translate;
            let mut child_shift = shift;
            if self.elements[i].scroll {
                let offset = self.clamped_offset(i);
                child_origin = child_origin - offset;
                child_shift = child_shift - offset;
                let widget = self.elements[i].widget;
                self.scroll.viewports.insert(widget, frame.size);
                // Store the clamped value so builds see where the content really is.
                self.scroll.offsets.insert(widget, offset);
            }
            let element = &self.elements[i];
            for child in &element.children {
                stack.push((*child, child_origin, false, child_shift));
            }
        }
    }

    fn paint(&mut self) {
        let _span = tracing::trace_span!("ravenrs::paint").entered();
        let theme = self.theme();
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
        self.next_live = live.then_some(self.time + LIVE_FRAME);
        self.tell_activity(crate::context::UiActivity::Painted(&self.scene));
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
        let underlaid = motion.is_some_and(|m| m.motion.underlay);
        if !underlaid {
            self.paint_shape(i, visual, quad);
        }
        let exit = motion.filter(|m| m.motion.exit.is_some());
        if exit.is_some() {
            self.motion.begin_capture();
        }
        // Everything from here on travels with the node.
        let content_start = self.scene.len();
        if let Some(color) = self.elements[i].text_color {
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
        let child_clip = if clips_children {
            self.scene.push_clip(frame);
            clip.intersect(&frame)
        } else {
            clip
        };
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
        if self.elements[i].scroll {
            self.paint_scrollbar(i, theme);
        }
        if clips_children {
            self.scene.pop_clip();
        }
        let element = &self.elements[i];
        if let Some(radius) = element.focus_radius
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
        let Some(entry) = motion else { return };
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

    fn paint_scrollbar(&mut self, i: usize, theme: &Theme) {
        let frame = self.frames[i];
        let (content, max) = (self.content[i], self.max_offset(i));
        let offset = self.clamped_offset(i);
        let color = theme.color(Semantic::InkMuted).with_alpha(0.45);
        if max.y > 0.5 && frame.height() > 0.0 {
            let track = frame.height() - 8.0;
            let thumb = (track * frame.height() / content.height).max(24.0).min(track);
            let y = frame.y() + 4.0 + (track - thumb) * (offset.y / max.y);
            let rect = Rect::new(frame.max_x() - SCROLLBAR_WIDTH - 3.0, y, SCROLLBAR_WIDTH, thumb);
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
        Placement::Fill => return Point::ZERO,
        Placement::At(p) => p,
        Placement::BottomRight => {
            Point::new(viewport.width - size.width - 16.0, viewport.height - size.height - 16.0)
        }
    };
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
