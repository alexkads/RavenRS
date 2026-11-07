//! Spinner, carousel, splitter, menubar, one-time-code input and form fields.

use std::cell::Cell;
use std::fmt;
use std::rc::Rc;

use ravenrs_core::{Border, Color, CursorIcon, Effect, Insets, Key, Quad, Rect, Role};
use ravenrs_layout::{Align, Axis, Justify, LayoutStyle};
use ravenrs_reactive::{Binding, State};
use ravenrs_style::{Semantic, TextStyle, Weight};

use crate::context::{BuildCx, DragPhase, NodeId, Paint, Placement, QuadPaint, Semantics};
use crate::overlay::{Entry, MenuList};
use crate::{AnyView, ButtonSize, ButtonStyle, IconButton, IconName, MenuItem, Motion, View, util};

/// An indeterminate activity indicator. With reduced motion it is static.
#[derive(Clone, Copy, Debug)]
pub struct Spinner {
    size: f32,
    color: Option<Color>,
}

impl Default for Spinner {
    fn default() -> Self {
        Self { size: 20.0, color: None }
    }
}

impl Spinner {
    /// A 20 px spinner.
    pub fn new() -> Self {
        Self::default()
    }

    /// Diameter in logical pixels (default 20).
    pub fn size(mut self, size: f32) -> Self {
        self.size = size;
        self
    }

    /// Sets the color; by default it comes from the theme accent, which disappears
    /// on an accent fill.
    pub fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }
}

/// Paints a spinner of `size` at the center of the element's frame. Being only a paint, it
/// can sit behind content that must keep its size, as in a loading button.
pub(crate) fn spinner_paint(size: f32, color: Color) -> Paint {
    Paint::Live(Rc::new(move |f: Rect, time, scene| {
        let c = f.center();
        let dot = (size * 0.18).max(2.0);
        let r = size / 2.0 - dot / 2.0;
        // One turn every 0.8 s; the brightness moves continuously around the ring.
        let head = (time / 0.8).fract() as f32 * 8.0;
        for i in 0..8 {
            let a = i as f32 / 8.0 * std::f32::consts::TAU;
            let age = (i as f32 - head).rem_euclid(8.0);
            let alpha = 1.0 - age / 9.0;
            let rect = Rect::new(c.x + r * a.cos() - dot / 2.0, c.y + r * a.sin() - dot / 2.0, dot, dot);
            scene.push_quad(Quad::new(rect, color.with_alpha(color.a * alpha)).radius(dot / 2.0));
        }
    }))
}

impl View for Spinner {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let size = self.size;
        let color =
            self.color.unwrap_or_else(|| theme.color(Semantic::Accent).lerp(theme.color(Semantic::Ink), 0.2));
        let node = cx.container(
            LayoutStyle { width: Some(size), height: Some(size), ..LayoutStyle::leaf() },
            &[],
            spinner_paint(size, color),
        );
        let name = cx.labels().loading.clone();
        cx.semantics(node, Semantics::new(Role::ProgressIndicator).name(name));
        node
    }
}

/// Slides with previous/next buttons, position dots and optional autoplay.
pub struct Carousel {
    slides: Vec<AnyView>,
    height: f32,
    autoplay: Option<f64>,
}

impl fmt::Debug for Carousel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Carousel").field("slides", &self.slides.len()).finish_non_exhaustive()
    }
}

impl Carousel {
    /// A carousel over `slides`, starting at the first.
    pub fn new(slides: Vec<AnyView>) -> Self {
        Self { slides, height: 240.0, autoplay: None }
    }

    /// Slide area height in logical pixels (default 240).
    pub fn height(mut self, height: f32) -> Self {
        self.height = height;
        self
    }

    /// Advances every `seconds`.
    pub fn autoplay(mut self, seconds: f64) -> Self {
        self.autoplay = Some(seconds.max(0.5));
        self
    }
}

struct Slide<'a>(&'a dyn View, f32);

impl View for Slide<'_> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let m = cx.theme().motion;
        let child = cx.child(self.0);
        let node = cx.container(
            LayoutStyle {
                grow: 1.0,
                align: Align::Stretch,
                justify: Justify::Center,
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &[child],
            Paint::None,
        );
        cx.motion(
            node,
            Motion::new()
                .enter(Effect::moved(32.0 * self.1, 0.0).with_opacity(0.0), m.snappy)
                .exit(Effect::hidden(), m.fast),
        );
        node
    }
}

