use std::cell::{Cell, RefCell};
use std::fmt;
use std::rc::Rc;

use raven_core::{Color, CursorIcon, Event, Key, Point, PointerButton, Quad, Rect, Scene, Size};
use raven_layout::{Align, Axis, LayoutStyle, LayoutTree};
use raven_reactive::Observer;
use raven_style::{Semantic, Theme};
use raven_text::TextSystem;

use crate::View;
use crate::context::{BuildCx, Element, Interaction, NodeId, Paint, TextLeaf, WidgetId};

type Waker = Rc<RefCell<Option<Rc<dyn Fn()>>>>;

/// The platform-independent UI runtime: owns the root view, reacts to input and
/// state changes, and produces a [`Scene`] for the renderer.
///
/// Platform layers drive it with [`Ui::set_viewport`], [`Ui::handle_event`] and
/// [`Ui::update`], and register a waker with [`Ui::set_waker`] to schedule a
/// frame when state changes outside of event handling (timers, async tasks).
pub struct Ui {
    root: Box<dyn View>,
    theme: Theme,
    text: TextSystem,
    layout: LayoutTree<TextLeaf>,
    elements: Vec<Element>,
    root_node: Option<NodeId>,
    /// Absolute frames, indexed like `elements`.
    frames: Vec<Rect>,
    /// Elements in painting (back-to-front) order; hit testing walks it backwards.
    paint_order: Vec<NodeId>,
    observer: Observer,
    state_dirty: Rc<Cell<bool>>,
    waker: Waker,
    needs_build: bool,
    viewport: Size,
    scale: f32,
    interaction: Interaction,
    pointer: Option<Point>,
    cursor: CursorIcon,
    scene: Scene,
}

impl fmt::Debug for Ui {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Ui")
            .field("theme", &self.theme.name)
            .field("elements", &self.elements.len())
            .field("viewport", &self.viewport)
            .finish_non_exhaustive()
    }
}

impl Ui {
    pub fn new(root: impl View, theme: Theme) -> Self {
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
            theme,
            text: TextSystem::new(),
            layout: LayoutTree::new(),
            elements: Vec::new(),
            root_node: None,
            frames: Vec::new(),
            paint_order: Vec::new(),
            observer,
            state_dirty,
            waker,
            needs_build: true,
            viewport: Size::new(800.0, 600.0),
            scale: 1.0,
            interaction: Interaction::default(),
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

    pub fn theme(&self) -> &Theme {
        &self.theme
    }

    pub fn set_theme(&mut self, theme: Theme) {
        self.theme = theme;
        self.needs_build = true;
    }

    /// Fonts registered here become available to every `Text`.
    pub fn text_system(&mut self) -> &mut TextSystem {
        &mut self.text
    }

    /// True when the next [`Ui::update`] would produce a different scene.
    pub fn needs_redraw(&self) -> bool {
        self.needs_build || self.state_dirty.get()
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
                self.refresh_hover();
            }
            Event::PointerDown { position, button: PointerButton::Primary } => {
                self.pointer = Some(*position);
                self.refresh_hover();
                let target = self.hit_test(*position);
                let widget = target.map(|n| self.elements[n.0 as usize].widget);
                if self.interaction.pressed != widget {
                    self.interaction.pressed = widget;
                    self.needs_build = true;
                }
                let focus = target.filter(|n| self.elements[n.0 as usize].focus_radius.is_some());
                let focused = focus.map(|n| self.elements[n.0 as usize].widget);
                if self.interaction.focused != focused || self.interaction.focus_visible {
                    self.interaction.focused = focused;
                    self.interaction.focus_visible = false;
                    self.needs_build = true;
                }
            }
            Event::PointerUp { position, button: PointerButton::Primary } => {
                let pressed = self.interaction.pressed.take();
                if pressed.is_some() {
                    self.needs_build = true;
                }
                let target = self.hit_test(*position);
                if let Some(node) = target {
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
                if self.interaction.pressed.take().is_some() {
                    self.needs_build = true;
                }
            }
            Event::KeyDown { key: Key::Tab, modifiers } => self.move_focus(!modifiers.shift),
            Event::KeyDown { key: Key::Enter | Key::Space, .. } => {
                let handler = self
                    .interaction
                    .focused
                    .and_then(|w| self.elements.iter().find(|e| e.widget == w && e.focus_radius.is_some()))
                    .and_then(|e| e.on_click.clone());
                if let Some(handler) = handler {
                    handler();
                }
            }
            Event::KeyDown { key: Key::Escape, .. } if self.interaction.focused.is_some() => {
                self.interaction.focused = None;
                self.needs_build = true;
            }
            Event::FocusLost if self.interaction.pressed.take().is_some() => self.needs_build = true,
            _ => {}
        }
        self.needs_redraw()
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
        if hovered != self.interaction.hovered {
            self.interaction.hovered = hovered;
            self.needs_build = true;
            return true;
        }
        false
    }

