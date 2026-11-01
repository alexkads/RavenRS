//! Implicit animation: a view declares how a node moves, and the runtime plays
//! it during paint.
//!
//! Tracks live here, outside the view tree, keyed by identity. A build only sets
//! targets. Frames in between run the paint phase alone, so an animation does
//! not rebuild views or lay them out again.

use std::collections::HashMap;
use std::ops::Range;

use ravenrs_animation::{Animated, Transition};
use ravenrs_core::{Border, Brush, Color, Effect, Point, Primitive, Rect, Shadow};
use ravenrs_layout::{Axis, LayoutStyle};

use crate::context::{BuildCx, NodeId, Paint, QuadPaint, WidgetId};
use crate::{AnyView, View};

/// How a node animates. Attach it with [`BuildCx::motion`].
///
/// ```
/// use ravenrs_animation::Transition;
/// use ravenrs_core::Effect;
/// use ravenrs_widgets::Motion;
///
/// // Colors ease on hover; the node pops in and fades out.
/// let motion = Motion::new()
///     .paint(Transition::fast())
///     .enter(Effect::hidden().with_scale(0.96), Transition::snappy())
///     .exit(Effect::hidden(), Transition::fast());
/// ```
#[derive(Clone, Copy, Debug)]
pub struct Motion {
    pub(crate) paint: Option<Transition>,
    pub(crate) layout: Option<Transition>,
    pub(crate) shape_only: bool,
    pub(crate) underlay: bool,
    pub(crate) shared: Option<(WidgetId, u64)>,
    pub(crate) effect: (Effect, Transition),
    pub(crate) enter: Option<(Effect, Transition)>,
    pub(crate) exit: Option<(Effect, Transition)>,
    pub(crate) key: Option<u64>,
    pub(crate) anchored: bool,
}

impl Default for Motion {
    fn default() -> Self {
        Self::new()
    }
}

impl Motion {
    /// Nothing animates yet.
    pub const fn new() -> Self {
        Self {
            paint: None,
            layout: None,
            shape_only: false,
            underlay: false,
            shared: None,
            effect: (Effect::IDENTITY, Transition::Instant),
            enter: None,
            exit: None,
            key: None,
            anchored: false,
        }
    }

    /// Eases this node's own fill, border, shadow, corner radius and text color
    /// whenever a build changes them, like CSS `transition`. Colors travel
    /// through OKLab. Gradients and `Paint::Custom` change at once.
    pub const fn paint(mut self, transition: Transition) -> Self {
        self.paint = Some(transition);
        self
    }

    /// Glides the node from its old frame to its new one when layout moves it.
    /// Its children travel with it. Input keeps using the final frame.
    pub const fn layout(mut self, transition: Transition) -> Self {
        self.layout = Some(transition);
        self
    }

    /// Makes this node the same visual object as the node of a previous build
    /// with the same `scope` and `tag`, wherever it is now: a selection marker
    /// that slides from the old tab to the new one. `scope` is usually the
    /// widget id of the common parent.
    pub const fn shared_layout(mut self, scope: WidgetId, tag: u64, transition: Transition) -> Self {
        self.shared = Some((scope, tag));
        self.layout = Some(transition);
        self
    }

    /// With [`Self::layout`], moves only the node's own shape. Its children stay
    /// where layout put them.
    pub const fn shape_only(mut self) -> Self {
        self.shape_only = true;
        self
    }

    /// Paints the node's own shape behind all its siblings, so a moving marker
    /// slides under the labels it crosses. The node must be a direct child of
    /// the container holding those siblings.
    pub const fn underlay(mut self) -> Self {
        self.underlay = true;
        self
    }

    /// Eases the node and its subtree to this effect, such as a slight shrink
    /// while pressed. Layout and hit testing are unaffected.
    pub const fn effect(mut self, effect: Effect, transition: Transition) -> Self {
        self.effect = (effect, transition);
        self
    }

    /// When the node first appears, it starts from this effect.
    pub const fn enter(mut self, from: Effect, transition: Transition) -> Self {
        self.enter = Some((from, transition));
        self
    }

    /// When the node stops being built, its last picture stays on screen and
    /// plays this effect. The picture takes no input and is hidden from
    /// assistive technology: for the app, the node is already gone. Reopening
    /// during the exit reverses it from where it is.
    pub const fn exit(mut self, to: Effect, transition: Transition) -> Self {
        self.exit = Some((to, transition));
        self
    }

    /// The node's identity among the motions of its view. Defaults to call
    /// order, so use it for nodes built in a loop whose items come and go.
    pub const fn key(mut self, key: u64) -> Self {
        self.key = Some(key);
        self
    }