impl View for Carousel {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let n = self.slides.len().max(1);
        let index: State<usize> = cx.use_state(|| 0);
        let since: State<f64> = cx.use_state(|| f64::NAN);
        let now = cx.now();
        if let Some(every) = self.autoplay {
            let start = *since.peek();
            if start.is_nan() {
                since.set_if_changed(now);
            } else if now - start >= every {
                index.update(|i| *i = (*i + 1) % n);
                since.set(now);
            }
            cx.wake_at(if start.is_nan() { now + every } else { start + every });
        }
        let current = index.get() % n;
        // Slides arrive from the side the user is heading to.
        let shown = cx.use_state(|| current);
        let direction = cx.use_state(|| 1.0_f32);
        let previous = *shown.peek();
        if previous != current {
            let forward = (current + n - previous) % n <= n / 2;
            direction.set_if_changed(if forward { 1.0 } else { -1.0 });
            shown.set_if_changed(current);
        }
        let heading = *direction.peek();
        let slide = match self.slides.get(current) {
            Some(s) => cx.keyed_child(current as u64, &Slide(s.as_ref(), heading)),
            None => cx.empty(),
        };
        let stage = cx.container(
            LayoutStyle {
                height: Some(self.height),
                align: Align::Stretch,
                justify: Justify::Center,
                overflow_hidden: true,
                align_self: Some(Align::Stretch),
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &[slide],
            Paint::Quad(QuadPaint {
                corner_radius: theme.radii.lg,
                ..QuadPaint::new(theme.color(Semantic::Control))
            }),
        );
        cx.clip(stage);
        let labels = cx.labels();
        let (prev, next, s1, s2) = (index.clone(), index.clone(), since.clone(), since.clone());
        let back = cx.child(
            &IconButton::new(IconName::ChevronLeft)
                .label(cx.labels().carousel_previous.clone())
                .style(ButtonStyle::Secondary)
                .size(ButtonSize::Small)
                .on_click(move || {
                    prev.update(|i| *i = (*i + n - 1) % n);
                    s1.set(f64::NAN);
                }),
        );
        let forward = cx.child(
            &IconButton::new(IconName::ChevronRight)
                .label(cx.labels().carousel_next.clone())
                .style(ButtonStyle::Secondary)
                .size(ButtonSize::Small)
                .on_click(move || {
                    next.update(|i| *i = (*i + 1) % n);
                    s2.set(f64::NAN);
                }),
        );
        let mut dots = Vec::new();
        for i in 0..n {
            let on = i == current;
            let color = if on {
                theme.color(Semantic::Accent)
            } else {
                theme.color(Semantic::InkMuted).with_alpha(0.4)
            };
            let dot = cx.container(
                LayoutStyle {
                    width: Some(if on { 20.0 } else { 8.0 }),
                    height: Some(8.0),
                    ..LayoutStyle::leaf()
                },
                &[],
                Paint::Quad(QuadPaint { corner_radius: 4.0, ..QuadPaint::new(color) }),
            );
            cx.semantics(
                dot,
                Semantics::new(Role::Button)
                    .name(format!("{} {} {} {n}", labels.carousel_slide, i + 1, labels.grid_of))
                    .selected(on),
            );
            let idx = index.clone();
            cx.on_click(dot, Rc::new(move || idx.set(i)));
            cx.cursor(dot, CursorIcon::Pointer);
            cx.motion(dot, Motion::new().key(i as u64).layout(theme.motion.snappy).paint(theme.motion.fast));
            dots.push(dot);
        }
        let dots = cx.container(
            LayoutStyle {
                spacing: 6.0,
                align: Align::Center,
                grow: 1.0,
                justify: Justify::Center,
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &dots,
            Paint::None,
        );
        let controls = cx.container(
            LayoutStyle {
                align: Align::Center,
                align_self: Some(Align::Stretch),
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &[back, dots, forward],
            Paint::None,
        );
        cx.container(
            LayoutStyle {
                spacing: 12.0,
                align: Align::Stretch,
                align_self: Some(Align::Stretch),
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &[stage, controls],
            Paint::None,
        )
    }
}

/// How far an arrow key moves the bar between panes, in logical pixels.
const SPLIT_STEP: f32 = 16.0;
/// How wide the bar between panes is, grip and room around it, in logical pixels.
pub(crate) const SPLIT_BAR: f32 = 10.0;

/// The line and the grip of the bar between panes.
fn split_bar_paint(vertical_stack: bool, line: Color, grip: Color) -> Paint {
    Paint::Custom(Rc::new(move |f: Rect, scene| {
        let c = f.center();
        if vertical_stack {
            scene.push_quad(Quad::new(Rect::new(f.x(), c.y - 0.5, f.width(), 1.0), line));
            scene.push_quad(Quad::new(Rect::new(c.x - 12.0, c.y - 2.0, 24.0, 4.0), grip).radius(2.0));
        } else {
            scene.push_quad(Quad::new(Rect::new(c.x - 0.5, f.y(), 1.0, f.height()), line));
            scene.push_quad(Quad::new(Rect::new(c.x - 2.0, c.y - 12.0, 4.0, 24.0), grip).radius(2.0));
        }
    }))
}

/// What an arrow key does to the bar between panes: it moves the way the arrow points.
/// Across a stack, Up and Down still work, as the keys assistive technology sends.
fn split_arrow(key: &Key, vertical_stack: bool) -> Option<f32> {
    match (key, vertical_stack) {
        (Key::ArrowUp, true) | (Key::ArrowLeft | Key::ArrowDown, false) => Some(-1.0),
        (Key::ArrowDown, true) | (Key::ArrowRight | Key::ArrowUp, false) => Some(1.0),
        _ => None,
    }
}

/// A bar to drag that changes a size, for a pane an app lays out itself, such as a strip
/// along the bottom of a window. Arrow keys move it too, Shift goes further, and Home and
/// End go to the limits. [`SplitPanes`] is built on it.
///
/// ```
/// use ravenrs_layout::Axis;
/// use ravenrs_reactive::State;
/// use ravenrs_widgets::{SplitHandle, Ui};
///
/// let height = State::new(120.0_f32);
/// // The strip is below the bar, so it grows as the bar goes up.
/// let bar = SplitHandle::new(Axis::Vertical, height.binding()).range(80.0, 300.0).inverted();
/// let mut ui = Ui::new(bar, ravenrs_style::Theme::raven_dark());
/// ui.update();
/// ```
pub struct SplitHandle {
    axis: Axis,
    value: Binding<f32>,
    min: f32,
    max: f32,
    inverted: bool,
    label: Option<String>,
    on_end: Option<Rc<dyn Fn()>>,
}

impl fmt::Debug for SplitHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SplitHandle").field("axis", &self.axis).finish_non_exhaustive()
    }
}

impl SplitHandle {
    /// A bar between things laid out along `axis`, which writes the size it controls to
    /// `value`. It lies across that axis: along `Axis::Horizontal`, it stands upright.
    pub fn new(axis: Axis, value: Binding<f32>) -> Self {
        Self { axis, value, min: 0.0, max: f32::MAX, inverted: false, label: None, on_end: None }
    }

    /// The smallest and the largest the size can get.
    pub fn range(mut self, min: f32, max: f32) -> Self {
        self.min = min;
        self.max = max.max(min);
        self
    }