    fn hit_test(&self, p: Point) -> Option<NodeId> {
        self.paint_order.iter().rev().copied().find(|n| {
            let i = n.0 as usize;
            self.elements[i].is_interactive() && self.frames.get(i).is_some_and(|f| f.contains(p))
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
        self.paint();
    }

    fn build(&mut self) -> Result<(), raven_layout::LayoutError> {
        let _span = tracing::trace_span!("raven::build").entered();
        self.layout.clear();
        self.elements.clear();
        self.root_node = None;
        let placeholder = self.layout.node(LayoutStyle::leaf(), &[])?;
        let viewport = self.viewport;
        let Self { root, theme, text, layout, elements, interaction, observer, .. } = self;
        let (node, error) = observer.track(|| {
            let mut cx = BuildCx::new(theme, text, layout, elements, *interaction, placeholder);
            let content = cx.child(&**root);
            let style = LayoutStyle {
                width: Some(viewport.width),
                height: Some(viewport.height),
                align: Align::Stretch,
                ..LayoutStyle::stack(Axis::Vertical)
            };
            let node = cx.container(style, &[content], Paint::None);
            (node, cx.error.take())
        });
        if let Some(e) = error {
            return Err(e);
        }
        self.root_node = Some(node);
        Ok(())
    }

    fn compute_layout(&mut self) -> Result<(), raven_layout::LayoutError> {
        let _span = tracing::trace_span!("raven::layout").entered();
        let Some(root) = self.root_node else { return Ok(()) };
        let root_layout = self.elements[root.0 as usize].layout;
        self.layout.compute(root_layout, self.viewport, |leaf, input| {
            let limit = input.known_width.or(input.available_width.limit());
            let size = leaf.layout.measure(limit);
            Size::new(
                input.known_width.unwrap_or(size.width.ceil()),
                input.known_height.unwrap_or(size.height),
            )
        })?;

        self.frames.clear();
        self.frames.resize(self.elements.len(), Rect::ZERO);
        let mut stack = vec![(root, Point::ZERO)];
        while let Some((node, parent_origin)) = stack.pop() {
            let element = &self.elements[node.0 as usize];
            let frame = self.layout.frame(element.layout)?.translate(parent_origin);
            self.frames[node.0 as usize] = frame;
            for child in &element.children {
                stack.push((*child, frame.origin));
            }
        }
        Ok(())
    }

    fn paint(&mut self) {
        let _span = tracing::trace_span!("raven::paint").entered();
        self.scene.clear();
        self.scene.set_clear_color(self.theme.color(Semantic::Canvas));
        self.paint_order.clear();
        if let Some(root) = self.root_node {
            self.paint_node(root);
        }
    }

    fn paint_node(&mut self, node: NodeId) {
        let i = node.0 as usize;
        self.paint_order.push(node);
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
        let children = self.elements[i].children.clone();
        for child in children {
            self.paint_node(child);
        }
        let element = &self.elements[i];
        if let Some(radius) = element.focus_radius
            && self.interaction.focus_visible
            && self.interaction.focused == Some(element.widget)
        {
            let ix = self.theme.interaction;
            let grow = ix.focus_ring_offset + ix.focus_ring_width;
            let ring = Quad::new(frame.inflate(grow), Color::TRANSPARENT)
                .radius(radius + grow)
                .border(ix.focus_ring_width, self.theme.color(Semantic::FocusRing));
            self.scene.push_quad(ring);
        }
    }
}