    /// For the root of an overlay: scales around the point nearest its anchor,
    /// and a vertical offset flips when the overlay opened on the other side.
    pub const fn anchored(mut self) -> Self {
        self.anchored = true;
        self
    }
}

/// The identity of an animated visual object.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum MotionKey {
    Node(WidgetId, u64),
    Shared(WidgetId, u64),
}

/// One [`BuildCx::motion`] call of the latest build.
#[derive(Clone, Copy, Debug)]
pub(crate) struct MotionEntry {
    pub node: NodeId,
    pub key: MotionKey,
    pub motion: Motion,
}

#[derive(Debug)]
struct PaintTrack {
    fill: Animated<Color>,
    radius: Animated<f32>,
    border_color: Animated<Color>,
    border_width: Animated<f32>,
    shadow_color: Animated<Color>,
    shadow_shape: Animated<Rect>,
}

impl PaintTrack {
    fn new(q: &QuadPaint) -> Self {
        let (border, shadow) = (border_of(q, None), shadow_of(q, None));
        Self {
            fill: Animated::new(solid(q.fill)),
            radius: Animated::new(q.corner_radius),
            border_color: Animated::new(border.color),
            border_width: Animated::new(border.width),
            shadow_color: Animated::new(shadow.color),
            shadow_shape: Animated::new(shadow_rect(&shadow)),
        }
    }

    fn set_target(&mut self, q: &QuadPaint, t: Transition, now: f64) {
        let was_border = self.border_width.target();
        let was_shadow = self.shadow_shape.target();
        let (border, shadow) = (border_of(q, Some(was_border)), shadow_of(q, Some(was_shadow)));
        self.fill.set_target(solid(q.fill), t, now);
        self.radius.set_target(q.corner_radius, t, now);
        self.border_color.set_target(border.color, t, now);
        self.border_width.set_target(border.width, t, now);
        self.shadow_color.set_target(shadow.color, t, now);
        self.shadow_shape.set_target(shadow_rect(&shadow), t, now);
    }

    fn sample(&self, q: &QuadPaint, now: f64) -> QuadPaint {
        let fill = match q.fill {
            Brush::Solid(_) => Brush::Solid(self.fill.value(now)),
            other => other,
        };
        let border_color = self.border_color.value(now);
        let border = (border_color.a > 0.003)
            .then(|| Border { width: self.border_width.value(now).max(0.0), color: border_color });
        let shadow_color = self.shadow_color.value(now);
        let shape = self.shadow_shape.value(now);
        let shadow = (shadow_color.a > 0.003).then_some(Shadow {
            offset: shape.origin,
            blur: shape.width(),
            color: shadow_color,
        });
        QuadPaint { fill, corner_radius: self.radius.value(now).max(0.0), border, shadow }
    }

    fn step(&mut self, now: f64) -> bool {
        // Not short-circuited: every property forgets what it has finished.
        self.fill.step(now)
            | self.radius.step(now)
            | self.border_color.step(now)
            | self.border_width.step(now)
            | self.shadow_color.step(now)
            | self.shadow_shape.step(now)
    }
}

fn solid(brush: Brush) -> Color {
    match brush {
        Brush::Solid(c) => c,
        Brush::LinearGradient(g) => g.to,
        _ => Color::TRANSPARENT,
    }
}

/// A missing border fades out where it was instead of shrinking from a default.
fn border_of(q: &QuadPaint, was_width: Option<f32>) -> Border {
    q.border.unwrap_or(Border { width: was_width.unwrap_or(1.0), color: solid(q.fill).with_alpha(0.0) })
}

fn shadow_of(q: &QuadPaint, was: Option<Rect>) -> Shadow {
    q.shadow.unwrap_or_else(|| {
        let was = was.unwrap_or(Rect::ZERO);
        Shadow { offset: was.origin, blur: was.width(), color: Color::TRANSPARENT }
    })
}

fn shadow_rect(s: &Shadow) -> Rect {
    Rect::new(s.offset.x, s.offset.y, s.blur, 0.0)
}

/// What the node looked like the last time it was painted, kept for its exit.
#[derive(Debug, Default)]
struct Snapshot {
    primitives: Vec<Primitive>,
    nested: Vec<NestedLayer>,
    clip: Rect,
    pivot: Point,
    flip: bool,
    layer: GhostLayer,
    valid: bool,
}