    /// The size grows as the bar moves left or up, for a pane after the bar.
    pub fn inverted(mut self) -> Self {
        self.inverted = true;
        self
    }

    /// Its name for screen readers (from [`Labels`](crate::Labels) by default).
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Runs when a drag ends or a key moved it, the moment to save the size.
    pub fn on_end(mut self, handler: impl Fn() + 'static) -> Self {
        self.on_end = Some(Rc::new(handler));
        self
    }
}

impl View for SplitHandle {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let vertical = self.axis == Axis::Vertical;
        let lit = cx.is_hovered() || cx.is_focus_visible();
        let (line, grip) = (
            theme.color(Semantic::Border),
            theme.color(if lit { Semantic::InkMuted } else { Semantic::Border }),
        );
        let style = if vertical {
            LayoutStyle { height: Some(SPLIT_BAR), align_self: Some(Align::Stretch), ..LayoutStyle::leaf() }
        } else {
            LayoutStyle { width: Some(SPLIT_BAR), align_self: Some(Align::Stretch), ..LayoutStyle::leaf() }
        };
        let node = cx.container(style, &[], split_bar_paint(vertical, line, grip));
        let (min, max, sign) = (self.min, self.max, if self.inverted { -1.0 } else { 1.0 });
        let name = self.label.clone().unwrap_or_else(|| cx.labels().split_resize.clone());
        let step = Some(f64::from(SPLIT_STEP));
        let current = self.value.get();
        // There is no separator role with a value in the tree yet; a slider says what this does.
        cx.semantics(
            node,
            Semantics::new(Role::Slider).name(name).numeric(
                f64::from(current),
                f64::from(min),
                f64::from(max),
                step,
            ),
        );
        let start = cx.use_state(|| Rc::new(Cell::new((0.0_f32, 0.0_f32)))).get();
        let (value, on_end) = (self.value.clone(), self.on_end.clone());
        cx.on_drag(
            node,
            Rc::new(move |d| {
                let position = if vertical { d.position.y } else { d.position.x };
                match d.phase {
                    DragPhase::Start => start.set((position, value.get())),
                    DragPhase::Move => {
                        let (from, base) = start.get();
                        value.set((base + sign * (position - from)).clamp(min, max));
                    }
                    DragPhase::End => {
                        if let Some(on_end) = &on_end {
                            on_end();
                        }
                    }
                }
            }),
        );
        let (value, on_end) = (self.value.clone(), self.on_end.clone());
        cx.on_key(
            node,
            Rc::new(move |key, modifiers| {
                let next = match key {
                    Key::Home => min,
                    Key::End => max,
                    _ => {
                        let Some(arrow) = split_arrow(key, vertical) else { return false };
                        let far = if modifiers.shift { 4.0 } else { 1.0 };
                        (value.get() + sign * arrow * far * SPLIT_STEP).clamp(min, max)
                    }
                };
                value.set(next);
                if let Some(on_end) = &on_end {
                    on_end();
                }
                true
            }),
        );
        cx.focusable(node, 2.0);
        cx.cursor(node, if vertical { CursorIcon::RowResize } else { CursorIcon::ColResize });
        node
    }
}

/// One pane of a [`SplitPanes`].
pub struct SplitPane {
    content: AnyView,
    size: Option<f32>,
    min: f32,
    max: f32,
    visible: bool,
}

impl fmt::Debug for SplitPane {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SplitPane").field("size", &self.size).finish_non_exhaustive()
    }
}

impl SplitPane {
    /// A pane around `content`. Without [`Self::size`], it takes the room the others leave.
    pub fn new(content: impl View + 'static) -> Self {
        Self { content: Box::new(content), size: None, min: 0.0, max: f32::MAX, visible: true }
    }

    /// Its size to start with along the stack, in logical pixels.
    pub fn size(mut self, size: f32) -> Self {
        self.size = Some(size);
        self
    }

    /// The smallest and the largest it can get, in logical pixels. A narrow window takes
    /// room from the panes down to their smallest.
    pub fn range(mut self, min: f32, max: f32) -> Self {
        self.min = min;
        self.max = max.max(min);
        self
    }

    /// Whether it is showing (it is by default). A hidden pane keeps its size for later,
    /// and takes its bar with it.
    pub fn visible(mut self, visible: bool) -> Self {
        self.visible = visible;
        self
    }
}

/// The sizes the visible panes get in `room`: what they ask for within their limits, with
/// the free room going to those that do not ask, and any lack taken from the last ones.
fn split_sizes(room: f32, asked: &[Option<f32>], limits: &[(f32, f32)]) -> Vec<f32> {
    let mut sizes: Vec<f32> =
        asked.iter().zip(limits).map(|(size, (min, max))| size.unwrap_or(*min).clamp(*min, *max)).collect();
    let free = room - asked.iter().zip(&sizes).filter(|(a, _)| a.is_some()).map(|(_, s)| *s).sum::<f32>();
    let flexible = asked.iter().filter(|a| a.is_none()).count();
    if flexible > 0 {
        let share = (free / flexible as f32).max(0.0);
        for ((size, ask), (min, max)) in sizes.iter_mut().zip(asked).zip(limits) {
            if ask.is_none() {
                *size = share.clamp(*min, *max);
            }
        }
    }
    let mut over = sizes.iter().sum::<f32>() - room;
    for (size, (min, _)) in sizes.iter_mut().zip(limits).rev() {
        if over <= 0.0 {
            break;
        }
        let taken = (*size - min).min(over).max(0.0);
        *size -= taken;
        over -= taken;
    }
    sizes
}

