use std::cell::{Cell, RefCell};
use std::fmt;
use std::rc::Rc;

use raven_core::{Color, CursorIcon, Event, Key, Modifiers, Point, PointerButton, Quad, Rect, Scene, Size};
use raven_layout::{Align, Axis, LayoutStyle, LayoutTree};
use raven_reactive::{Observer, State};
use raven_style::{Semantic, Theme};
use raven_text::TextSystem;

use crate::View;
use crate::context::{
    BuildCx, Drag, DragPhase, Element, Interaction, LocalStore, NodeId, Overlay, Paint, Placement,
    ScrollState, Shortcut, TextLeaf, WidgetId,
};

type Waker = Rc<RefCell<Option<Rc<dyn Fn()>>>>;

/// Lines scrolled per wheel notch are converted to pixels by the platform; this
/// is the width of the overlay scrollbar.
const SCROLLBAR_WIDTH: f32 = 4.0;

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
    waker: Waker,
    needs_build: bool,
    viewport: Size,
    scale: f32,
    interaction: Interaction,
    dragging: Option<WidgetId>,
    /// Seconds since start, set by the platform.
    time: f64,
    shortcuts: Vec<Shortcut>,
    /// Earliest time a widget asked to be rebuilt at.
    wake: Option<f64>,
    pointer: Option<Point>,
    cursor: CursorIcon,
    scene: Scene,
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
            waker,
            needs_build: true,
            viewport: Size::new(800.0, 600.0),
            scale: 1.0,
            interaction: Interaction::default(),
            dragging: None,
            time: 0.0,
            shortcuts: Vec::new(),
            wake: None,
            pointer: None,
            cursor: CursorIcon::Default,
            scene: Scene::new(),
        }
    }

    /// Called whenever reactive state the UI depends on changes.
    pub fn set_waker(&mut self, waker: impl Fn() + 'static) {
        *self.waker.borrow_mut() = Some(Rc::new(waker));
    }

    /// Sets the viewport in logical pixels and the device scale factor.
    pub fn set_viewport(&mut self, size: Size, scale: f32) {
        if size != self.viewport || scale != self.scale {
            self.viewport = size;
            self.scale = scale;
            self.needs_build = true;
        }
    }

    pub fn viewport(&self) -> Size {
        self.viewport
    }

    pub fn scale(&self) -> f32 {
        self.scale
    }

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
        self.needs_build || self.state_dirty.get() || self.wake.is_some_and(|w| self.time >= w)
    }

    /// Sets the clock (seconds since the app started). Platforms call it before
    /// handling events and before each frame.
    pub fn set_time(&mut self, seconds: f64) {
        self.time = seconds;
    }

    /// When the UI wants its next frame even without input (seconds, same clock
    /// as [`Ui::set_time`]). Platforms schedule a timer for it.
    pub fn next_wake(&self) -> Option<f64> {
        self.wake
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
        if self.needs_redraw() {
            self.rebuild();
            // The tree changed under a stationary pointer: refresh hover once.
            if self.refresh_hover() {
                self.rebuild();
            }
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

    /// Feeds one input event; returns true when a redraw is needed.
    pub fn handle_event(&mut self, event: &Event) -> bool {
        match event {
            Event::PointerMove { position } => {
                self.pointer = Some(*position);
                if let Some(widget) = self.dragging {
                    self.drag(widget, DragPhase::Move, *position);
                }
                self.refresh_hover();
            }
            Event::PointerDown { position, button: PointerButton::Primary } => {
                self.pointer = Some(*position);
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
        for overlay in &self.overlays {
            let Some(dismiss) = &overlay.on_dismiss else { continue };
            let inside = |n: NodeId| self.frames.get(n.0 as usize).is_some_and(|f| f.contains(p));
            let in_anchor = overlay.placement != Placement::Fill && inside(overlay.anchor);
            if !inside(overlay.root) && !in_anchor {
                handlers.push(dismiss.clone());
            }
        }
        for h in handlers {
            h();
        }
    }

    /// Scrolls the innermost scroll container under `p` that can move in the
    /// direction of `delta`. Returns true if something scrolled.
    fn scroll(&mut self, p: Point, delta: Point) -> bool {
        for &node in self.paint_order.iter().rev() {
            let i = node.0 as usize;
            let e = &self.elements[i];
            if !e.scroll || !self.frames[i].intersect(&self.clips[i]).contains(p) {
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
        let focusable: Vec<WidgetId> = self
            .paint_order
            .iter()
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
        let _span = tracing::trace_span!("raven::frame").entered();
        self.needs_build = false;
        self.state_dirty.set(false);
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

    fn build(&mut self) -> Result<(), raven_layout::LayoutError> {
        let _span = tracing::trace_span!("raven::build").entered();
        self.layout.clear();
        self.elements.clear();
        self.root_node = None;
        self.overlays.clear();
        let placeholder = self.layout.node(LayoutStyle::leaf(), &[])?;
        let viewport = self.viewport;
        let theme = self.theme.clone();
        self.local.begin_build();
        let modifiers = self.modifiers.clone();
        let Self { root, text, layout, elements, interaction, observer, hover_chain, local, scroll, .. } =
            self;
        let now = self.time;
        let (node, overlays, requests, focus, wake, shortcuts, error) = observer.track(|| {
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
            );
            cx.now = now;
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
                cx.error.take(),
            )
        });
        self.wake = wake;
        self.shortcuts = shortcuts;
        if let Some(widget) = focus
            && self.interaction.focused != Some(widget)
        {
            self.interaction.focused = Some(widget);
            self.interaction.focus_visible = false;
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
        Ok(())
    }

    fn compute_layout(&mut self) -> Result<(), raven_layout::LayoutError> {
        let _span = tracing::trace_span!("raven::layout").entered();
        let Some(root) = self.root_node else { return Ok(()) };
        let roots = std::iter::once(root).chain(self.overlays.iter().map(|o| o.root)).collect::<Vec<_>>();
        for node in roots {
            let layout_id = self.elements[node.0 as usize].layout;
            self.layout.compute(layout_id, self.viewport, |leaf, input| {
                let limit = input.known_width.or(input.available_width.limit());
                let size = leaf.layout.measure(limit);
                Size::new(
                    input.known_width.unwrap_or(size.width.ceil()),
                    input.known_height.unwrap_or(size.height),
                )
            })?;
        }
        self.content.clear();
        self.content.resize(self.elements.len(), Size::ZERO);
        for (i, e) in self.elements.iter().enumerate() {
            if e.scroll {
                self.content[i] = self.layout.content_size(e.layout)?;
            }
        }
        Ok(())
    }

    /// Positions every element (scroll offsets and overlays included) and paints.
    fn place_and_paint(&mut self) {
        let Some(root) = self.root_node else { return };
        self.frames.clear();
        self.frames.resize(self.elements.len(), Rect::ZERO);
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
        self.paint();
    }

    /// Sets absolute frames for `node` and its subtree; `origin` is where the
    /// node's own frame starts (its layout location is ignored for roots).
    fn place(&mut self, node: NodeId, origin: Point) {
        let mut stack = vec![(node, origin, true)];
        while let Some((node, parent_origin, is_root)) = stack.pop() {
            let i = node.0 as usize;
            let Ok(local) = self.layout.frame(self.elements[i].layout) else { continue };
            let frame = if is_root {
                Rect::from_origin_size(parent_origin, local.size)
            } else {
                local.translate(parent_origin)
            };
            self.frames[i] = frame;
            let mut child_origin = frame.origin + self.elements[i].translate;
            if self.elements[i].scroll {
                let offset = self.clamped_offset(i);
                child_origin = child_origin - offset;
                let widget = self.elements[i].widget;
                self.scroll.viewports.insert(widget, frame.size);
                // Store the clamped value so builds see where the content really is.
                self.scroll.offsets.insert(widget, offset);
            }
            let element = &self.elements[i];
            for child in &element.children {
                stack.push((*child, child_origin, false));
            }
        }
    }

    fn paint(&mut self) {
        let _span = tracing::trace_span!("raven::paint").entered();
        self.scene.clear();
        let theme = self.theme();
        self.scene.set_clear_color(theme.color(Semantic::Canvas));
        self.paint_order.clear();
        self.clips.clear();
        self.clips.resize(self.elements.len(), Rect::ZERO);
        let viewport = Rect::from_origin_size(Point::ZERO, self.viewport);
        if let Some(root) = self.root_node {
            self.paint_node(root, viewport, &theme);
        }
        let roots: Vec<NodeId> = self.overlays.iter().map(|o| o.root).collect();
        for root in roots {
            self.paint_node(root, viewport, &theme);
        }
    }

    fn paint_node(&mut self, node: NodeId, clip: Rect, theme: &Theme) {
        let i = node.0 as usize;
        self.paint_order.push(node);
        self.clips[i] = clip;
        let frame = self.frames[i];
        match &self.elements[i].paint {
            Paint::None => {}
            Paint::Quad(q) => {
                let mut quad = Quad::new(frame, q.fill).radius(q.corner_radius);
                quad.border = q.border;
                quad.shadow = q.shadow;
                self.scene.push_quad(quad);
            }
            Paint::Custom(draw) => {
                let draw = draw.clone();
                draw(frame, &mut self.scene);
            }
        }
        if let Some(color) = self.elements[i].text_color {
            let layout_id = self.elements[i].layout;
            if let Some(leaf) = self.layout.context_mut(layout_id) {
                // Re-break at the final width; the half pixel absorbs rounding.
                leaf.layout.measure(Some(frame.width() + 0.5));
                leaf.layout.paint(frame.origin, color, &mut self.scene);
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
fn overlay_origin(placement: Placement, anchor: Rect, size: Size, gap: f32, viewport: Size) -> Point {
    let mut p = match placement {
        Placement::Below => Point::new(anchor.x(), anchor.max_y() + gap),
        Placement::Above => Point::new(anchor.x(), anchor.y() - gap - size.height),
        Placement::Right => Point::new(anchor.max_x() + gap, anchor.y()),
        Placement::AboveCenter => {
            Point::new(anchor.center().x - size.width / 2.0, anchor.y() - gap - size.height)
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