/// A part of a snapshot with an exit of its own, such as a dialog's card inside
/// its backdrop.
#[derive(Clone, Debug)]
pub(crate) struct NestedLayer {
    range: Range<usize>,
    pivot: Point,
    exit: (Effect, Transition),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum GhostLayer {
    /// Between the main tree and the overlays.
    #[default]
    Main,
    /// Above the overlays.
    Overlay,
    /// Above everything: the old picture during a theme change.
    Top,
}

#[derive(Debug)]
struct Track {
    seen: u64,
    effect: Animated<Effect>,
    paint: Option<PaintTrack>,
    text: Option<Animated<Color>>,
    rect: Option<Animated<Rect>>,
    exit: Option<(Effect, Transition)>,
    snapshot: Snapshot,
}

#[derive(Debug)]
struct Ghost {
    key: Option<MotionKey>,
    primitives: Vec<Primitive>,
    clip: Rect,
    pivot: Point,
    layer: GhostLayer,
    effect: Animated<Effect>,
    nested: Vec<(Range<usize>, Point, Animated<Effect>)>,
}

/// The values to paint a node with this frame.
#[derive(Debug)]
pub(crate) struct Sampled {
    pub quad: Option<QuadPaint>,
    pub text: Option<Color>,
    /// The node's frame on screen, when it differs from layout.
    pub frame: Option<Rect>,
    pub effect: Effect,
}

/// Every running animation of a [`Ui`](crate::Ui).
#[derive(Debug, Default)]
pub(crate) struct MotionSystem {
    tracks: HashMap<MotionKey, Track>,
    ghosts: Vec<Ghost>,
    build: u64,
    /// The latest paint found something still moving.
    pub active: bool,
    /// The animation clock, in seconds (see `Ui::set_time`).
    pub clock: f64,
    /// A platform drives the clock. Without one nothing could ever finish, so
    /// everything lands at once; this is also what makes headless tests stable.
    pub enabled: bool,
    pub reduced: bool,
    /// Nested exits met while painting the subtree of an outer exit.
    capture_depth: u32,
    nested: Vec<NestedLayer>,
}

impl MotionSystem {
    /// Applies reduced motion: nothing travels, but things may still fade.
    fn adapt(&self, effect: Effect, t: Transition) -> (Effect, Transition) {
        if self.reduced {
            (Effect { opacity: effect.opacity, ..Effect::IDENTITY }, t.without_bounce())
        } else {
            (effect, t)
        }
    }

    /// Records the targets of a build. `snap` lands colors at once (the theme
    /// changed, and the whole picture crossfades instead).
    pub fn sync(&mut self, entries: &[MotionEntry], paints: impl Fn(NodeId) -> NodePaint, snap: bool) {
        self.build += 1;
        let (now, build) = (self.clock, self.build);
        let mut started = !self.tracks.is_empty() || self.build > 1;
        started &= self.enabled;
        for entry in entries {
            let m = &entry.motion;
            let (target, target_t) = self.adapt(m.effect.0, m.effect.1);
            let track = match self.tracks.get_mut(&entry.key) {
                Some(track) => {
                    let t = if self.enabled { target_t } else { Transition::Instant };
                    track.effect.set_target(target, t, now);
                    track
                }
                None => {
                    let revived = self.ghosts.iter().position(|g| g.key == Some(entry.key));
                    let (from, t) = match (revived, m.enter) {
                        (Some(i), _) => (self.ghosts.swap_remove(i).effect.value(now), m.enter.map(|e| e.1)),
                        (None, Some((from, t))) if started => {
                            let (from, t) = self.adapt(from, t);
                            (from, Some(t))
                        }
                        _ => (target, None),
                    };
                    let mut effect = Animated::new(from);
                    effect.set_target(target, t.unwrap_or(target_t), now);
                    self.tracks.entry(entry.key).or_insert(Track {
                        seen: build,
                        effect,
                        paint: None,
                        text: None,
                        rect: None,
                        exit: None,
                        snapshot: Snapshot::default(),
                    })
                }
            };
            track.seen = build;
            track.exit = m.exit.map(|(e, t)| {
                if self.reduced {
                    (Effect { opacity: e.opacity, ..Effect::IDENTITY }, t.without_bounce())
                } else {
                    (e, t)
                }
            });
            if let Some(t) = m.paint {
                let t = if snap || !self.enabled {
                    Transition::Instant
                } else if self.reduced {
                    t.without_bounce()
                } else {
                    t
                };
                match paints(entry.node) {
                    NodePaint::Quad(q) => match &mut track.paint {
                        Some(p) => p.set_target(&q, t, now),
                        None => track.paint = Some(PaintTrack::new(&q)),
                    },
                    NodePaint::Text(c) => match &mut track.text {
                        Some(a) => a.set_target(c, t, now),
                        None => track.text = Some(Animated::new(c)),
                    },
                    NodePaint::Other => {}
                }
            }
        }
        // Whatever was not built starts its exit, from the picture it left.
        let gone: Vec<MotionKey> =
            self.tracks.iter().filter(|(_, t)| t.seen != build).map(|(k, _)| *k).collect();
        for key in gone {
            let Some(track) = self.tracks.remove(&key) else { continue };
            let Some((to, t)) = track.exit else { continue };
            if !self.enabled || !track.snapshot.valid {
                continue;
            }
            let from = track.effect.value(now);
            let snapshot = track.snapshot;
            let flip = snapshot.flip;
            let mut effect = Animated::new(from);
            effect.set_target(flipped(to, flip), t, now);
            let nested = snapshot.nested.iter().map(|n| {
                let mut e = Animated::new(Effect::IDENTITY);
                let (to, t) = self.adapt(n.exit.0, n.exit.1);
                e.set_target(to, t, now);
                (n.range.clone(), n.pivot, e)
            });
            let nested = nested.collect();
            self.ghosts.push(Ghost {
                key: Some(key),
                primitives: snapshot.primitives,
                clip: snapshot.clip,
                pivot: snapshot.pivot,
                layer: snapshot.layer,
                effect,
                nested,
            });
        }
    }