/// Any number of panes side by side or stacked, with a bar to drag between each two.
/// Panes can have limits, and the sizes can be kept by the app to remember them.
///
/// ```
/// use ravenrs_widgets::{SplitPane, SplitPanes, Text, Ui};
///
/// let columns = SplitPanes::horizontal()
///     .pane(SplitPane::new(Text::new("Sessions")).size(280.0).range(200.0, 400.0))
///     .pane(SplitPane::new(Text::new("Receipt")).range(320.0, f32::MAX))
///     .pane(SplitPane::new(Text::new("Activity")).size(300.0).range(240.0, 420.0))
///     .height(400.0);
/// let mut ui = Ui::new(columns, ravenrs_style::Theme::raven_dark());
/// ui.update();
/// ```
#[derive(Debug)]
pub struct SplitPanes {
    axis: Axis,
    panes: Vec<SplitPane>,
    sizes: Option<Binding<Vec<Option<f32>>>>,
    height: Option<f32>,
    on_end: Option<SplitEnd>,
}

/// Runs when a bar stops moving.
struct SplitEnd(Rc<dyn Fn()>);

impl fmt::Debug for SplitEnd {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SplitEnd")
    }
}

impl SplitPanes {
    /// No panes yet, to go side by side.
    pub fn horizontal() -> Self {
        Self { axis: Axis::Horizontal, panes: Vec::new(), sizes: None, height: None, on_end: None }
    }

    /// No panes yet, to stack.
    pub fn vertical() -> Self {
        Self { axis: Axis::Vertical, ..Self::horizontal() }
    }

    /// Adds a pane.
    pub fn pane(mut self, pane: SplitPane) -> Self {
        self.panes.push(pane);
        self
    }

    /// Keeps the sizes here, one for each pane, hidden ones included; `None` is a pane
    /// that takes the room left. What is missing comes from [`SplitPane::size`].
    pub fn sizes(mut self, sizes: Binding<Vec<Option<f32>>>) -> Self {
        self.sizes = Some(sizes);
        self
    }

    /// Runs when a drag ends or a key moved a bar, the moment to save the sizes.
    pub fn on_end(mut self, handler: impl Fn() + 'static) -> Self {
        self.on_end = Some(SplitEnd(Rc::new(handler)));
        self
    }

    /// Its height, in logical pixels. Without it, it takes the room its parent has.
    pub fn height(mut self, height: f32) -> Self {
        self.height = Some(height);
        self
    }
}

impl View for SplitPanes {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let vertical = self.axis == Axis::Vertical;
        let own: State<Vec<Option<f32>>> = cx.use_state(Vec::new);
        let store = self.sizes.clone().unwrap_or_else(|| own.binding());
        let stored = store.get();
        let asked_of = |i: usize, pane: &SplitPane| stored.get(i).copied().unwrap_or(pane.size);
        let shown: Vec<usize> =
            self.panes.iter().enumerate().filter(|(_, p)| p.visible).map(|(i, _)| i).collect();
        let asked: Vec<Option<f32>> = shown.iter().map(|&i| asked_of(i, &self.panes[i])).collect();
        let limits: Vec<(f32, f32)> = shown.iter().map(|&i| (self.panes[i].min, self.panes[i].max)).collect();
        // Known from the second build on, when the bars need it.
        let room = cx.scroll_viewport().map(|v| {
            (if vertical { v.height } else { v.width }) - shown.len().saturating_sub(1) as f32 * SPLIT_BAR
        });
        let sizes = room.map(|room| split_sizes(room, &asked, &limits));
        let mut children = Vec::with_capacity(shown.len() * 2);
        for (at, &index) in shown.iter().enumerate() {
            if at > 0 {
                let (a, b) = (at - 1, at);
                let (size_a, size_b) = sizes.as_ref().map_or_else(
                    || (asked[a].unwrap_or(limits[a].0), asked[b].unwrap_or(limits[b].0)),
                    |s| (s[a], s[b]),
                );
                let both = size_a + size_b;
                let min = limits[a].0.max(both - limits[b].1);
                let max = limits[a].1.min(both - limits[b].0).max(min);
                let write = store.clone();
                let panes = (shown[a], shown[b]);
                let fixed = (asked[a].is_some(), asked[b].is_some());
                let defaults: Vec<Option<f32>> = self.panes.iter().map(|p| p.size).collect();
                let value = Binding::new(
                    move || size_a,
                    move |next: f32| {
                        let mut all = write.get();
                        if all.len() < defaults.len() {
                            let have = all.len();
                            all.extend_from_slice(&defaults[have..]);
                        }
                        // A pane that takes what is left goes on doing so, unless both do.
                        if fixed.0 || !fixed.1 {
                            all[panes.0] = Some(next);
                        }
                        if fixed.1 {
                            all[panes.1] = Some(both - next);
                        }
                        write.set(all);
                    },
                );
                let mut bar = SplitHandle::new(self.axis, value).range(min, max);
                if let Some(end) = &self.on_end {
                    let end = end.0.clone();
                    bar = bar.on_end(move || end());
                }
                children.push(cx.keyed_child(u64::MAX - index as u64, &bar));
            }
            let pane = &self.panes[index];
            let content = cx.keyed_child(index as u64, pane.content.as_ref());
            // What takes the room left grows; the others hold their size.
            let size = asked[at].map(|asked| sizes.as_ref().map_or(asked, |s| s[at]));
            let node = cx.container(
                LayoutStyle {
                    grow: if size.is_some() { 0.0 } else { 1.0 },
                    width: if vertical { None } else { size.or(Some(0.0)) },
                    height: if vertical { size.or(Some(0.0)) } else { None },
                    min_width: Some(0.0),
                    min_height: Some(0.0),
                    align: Align::Stretch,
                    overflow_hidden: true,
                    ..LayoutStyle::stack(Axis::Vertical)
                },
                &[content],
                Paint::None,
            );
            cx.clip(node);
            children.push(node);
        }
        let node = cx.container(
            LayoutStyle {
                height: self.height,
                grow: if self.height.is_none() { 1.0 } else { 0.0 },
                min_width: Some(0.0),
                min_height: Some(0.0),
                align: Align::Stretch,
                align_self: Some(Align::Stretch),
                overflow_hidden: true,
                ..LayoutStyle::stack(self.axis)
            },
            &children,
            Paint::None,
        );
        cx.report_frame(node);
        node
    }
}

/// Two panes with a draggable divider between them. Arrow keys move the divider too. For
/// more panes, or limits in logical pixels, see [`SplitPanes`].
///
/// ```
/// use ravenrs_widgets::{Splitter, Text, Ui};
///
/// let split = Splitter::horizontal(Text::new("Files"), Text::new("Editor"))
///     .ratio(0.3)
///     .height(400.0);
/// let mut ui = Ui::new(split, ravenrs_style::Theme::raven_dark());
/// ui.update();
/// ```
pub struct Splitter<A, B> {
    first: A,
    second: B,
    vertical: bool,
    initial: f32,
    height: f32,
    limits: (f32, f32),
}

impl<A, B> fmt::Debug for Splitter<A, B> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Splitter").field("vertical", &self.vertical).finish_non_exhaustive()
    }
}

impl<A: View, B: View> Splitter<A, B> {
    /// Side-by-side panes.
    pub fn horizontal(first: A, second: B) -> Self {
        Self { first, second, vertical: false, initial: 0.5, height: 260.0, limits: (0.1, 0.9) }
    }

    /// Stacked panes.
    pub fn vertical(first: A, second: B) -> Self {
        Self { first, second, vertical: true, initial: 0.5, height: 260.0, limits: (0.1, 0.9) }
    }

    /// The first pane's initial share, within the limits (0.1 to 0.9 by default).
    pub fn ratio(mut self, ratio: f32) -> Self {
        self.initial = ratio;
        self
    }

    /// The smallest and the largest share of the first pane (0.1 and 0.9 by default).
    pub fn limits(mut self, min: f32, max: f32) -> Self {
        let min = min.clamp(0.0, 1.0);
        self.limits = (min, max.clamp(min, 1.0));
        self
    }

    /// Total height in logical pixels (default 260).
    pub fn height(mut self, height: f32) -> Self {
        self.height = height;
        self
    }
}

impl<A: View, B: View> View for Splitter<A, B> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let (min, max) = self.limits;
        let initial = self.initial.clamp(min, max);
        let ratio: State<f32> = cx.use_state(move || initial);
        let r = ratio.get();
        let pane = |grow: f32| LayoutStyle {
            grow,
            width: (!self.vertical).then_some(0.0),
            height: self.vertical.then_some(0.0),
            min_width: Some(0.0),
            min_height: Some(0.0),
            align: Align::Stretch,
            overflow_hidden: true,
            ..LayoutStyle::stack(Axis::Vertical)
        };
        let a = cx.child(&self.first);
        let a = cx.container(pane(r), &[a], Paint::None);
        cx.clip(a);
        let b = cx.child(&self.second);
        let b = cx.container(pane(1.0 - r), &[b], Paint::None);
        cx.clip(b);
        let vertical = self.vertical;
        let (line, grip) = (theme.color(Semantic::Border), theme.color(Semantic::InkMuted));
        let lit = cx.is_hover_within() || cx.is_focus_visible();
        let handle_style = if vertical {
            LayoutStyle { height: Some(SPLIT_BAR), align_self: Some(Align::Stretch), ..LayoutStyle::leaf() }
        } else {
            LayoutStyle { width: Some(SPLIT_BAR), align_self: Some(Align::Stretch), ..LayoutStyle::leaf() }
        };
        let handle =
            cx.container(handle_style, &[], split_bar_paint(vertical, line, if lit { grip } else { line }));
        let drag = ratio.clone();
        cx.on_drag(
            handle,
            Rc::new(move |d| {
                if d.phase == DragPhase::End {
                    return;
                }
                let (pos, start, len) = if vertical {
                    (d.position.y, d.container.y(), d.container.height())
                } else {
                    (d.position.x, d.container.x(), d.container.width())
                };
                if len > SPLIT_BAR {
                    drag.set(((pos - start - SPLIT_BAR / 2.0) / (len - SPLIT_BAR)).clamp(min, max));
                }
            }),
        );
        let name = cx.labels().split_resize.clone();
        cx.semantics(
            handle,
            Semantics::new(Role::Slider).name(name).numeric(
                f64::from(r),
                f64::from(min),
                f64::from(max),
                Some(0.02),
            ),
        );
        let keys = ratio.clone();
        cx.on_key(
            handle,
            Rc::new(move |key, modifiers| {
                let next = match key {
                    Key::Home => min,
                    Key::End => max,
                    _ => {
                        let Some(arrow) = split_arrow(key, vertical) else { return false };
                        let far = if modifiers.shift { 0.08 } else { 0.02 };
                        (*keys.peek() + arrow * far).clamp(min, max)
                    }
                };
                keys.set(next);
                true
            }),
        );
        cx.focusable(handle, 2.0);
        cx.cursor(handle, if vertical { CursorIcon::RowResize } else { CursorIcon::ColResize });
        let node = cx.container(
            LayoutStyle {
                height: Some(self.height),
                align: Align::Stretch,
                align_self: Some(Align::Stretch),
                ..LayoutStyle::stack(if vertical { Axis::Vertical } else { Axis::Horizontal })
            },
            &[a, handle, b],
            Paint::Quad(QuadPaint {
                corner_radius: theme.radii.lg,
                border: Some(Border { width: 1.0, color: theme.color(Semantic::Border) }),
                ..QuadPaint::new(theme.color(Semantic::Surface))
            }),
        );
        cx.clip(node);
        node
    }
}

/// A horizontal bar of menus (File, Edit, View…).
#[derive(Default)]
pub struct Menubar {
    menus: Vec<(String, Vec<Entry>)>,
}

impl fmt::Debug for Menubar {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.menus.iter().map(|m| &m.0)).finish()
    }
}

impl Menubar {
    /// An empty menubar; add menus with [`Menubar::menu`].
    pub fn new() -> Self {
        Self::default()
    }