    /// Moves layout targets to the node's frame, measured without scrolling so
    /// that scrolling does not count as moving. `snap` lands them at once.
    pub fn sync_frame(&mut self, key: MotionKey, frame: Rect, t: Transition, snap: bool) {
        let t = if snap || !self.enabled || self.reduced { Transition::Instant } else { t };
        let now = self.clock;
        if let Some(track) = self.tracks.get_mut(&key) {
            match &mut track.rect {
                Some(rect) => rect.set_target(frame, t, now),
                None => track.rect = Some(Animated::new(frame)),
            }
        }
    }

    /// The node's values at this frame. `shift` is what scrolling added to its frame.
    pub fn sample(&mut self, key: MotionKey, paint: &NodePaint, shift: Point) -> Sampled {
        let now = self.clock;
        let Some(track) = self.tracks.get_mut(&key) else {
            return Sampled { quad: None, text: None, frame: None, effect: Effect::IDENTITY };
        };
        let mut moving = track.effect.step(now);
        let quad = match (&mut track.paint, paint) {
            (Some(p), NodePaint::Quad(q)) => {
                moving |= p.step(now);
                Some(p.sample(q, now))
            }
            _ => None,
        };
        let text = track.text.as_mut().map(|a| {
            moving |= a.step(now);
            a.value(now)
        });
        let frame = track.rect.as_mut().map(|a| {
            moving |= a.step(now);
            a.value(now).translate(shift)
        });
        self.active |= moving;
        Sampled { quad, text, frame, effect: track.effect.value(now) }
    }

    /// Starts recording a node's picture; returns false when nested in another exit.
    pub fn begin_capture(&mut self) -> bool {
        self.capture_depth += 1;
        self.capture_depth == 1
    }

    /// Stores the picture of an exit node painted at `range` of `scene`.
    #[allow(clippy::too_many_arguments)]
    pub fn end_capture(
        &mut self,
        key: MotionKey,
        primitives: &[Primitive],
        range: Range<usize>,
        clip: Rect,
        pivot: Point,
        flip: bool,
        layer: GhostLayer,
    ) {
        self.capture_depth = self.capture_depth.saturating_sub(1);
        let Some(track) = self.tracks.get_mut(&key) else { return };
        let Some(exit) = track.exit else { return };
        if self.capture_depth > 0 {
            self.nested.push(NestedLayer { range, pivot, exit });
            return;
        }
        let s = &mut track.snapshot;
        s.primitives.clear();
        s.primitives.extend_from_slice(primitives);
        s.nested.clear();
        s.nested.extend(
            self.nested
                .drain(..)
                .map(|n| NestedLayer { range: n.range.start - range.start..n.range.end - range.start, ..n }),
        );
        s.clip = clip;
        s.pivot = pivot;
        s.flip = flip;
        s.layer = layer;
        s.valid = true;
    }

    /// Fades out a whole picture: what the screen showed before a theme change.
    pub fn crossfade(&mut self, primitives: Vec<Primitive>, viewport: Rect, t: Transition) {
        let mut effect = Animated::new(Effect::IDENTITY);
        effect.set_target(Effect::hidden(), t, self.clock);
        self.ghosts.push(Ghost {
            key: None,
            primitives,
            clip: viewport,
            pivot: viewport.center(),
            layer: GhostLayer::Top,
            effect,
            nested: Vec::new(),
        });
    }