    /// Starts a new menu; following `item`/`separator` calls add to it.
    pub fn menu(mut self, title: impl Into<String>) -> Self {
        self.menus.push((title.into(), Vec::new()));
        self
    }

    /// Adds an item to the current menu (ignored before the first `menu`).
    pub fn item(mut self, item: MenuItem) -> Self {
        if let Some((_, e)) = self.menus.last_mut() {
            e.push(Entry::Item(item));
        }
        self
    }

    /// Adds a separator to the current menu (ignored before the first `menu`).
    pub fn separator(mut self) -> Self {
        if let Some((_, e)) = self.menus.last_mut() {
            e.push(Entry::Separator);
        }
        self
    }
}

struct MenuTitle<'a> {
    bar: &'a Menubar,
    open: State<Option<usize>>,
    roving: util::Roving,
}

impl View for MenuTitle<'_> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let (i, count) = (self.roving.index, self.roving.count);
        let is_open = self.open.get() == Some(i);
        let hovered = cx.is_hovered();
        let ink = theme.color(Semantic::Ink);
        let label = util::label(cx, &self.bar.menus[i].0, TextStyle::Callout, Some(Weight::SEMIBOLD), ink);
        let fill = if is_open {
            theme.color(Semantic::Control)
        } else if hovered {
            ink.with_alpha(0.06)
        } else {
            Color::TRANSPARENT
        };
        let node = cx.container(
            LayoutStyle {
                height: Some(30.0),
                padding: Insets::symmetric(0.0, 10.0),
                align: Align::Center,
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &[label],
            Paint::Quad(QuadPaint { corner_radius: 6.0, ..QuadPaint::new(fill) }),
        );
        cx.semantics(node, Semantics::new(Role::MenuItem).expanded(is_open));
        let open = self.open.clone();
        cx.on_click(node, Rc::new(move || open.update(|o| *o = if *o == Some(i) { None } else { Some(i) })));
        cx.cursor(node, CursorIcon::Pointer);
        cx.focusable(node, 6.0);
        // WAI-ARIA menubar: Left and Right move between titles (switching the
        // open menu if one is open); Down, Enter and Space open the menu.
        let opener = self.open.clone();
        let before: crate::KeyHandler = Rc::new(move |key, _| {
            let opens = matches!(key, Key::ArrowDown | Key::Enter | Key::Space);
            if opens {
                opener.set(Some(i));
            }
            opens || *key == Key::ArrowUp
        });
        let follow = self.open.clone();
        let on_move: Rc<dyn Fn(usize)> = Rc::new(move |to| {
            if follow.get().is_some() {
                follow.set(Some(to));
            }
        });
        self.roving.wire_with(cx, node, Some(on_move), Some(before));
        let was_open = cx.use_state(|| Rc::new(Cell::new(false))).get();
        if is_open {
            was_open.set(true);
            let close: Rc<dyn Fn()> = {
                let open = self.open.clone();
                Rc::new(move || open.set(None))
            };
            // The neighbor menu opens and focuses its first item.
            let side_open = self.open.clone();
            let on_side: Rc<dyn Fn(bool)> = Rc::new(move |forward| {
                side_open.set(Some(if forward { (i + 1) % count } else { (i + count - 1) % count }));
            });
            cx.overlay(
                node,
                Placement::Below,
                &MenuList { entries: &self.bar.menus[i].1, close: close.clone(), on_side: Some(on_side) },
                Some(close),
            );
        } else if was_open.replace(false) && self.open.get().is_none() {
            // Focus returns to the title when the menubar's menu closes.
            cx.request_focus_node(node);
        }
        node
    }
}

impl View for Menubar {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let open: State<Option<usize>> = cx.use_state(|| None);
        let (count, focus_to) = (self.menus.len(), util::roving_cell(cx, None));
        let titles: Vec<NodeId> = (0..count)
            .map(|index| {
                let roving = util::Roving { index, count, focus_to: focus_to.clone() };
                cx.keyed_child(index as u64, &MenuTitle { bar: self, open: open.clone(), roving })
            })
            .collect();
        let node = cx.container(
            LayoutStyle {
                padding: Insets::uniform(4.0),
                spacing: 2.0,
                align_self: Some(Align::Fit),
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &titles,
            Paint::Quad(QuadPaint {
                corner_radius: theme.radii.md,
                border: Some(Border { width: 1.0, color: theme.color(Semantic::Border) }),
                ..QuadPaint::new(theme.color(Semantic::Surface))
            }),
        );
        cx.semantics(node, Semantics::new(Role::MenuBar));
        node
    }
}

/// Separate boxes for a one-time code; accepts digits by default.
pub struct InputOtp {
    value: Binding<String>,
    length: usize,
    on_complete: Option<Rc<dyn Fn(String)>>,
}

impl fmt::Debug for InputOtp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("InputOtp").field("length", &self.length).finish_non_exhaustive()
    }
}

impl InputOtp {
    /// `length` boxes (clamped to 1–12) editing `value`.
    pub fn new(value: Binding<String>, length: usize) -> Self {
        Self { value, length: length.clamp(1, 12), on_complete: None }
    }

    /// Called once all boxes are filled.
    pub fn on_complete(mut self, handler: impl Fn(String) + 'static) -> Self {
        self.on_complete = Some(Rc::new(handler));
        self
    }
}

impl View for InputOtp {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let focused = cx.is_focused();
        let text: Vec<char> = self.value.get().chars().take(self.length).collect();
        let mut boxes = Vec::new();
        let mut attrs = util::attrs(&theme, TextStyle::Title, Some(Weight::SEMIBOLD));
        attrs.size = 20.0;
        for i in 0..self.length {
            let ch = text.get(i).map(|c| c.to_string()).unwrap_or_default();
            let label = cx.text(&ch, &attrs, theme.color(Semantic::Ink), LayoutStyle::leaf());
            let active = focused && i == text.len().min(self.length - 1);
            let border = if active {
                Border { width: 1.5, color: theme.color(Semantic::Accent) }
            } else {
                Border { width: 1.0, color: theme.color(Semantic::Border) }
            };
            boxes.push(cx.container(
                LayoutStyle {
                    width: Some(44.0),
                    height: Some(52.0),
                    align: Align::Center,
                    justify: Justify::Center,
                    ..LayoutStyle::stack(Axis::Horizontal)
                },
                &[label],
                Paint::Quad(QuadPaint {
                    corner_radius: theme.radii.md,
                    border: Some(border),
                    ..QuadPaint::new(theme.color(Semantic::Control))
                }),
            ));
        }
        let node = cx.container(
            LayoutStyle {
                spacing: 8.0,
                align_self: Some(Align::Fit),
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &boxes,
            Paint::None,
        );
        for b in &boxes {
            cx.semantics(*b, Semantics::hidden());
        }
        let typed: String = self.value.get().chars().take(self.length).collect();
        let mut semantics = Semantics::new(Role::TextInput)
            .name(cx.labels().verification_code.clone())
            .value(typed)
            .description(format!("{} digits", self.length));
        semantics.fallback_name = true;
        cx.semantics(node, semantics);
        let (value, len, done) = (self.value.clone(), self.length, self.on_complete.clone());
        cx.on_key(
            node,
            Rc::new(move |key, _| {
                let mut v: String = value.get().chars().take(len).collect();
                match key {
                    Key::Character(s) => {
                        for c in s.chars().filter(char::is_ascii_digit) {
                            if v.chars().count() < len {
                                v.push(c);
                            }
                        }
                    }
                    Key::Backspace => {
                        v.pop();
                    }
                    _ => return false,
                }
                let complete = v.chars().count() == len;
                value.set(v.clone());
                if complete && let Some(d) = &done {
                    d(v);
                }
                true
            }),
        );
        cx.focusable(node, theme.radii.md);
        cx.cursor(node, CursorIcon::Text);
        node
    }
}

/// Common validation rules for [`FormField::validate`].
pub mod validate {
    /// The field must not be empty.
    pub fn required(value: &str) -> Result<(), String> {
        if value.trim().is_empty() { Err("This field is required.".into()) } else { Ok(()) }
    }

    /// A plausible email address (something@something.tld).
    pub fn email(value: &str) -> Result<(), String> {
        let ok = value.split_once('@').is_some_and(|(user, domain)| {
            !user.is_empty() && domain.contains('.') && !domain.starts_with('.') && !domain.ends_with('.')
        });
        if ok || value.is_empty() { Ok(()) } else { Err("Enter an email like name@example.com.".into()) }
    }

    /// At least `n` characters.
    pub fn min_length(n: usize) -> impl Fn(&str) -> Result<(), String> {
        move |value| {
            if value.chars().count() >= n || value.is_empty() {
                Ok(())
            } else {
                Err(format!("Use at least {n} characters."))
            }
        }
    }
}

/// A validation rule: `Ok(())` or a message for the user.
pub type Rule = Rc<dyn Fn(&str) -> Result<(), String>>;

/// A labeled form field with a hint and validation messages.
///
/// Errors appear once the value is non-empty or the form was submitted
/// (see [`FormField::show_errors`]), so empty forms do not start out red.
///
/// ```
/// use ravenrs_reactive::State;
/// use ravenrs_widgets::{FormField, TextField, Ui, validate};
///
/// let email = State::new("raven@".to_string());
/// let field = FormField::new("Email", TextField::new(email.binding()))
///     .hint("We never share it.")
///     .validate(email.binding(), validate::email);
/// assert!(field.error().is_some());
///
/// email.set("raven@example.com".into());
/// assert_eq!(field.error(), None);
///
/// let mut ui = Ui::new(field, ravenrs_style::Theme::raven_dark());
/// ui.update();
/// ```
pub struct FormField<V> {
    label: crate::Label,
    content: V,
    hint: Option<String>,
    value: Option<Binding<String>>,
    rules: Vec<Rule>,
    show_errors: bool,
    required: bool,
    told_error: Option<String>,
    told_note: Option<(String, crate::Tone)>,
    fill: Option<Semantic>,
    accessible_name: Option<String>,
    accessible_description: Option<String>,
}

impl<V> fmt::Debug for FormField<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FormField").field("rules", &self.rules.len()).finish_non_exhaustive()
    }
}

impl<V: View> FormField<V> {
    /// A field with this label above `content`.
    pub fn new(label: impl Into<String>, content: V) -> Self {
        Self {
            label: crate::Label::new(label),
            content,
            hint: None,
            value: None,
            rules: Vec::new(),
            show_errors: false,
            required: false,
            told_error: None,
            told_note: None,
            fill: None,
            accessible_name: None,
            accessible_description: None,
        }
    }

    /// What screen readers call the control, instead of the label: for a label that only makes
    /// sense next to what is around it.
    pub fn accessible_name(mut self, name: impl Into<String>) -> Self {
        self.accessible_name = Some(name.into());
        self
    }

    /// What screen readers say after its name, instead of the hint. An error or a note still
    /// comes first.
    pub fn accessible_description(mut self, text: impl Into<String>) -> Self {
        self.accessible_description = Some(text.into());
        self
    }

    /// Marks the label with `*` and adds the [`validate::required`] rule (for the value set by [`FormField::validate`]).
    pub fn required(mut self) -> Self {
        self.label = self.label.required();
        self.required = true;
        self
    }

    /// What is wrong, as something else found out, such as rules that read a database.
    pub fn error_text(mut self, error: Option<String>) -> Self {
        self.told_error = error;
        self
    }

    /// The color of its label, instead of the usual one.
    pub fn label_ink(mut self, ink: Semantic) -> Self {
        self.label = self.label.ink(ink);
        self
    }

    /// A faint color behind the field, to call attention to it.
    pub fn tint(mut self, fill: Semantic) -> Self {
        self.fill = Some(fill);
        self
    }