    /// Draws the exiting pictures of `layer` and forgets those that finished.
    pub fn paint_ghosts(&mut self, layer: GhostLayer, scene: &mut ravenrs_core::Scene) {
        let now = self.clock;
        let mut active = false;
        self.ghosts.retain_mut(|g| {
            if g.layer != layer {
                return true;
            }
            let mut moving = g.effect.step(now);
            for (_, _, e) in &mut g.nested {
                moving |= e.step(now);
            }
            if !moving {
                return false;
            }
            active = true;
            scene.push_clip(g.clip);
            let start = scene.len();
            scene.extend_from_slice(&g.primitives);
            for (range, pivot, e) in &g.nested {
                let r = start + range.start..start + range.end;
                scene.apply_effect(r, e.value(now), *pivot);
            }
            scene.apply_effect(start..scene.len(), g.effect.value(now), g.pivot);
            scene.pop_clip();
            true
        });
        self.active |= active;
    }
}

/// An overlay on the far side of its anchor exits toward it the other way.
pub(crate) fn flipped(effect: Effect, flip: bool) -> Effect {
    if flip { Effect { offset: Point::new(effect.offset.x, -effect.offset.y), ..effect } } else { effect }
}

/// The animatable part of what a node paints.
#[derive(Clone, Copy, Debug)]
pub(crate) enum NodePaint {
    Quad(QuadPaint),
    Text(Color),
    Other,
}

impl NodePaint {
    pub(crate) fn of(paint: &Paint, text: Option<Color>) -> Self {
        match (paint, text) {
            (Paint::Quad(q), _) => Self::Quad(*q),
            (_, Some(c)) => Self::Text(c),
            _ => Self::Other,
        }
    }
}

/// Animates its content's height as it opens and closes: the "height: auto"
/// transition. The content stays built until it has finished closing.
pub struct Collapse {
    open: bool,
    content: AnyView,
}

impl std::fmt::Debug for Collapse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Collapse").field("open", &self.open).finish_non_exhaustive()
    }
}

impl Collapse {
    /// Shows `content` at full height while `open` is true, and nothing otherwise.
    pub fn new(open: bool, content: impl View + 'static) -> Self {
        Self { open, content: Box::new(content) }
    }

    /// The same, for content already boxed.
    pub fn boxed(open: bool, content: AnyView) -> Self {
        Self { open, content }
    }
}

impl View for Collapse {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        collapse(cx, self.open, self.content.as_ref())
    }
}

pub(crate) fn collapse(cx: &mut BuildCx<'_>, open: bool, content: &dyn View) -> NodeId {
    struct Body<'a>(bool, &'a dyn View);
    impl View for Body<'_> {
        fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
            let transition = cx.theme().motion.gentle;
            let target = if self.0 { 1.0_f32 } else { 0.0 };
            let progress = cx.animated(target, transition).clamp(0.0, 1.0);
            let at_rest = (progress - target).abs() < f32::EPSILON;
            if at_rest && !self.0 {
                return cx.empty();
            }
            let child = cx.child(self.1);
            let natural = cx.measured_content().map_or(0.0, |s| s.height);
            let style = LayoutStyle {
                height: (!at_rest).then_some(natural * progress),
                overflow_hidden: !at_rest,
                align: ravenrs_layout::Align::Stretch,
                ..LayoutStyle::stack(Axis::Vertical)
            };
            let node = cx.container(style, &[child], Paint::None);
            cx.measure_content(node);
            if !at_rest {
                cx.clip(node);
            }
            node
        }
    }
    cx.child(&Body(open, content))
}

/// How floating panels appear: from the side of their anchor.
pub(crate) fn popup(cx: &mut BuildCx<'_>, node: NodeId) {
    let m = cx.theme().motion;
    cx.motion(
        node,
        Motion::new()
            .anchored()
            .enter(Effect::hidden().with_scale(0.96), m.snappy)
            .exit(Effect::hidden().with_scale(0.98), m.fast),
    );
}

/// How the dimmed backdrop of a modal layer comes and goes.
pub(crate) fn scrim(cx: &mut BuildCx<'_>, node: NodeId) {
    let m = cx.theme().motion;
    cx.motion(node, Motion::new().enter(Effect::hidden(), m.base).exit(Effect::hidden(), m.base));
}

/// How a control acknowledges hover and press.
pub(crate) fn control(cx: &mut BuildCx<'_>, node: NodeId) {
    let t = cx.theme().motion.fast;
    cx.motion(node, Motion::new().paint(t));
}