    /// Something to know under the content, with an icon, which is not an error. An error replaces it.
    pub fn note_text(mut self, note: Option<String>, tone: crate::Tone) -> Self {
        self.told_note = note.map(|note| (note, tone));
        self
    }

    /// A muted caption under the content.
    pub fn hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    /// Validates `value` with `rule` (rules run in order; the first error shows).
    pub fn validate(
        mut self,
        value: Binding<String>,
        rule: impl Fn(&str) -> Result<(), String> + 'static,
    ) -> Self {
        self.value = Some(value);
        self.rules.push(Rc::new(rule));
        self
    }

    /// Shows errors even for empty values (e.g. after a submit attempt).
    pub fn show_errors(mut self, show: bool) -> Self {
        self.show_errors = show;
        self
    }

    /// The first error for the current value, if any.
    pub fn error(&self) -> Option<String> {
        let v = self.value.as_ref()?.get();
        if self.required
            && let Err(e) = validate::required(&v)
        {
            return Some(e);
        }
        self.rules.iter().find_map(|r| r(&v).err())
    }
}

impl<V: View> View for FormField<V> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let label = cx.child(&self.label);
        let content = cx.child(&self.content);
        let mut children = vec![label, content];
        let value = self.value.as_ref().map(Binding::get).unwrap_or_default();
        let blank = self.required && self.value.is_some() && value.trim().is_empty();
        // The same check as `error`, in the words of this part of the app.
        let error = self.told_error.clone().or_else(|| {
            (self.show_errors || !value.is_empty())
                .then(|| if blank { Some(cx.labels().required.clone()) } else { self.error() })
                .flatten()
        });
        let note = self.told_note.as_ref().map(|(note, _)| note.as_str());
        cx.label_control(content, self.label.text(), error.as_deref().or(note).or(self.hint.as_deref()));
        if self.accessible_name.is_some() || self.accessible_description.is_some() {
            let told = error.as_deref().or(note);
            let description = match (told, self.accessible_description.as_deref()) {
                (Some(told), Some(own)) => Some(format!("{told}. {own}")),
                (None, Some(own)) => Some(own.to_owned()),
                (_, None) => None,
            };
            cx.rename_control(content, self.accessible_name.as_deref(), description.as_deref());
        }
        if let Some(e) = error {
            let icon = util::icon(cx, IconName::CircleAlert, 14.0, theme.color(Semantic::Danger));
            let text = util::label(cx, &e, TextStyle::Callout, None, theme.color(Semantic::Danger));
            children.push(cx.container(
                LayoutStyle { spacing: 6.0, align: Align::Center, ..LayoutStyle::stack(Axis::Horizontal) },
                &[icon, text],
                Paint::None,
            ));
        } else if let Some((note, tone)) = &self.told_note {
            let color = tone.color(&theme);
            let shape = if *tone == crate::Tone::Warning { IconName::TriangleAlert } else { IconName::Info };
            let icon = util::icon(cx, shape, 14.0, color);
            let text = util::label(cx, note, TextStyle::Callout, None, theme.color(Semantic::InkMuted));
            children.push(cx.container(
                LayoutStyle { spacing: 6.0, align: Align::Center, ..LayoutStyle::stack(Axis::Horizontal) },
                &[icon, text],
                Paint::None,
            ));
        } else if let Some(h) = &self.hint {
            children.push(util::label(cx, h, TextStyle::Callout, None, theme.color(Semantic::InkMuted)));
        }
        let (padding, paint) = match self.fill {
            Some(fill) => (
                Insets::uniform(6.0),
                Paint::Quad(QuadPaint {
                    corner_radius: theme.radii.md,
                    ..QuadPaint::new(theme.color(fill).with_alpha(0.14))
                }),
            ),
            None => (Insets::ZERO, Paint::None),
        };
        cx.container(
            LayoutStyle {
                spacing: 6.0,
                padding,
                align: Align::Stretch,
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &children,
            paint,
        )
    }
}

/// A control with a name and a description of its own for screen readers, for one that has no
/// label around it to take them from, such as a check box that says its own caption.
///
/// ```
/// use ravenrs_core::Role;
/// use ravenrs_reactive::Binding;
/// use ravenrs_widgets::{AccessibleControl, Checkbox, Ui};
///
/// let check = Checkbox::new("Ativo", Binding::constant(true));
/// let named = AccessibleControl::new(check).name("Conta ativa").description("Aparece nos lançamentos");
/// let mut ui = Ui::new(named, ravenrs_style::Theme::raven_dark());
/// ui.update();
/// let tree = ui.access_tree();
/// let node = tree.find(Role::CheckBox, "Conta ativa").expect("renamed");
/// assert_eq!(node.description.as_deref(), Some("Aparece nos lançamentos"));
/// ```
#[derive(Debug)]
pub struct AccessibleControl<V> {
    content: V,
    name: Option<String>,
    description: Option<String>,
}

impl<V: View> AccessibleControl<V> {
    /// `content`, as it calls itself.
    pub fn new(content: V) -> Self {
        Self { content, name: None, description: None }
    }

    /// What screen readers call it.
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// What screen readers say after its name.
    pub fn description(mut self, text: impl Into<String>) -> Self {
        self.description = Some(text.into());
        self
    }
}

impl<V: View> View for AccessibleControl<V> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let node = cx.child(&self.content);
        cx.rename_control(node, self.name.as_deref(), self.description.as_deref());
        node
    }
}

#[cfg(test)]
mod tests {
    use super::validate;

    #[test]
    fn validators() {
        assert!(validate::required("  ").is_err());
        assert!(validate::email("ana@raven.rs").is_ok());
        assert!(validate::email("ana@raven").is_err());
        assert!(validate::email("").is_ok(), "emptiness is required()'s job");
        assert!(validate::min_length(8)("short").is_err());
        assert!(validate::min_length(8)("long enough").is_ok());
    }
}
