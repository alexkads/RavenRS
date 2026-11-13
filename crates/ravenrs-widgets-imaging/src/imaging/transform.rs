//! A box you can move, resize and turn, the crop built on it, and the band that sets an angle.
//!
//! The renderer has no transforms, so none of this turns a picture: these widgets draw and hand
//! back geometry, and applying it is up to whoever owns the pixels.

use std::cell::Cell;
use std::fmt;
use std::rc::Rc;

use ravenrs_core::{
    Color, CursorIcon, FillRule, Insets, Key, Modifiers, PathElement, PathFill, Point, Quad, Rect, Role,
    Scene, Size,
};
use ravenrs_layout::{Align, Axis, Justify, LayoutStyle};
use ravenrs_reactive::{Binding, Observer};
use ravenrs_style::{Semantic, TextStyle};

use super::overlays::{
    CompositionGuide, ContentMap, ContentSpace, OverlayBox, corners_of, gesture_paint, overlay_builders,
    paint_guide, picture_inks,
};
use super::stroke_outline;
use crate::View;
use crate::context::{BuildCx, DragPhase, NodeId, Paint, Semantics};
use crate::util;

// ---------------------------------------------------------------------------------------------
// Geometry
// ---------------------------------------------------------------------------------------------

/// A rectangle turned around a point, which is what a [`TransformHandles`] edits.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct BoxTransform {
    /// The box before turning, in the units of the content.
    pub rect: Rect,
    /// How far it is turned clockwise, in degrees.
    pub angle: f32,
    /// The point it turns around, as a share of the box: `(0.5, 0.5)` is its middle.
    pub pivot: Point,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl BoxTransform {
    /// An upright box that turns around its middle.
    pub const fn new(rect: Rect) -> Self {
        Self { rect, angle: 0.0, pivot: Point::new(0.5, 0.5) }
    }

    /// The same box turned clockwise by this many degrees.
    pub const fn rotated(mut self, angle: f32) -> Self {
        self.angle = angle;
        self
    }

    /// The point it turns around, in the units of the content.
    pub fn pivot_point(&self) -> Point {
        self.at_share(self.pivot.x, self.pivot.y)
    }

    fn at_share(&self, x: f32, y: f32) -> Point {
        Point::new(self.rect.x() + self.rect.width() * x, self.rect.y() + self.rect.height() * y)
    }

    /// Where a point of the upright box ends up once turned.
    pub fn to_world(&self, local: Point) -> Point {
        let p = self.pivot_point();
        let (sin, cos) = self.angle.to_radians().sin_cos();
        let (x, y) = (local.x - p.x, local.y - p.y);
        Point::new(p.x + x * cos - y * sin, p.y + x * sin + y * cos)
    }

    /// The point of the upright box that ends up at `world` once turned.
    pub fn to_local(&self, world: Point) -> Point {
        let p = self.pivot_point();
        let (sin, cos) = self.angle.to_radians().sin_cos();
        let (x, y) = (world.x - p.x, world.y - p.y);
        Point::new(p.x + x * cos + y * sin, p.y - x * sin + y * cos)
    }

    /// The four corners once turned, clockwise from what was the top left.
    pub fn corners(&self) -> [Point; 4] {
        corners_of(self.rect).map(|p| self.to_world(p))
    }

    /// Where a handle is once turned.
    pub fn handle(&self, handle: BoxHandle) -> Point {
        let (x, y) = handle.share();
        self.to_world(self.at_share(x, y))
    }

    fn is_inside(&self, bounds: Rect) -> bool {
        const SLACK: f32 = 0.01;
        self.corners().iter().all(|c| {
            c.x >= bounds.x() - SLACK
                && c.x <= bounds.max_x() + SLACK
                && c.y >= bounds.y() - SLACK
                && c.y <= bounds.max_y() + SLACK
        })
    }

    /// Moved the least it takes to be inside `bounds`.
    fn moved_inside(mut self, bounds: Rect) -> Self {
        let corners = self.corners();
        let low = |pick: fn(&Point) -> f32| corners.iter().map(pick).fold(f32::INFINITY, f32::min);
        let high = |pick: fn(&Point) -> f32| corners.iter().map(pick).fold(f32::NEG_INFINITY, f32::max);
        let axis = |low: f32, high: f32, from: f32, to: f32| {
            if high - low > to - from {
                (from + to - low - high) / 2.0
            } else if low < from {
                from - low
            } else if high > to {
                to - high
            } else {
                0.0
            }
        };
        let dx = axis(low(|p| p.x), high(|p| p.x), bounds.x(), bounds.max_x());
        let dy = axis(low(|p| p.y), high(|p| p.y), bounds.y(), bounds.max_y());
        self.rect = self.rect.translate(Point::new(dx, dy));
        self
    }

    /// Made smaller around its middle until it is inside `bounds`, for a box that turns around
    /// its middle. A crop does this as it is turned, the way a frame must over a still picture.
    fn shrunk_inside(mut self, bounds: Rect) -> Self {
        let c = self.rect.center();
        let mut share = 1.0_f32;
        for corner in self.corners() {
            let (dx, dy) = (corner.x - c.x, corner.y - c.y);
            if dx > 0.0 {
                share = share.min((bounds.max_x() - c.x) / dx);
            } else if dx < 0.0 {
                share = share.min((bounds.x() - c.x) / dx);
            }
            if dy > 0.0 {
                share = share.min((bounds.max_y() - c.y) / dy);
            } else if dy < 0.0 {
                share = share.min((bounds.y() - c.y) / dy);
            }
        }
        let share = share.clamp(0.0, 1.0);
        let (w, h) = (self.rect.width() * share, self.rect.height() * share);
        self.rect = Rect::new(c.x - w / 2.0, c.y - h / 2.0, w, h);
        self
    }
}

/// One of the eight handles of a [`TransformHandles`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "variants"))]
pub enum BoxHandle {
    /// The top left corner.
    TopLeft,
    /// The middle of the top side.
    Top,
    /// The top right corner.
    TopRight,
    /// The middle of the right side.
    Right,
    /// The bottom right corner.
    BottomRight,
    /// The middle of the bottom side.
    Bottom,
    /// The bottom left corner.
    BottomLeft,
    /// The middle of the left side.
    Left,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl BoxHandle {
    /// All of them, clockwise from the top left.
    pub const ALL: [Self; 8] = [
        Self::TopLeft,
        Self::Top,
        Self::TopRight,
        Self::Right,
        Self::BottomRight,
        Self::Bottom,
        Self::BottomLeft,
        Self::Left,
    ];

    /// Where it sits, as a share of the box.
    pub const fn share(self) -> (f32, f32) {
        match self {
            Self::TopLeft => (0.0, 0.0),
            Self::Top => (0.5, 0.0),
            Self::TopRight => (1.0, 0.0),
            Self::Right => (1.0, 0.5),
            Self::BottomRight => (1.0, 1.0),
            Self::Bottom => (0.5, 1.0),
            Self::BottomLeft => (0.0, 1.0),
            Self::Left => (0.0, 0.5),
        }
    }

    const fn is_corner(self) -> bool {
        matches!(self, Self::TopLeft | Self::TopRight | Self::BottomRight | Self::BottomLeft)
    }
}

/// How a box answers a resize.
#[derive(Clone, Copy, Debug)]
struct Resize {
    handle: BoxHandle,
    /// Width over height to hold.
    ratio: Option<f32>,
    around_pivot: bool,
    least: Size,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl Resize {
    /// `start` with its handle moved by this much along the box's own axes. The opposite handle,
    /// or the pivot, stays where it was on screen.
    fn apply(&self, start: BoxTransform, du: f32, dv: f32) -> BoxTransform {
        let (fx, fy) = self.handle.share();
        let (sx, sy) = (fx * 2.0 - 1.0, fy * 2.0 - 1.0);
        let (ax, ay) = if self.around_pivot { (start.pivot.x, start.pivot.y) } else { (1.0 - fx, 1.0 - fy) };
        // Around a pivot a handle moves less than the side does, in proportion to how far apart
        // they are. A pivot on the handle would make that endless.
        let reach = |at: f32, from: f32| (at - from).abs().max(0.1);
        let (w0, h0) = (start.rect.width(), start.rect.height());
        let mut w = w0 + sx * du / reach(fx, ax);
        let mut h = h0 + sy * dv / reach(fy, ay);
        if let Some(ratio) = self.ratio {
            if self.handle.is_corner() {
                let grow = (w / w0.max(f32::EPSILON)).max(h / h0.max(f32::EPSILON));
                w = w0 * grow;
                h = w / ratio;
            } else if sx != 0.0 {
                h = w / ratio;
            } else {
                w = h * ratio;
            }
            let hold = (self.least.width / w.max(f32::EPSILON)).max(self.least.height / h.max(f32::EPSILON));
            if hold > 1.0 {
                w *= hold;
                h *= hold;
            }
        } else {
            w = w.max(self.least.width);
            h = h.max(self.least.height);
        }
        // Where the fixed point must stay, and the rectangle that puts it there at the new size.
        let fixed = start.to_world(start.at_share(ax, ay));
        let (sin, cos) = start.angle.to_radians().sin_cos();
        let (x, y) = ((ax - start.pivot.x) * w, (ay - start.pivot.y) * h);
        let pivot = Point::new(fixed.x - (x * cos - y * sin), fixed.y - (x * sin + y * cos));
        BoxTransform {
            rect: Rect::new(pivot.x - start.pivot.x * w, pivot.y - start.pivot.y * h, w, h),
            ..start
        }
    }

    /// Like [`Self::apply`], as far as it goes while inside `bounds`. Without a ratio each axis
    /// stops on its own, so a handle slides along an edge it reached.
    fn confined(&self, start: BoxTransform, du: f32, dv: f32, bounds: Rect) -> BoxTransform {
        let wanted = self.apply(start, du, dv);
        if wanted.is_inside(bounds) || !start.is_inside(bounds) {
            return wanted;
        }
        let furthest = |try_at: &dyn Fn(f32) -> BoxTransform| {
            let (mut inside, mut outside) = (0.0_f32, 1.0_f32);
            if try_at(1.0).is_inside(bounds) {
                return 1.0;
            }
            for _ in 0..14 {
                let middle = (inside + outside) / 2.0;
                if try_at(middle).is_inside(bounds) {
                    inside = middle;
                } else {
                    outside = middle;
                }
            }
            inside
        };
        if self.ratio.is_some() {
            let t = furthest(&|t| self.apply(start, du * t, dv * t));
            return self.apply(start, du * t, dv * t).moved_inside(bounds);
        }
        let across = du * furthest(&|t| self.apply(start, du * t, 0.0));
        let down = dv * furthest(&|t| self.apply(start, across, dv * t));
        // The search stops a hair from the edge, on either side of it.
        self.apply(start, across, down).moved_inside(bounds)
    }
}

/// An angle in the range `-180..=180`.
fn wrapped(angle: f32) -> f32 {
    let turned = (angle + 180.0).rem_euclid(360.0) - 180.0;
    if turned == -180.0 { 180.0 } else { turned }
}

// ---------------------------------------------------------------------------------------------
// TransformHandles
// ---------------------------------------------------------------------------------------------

/// How far from a handle a press still takes it, in pixels: more than it looks, or pressing just
/// off it would do something else.
const HANDLE_REACH: f32 = 11.0;
/// How far from a side a press takes that side.
const EDGE_REACH: f32 = 5.0;
const HANDLE_SIDE: f32 = 8.0;
/// How far above the box the handle that turns it is.
const KNOB_STEM: f32 = 24.0;
const ROTATE_SNAP: f32 = 15.0;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Part {
    None,
    Handle(BoxHandle),
    Rotate,
    Pivot,
    Move,
}

#[derive(Clone, Copy, Debug)]
struct Rules {
    keep_corners: bool,
    aspect: Option<f32>,
    confined: bool,
    min_size: f32,
    knob: bool,
    pivot: bool,
    outside_rotates: bool,
    angle_limit: Option<f32>,
    /// Turning makes it smaller instead of stopping at the bounds.
    shrink_to_turn: bool,
}

#[derive(Clone, Copy, Debug)]
struct Look {
    veil: bool,
    guide: Option<CompositionGuide>,
    turning_guide: Option<CompositionGuide>,
}

#[derive(Clone, Copy, Debug)]
struct Grab {
    part: Part,
    pressed: Point,
    start: BoxTransform,
}

#[derive(Debug, Default)]
struct TransformMemory {
    frame: Cell<Rect>,
    pointer: Cell<Option<Point>>,
    grab: Cell<Option<Grab>>,
}

/// Where the handle that turns the box is, on screen.
fn knob_at(t: &BoxTransform, map: &ContentMap) -> Point {
    let top = map.screen(t.handle(BoxHandle::Top));
    let (sin, cos) = t.angle.to_radians().sin_cos();
    Point::new(top.x + sin * KNOB_STEM, top.y - cos * KNOB_STEM)
}

fn part_at(t: &BoxTransform, map: &ContentMap, at: Point, rules: Rules) -> Part {
    let near = |p: Point, reach: f32| (p.x - at.x).hypot(p.y - at.y) <= reach;
    if rules.pivot && near(map.screen(t.pivot_point()), HANDLE_REACH) {
        return Part::Pivot;
    }
    // The corners first: on a small box they win over the middles.
    for handle in BoxHandle::ALL
        .into_iter()
        .filter(|h| h.is_corner())
        .chain(BoxHandle::ALL.into_iter().filter(|h| !h.is_corner()))
    {
        if near(map.screen(t.handle(handle)), HANDLE_REACH) {
            return Part::Handle(handle);
        }
    }
    if rules.knob && near(knob_at(t, map), HANDLE_REACH) {
        return Part::Rotate;
    }
    let local = t.to_local(map.content(at));
    let reach = EDGE_REACH / map.scale.max(f32::EPSILON);
    let rect = t.rect;
    let across = local.x >= rect.x() - reach && local.x <= rect.max_x() + reach;
    let along = local.y >= rect.y() - reach && local.y <= rect.max_y() + reach;
    if across && (local.y - rect.y()).abs() <= reach {
        return Part::Handle(BoxHandle::Top);
    }
    if across && (local.y - rect.max_y()).abs() <= reach {
        return Part::Handle(BoxHandle::Bottom);
    }
    if along && (local.x - rect.x()).abs() <= reach {
        return Part::Handle(BoxHandle::Left);
    }
    if along && (local.x - rect.max_x()).abs() <= reach {
        return Part::Handle(BoxHandle::Right);
    }
    if rect.contains(local) {
        Part::Move
    } else if rules.outside_rotates {
        Part::Rotate
    } else {
        Part::None
    }
}

/// `CursorIcon` has no diagonal resize and no grab: sides turn as far as the two resize cursors
/// can show, and everything else that acts on a press uses the pointing hand.
fn cursor_of(part: Part, angle: f32) -> CursorIcon {
    match part {
        Part::None => CursorIcon::Default,
        Part::Handle(handle) if !handle.is_corner() => {
            let (sin, cos) = angle.to_radians().sin_cos();
            let sideways = matches!(handle, BoxHandle::Left | BoxHandle::Right);
            if (if sideways { cos } else { sin }).abs() >= std::f32::consts::FRAC_1_SQRT_2 {
                CursorIcon::ColResize
            } else {
                CursorIcon::RowResize
            }
        }
        Part::Handle(_) | Part::Rotate | Part::Pivot | Part::Move => CursorIcon::Pointer,
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl Rules {
    fn resize(&self, handle: BoxHandle, start: BoxTransform, scale: f32, modifiers: Modifiers) -> Resize {
        let least = self.min_size / scale.max(f32::EPSILON);
        let held = (self.keep_corners && handle.is_corner() && !modifiers.shift)
            .then(|| start.rect.width() / start.rect.height().max(f32::EPSILON));
        Resize {
            handle,
            ratio: self.aspect.or(held),
            around_pivot: modifiers.alt,
            least: Size::new(least, least),
        }
    }

    fn resized(
        &self,
        handle: BoxHandle,
        start: BoxTransform,
        du: f32,
        dv: f32,
        map: &ContentMap,
        modifiers: Modifiers,
    ) -> BoxTransform {
        let resize = self.resize(handle, start, map.scale, modifiers);
        if self.confined { resize.confined(start, du, dv, map.bounds()) } else { resize.apply(start, du, dv) }
    }

    fn moved(&self, start: BoxTransform, dx: f32, dy: f32, map: &ContentMap) -> BoxTransform {
        let t = BoxTransform { rect: start.rect.translate(Point::new(dx, dy)), ..start };
        if self.confined { t.moved_inside(map.bounds()) } else { t }
    }

    fn turned(&self, start: BoxTransform, angle: f32, map: &ContentMap) -> BoxTransform {
        let limit = self.angle_limit.unwrap_or(180.0);
        let t = BoxTransform { angle: wrapped(angle).clamp(-limit, limit), ..start };
        if !self.confined || t.is_inside(map.bounds()) {
            return t;
        }
        if self.shrink_to_turn {
            return self.made_to_fit(t, map.bounds(), map.scale);
        }
        let (mut inside, mut outside) = (0.0_f32, 1.0_f32);
        for _ in 0..14 {
            let middle = (inside + outside) / 2.0;
            let tried = BoxTransform { angle: start.angle + (t.angle - start.angle) * middle, ..start };
            if tried.is_inside(map.bounds()) {
                inside = middle;
            } else {
                outside = middle;
            }
        }
        BoxTransform { angle: start.angle + (t.angle - start.angle) * inside, ..start }
    }

    /// Made smaller until it fits, and moved first if that alone would leave almost nothing: a
    /// crop against the edge of the content would otherwise vanish as it turns.
    fn made_to_fit(&self, t: BoxTransform, bounds: Rect, scale: f32) -> BoxTransform {
        let least = self.min_size / scale.max(f32::EPSILON);
        let shrunk = t.shrunk_inside(bounds);
        if shrunk.rect.width() >= least && shrunk.rect.height() >= least {
            return shrunk;
        }
        t.moved_inside(bounds).shrunk_inside(bounds)
    }

    /// What a value set from outside becomes under these rules, for a crop whose proportion or
    /// angle was changed by another control.
    fn settled(&self, t: BoxTransform, bounds: Rect, scale: f32) -> BoxTransform {
        let limit = self.angle_limit.unwrap_or(180.0);
        let mut next = BoxTransform { angle: t.angle.clamp(-limit, limit), ..t };
        if let Some(ratio) = self.aspect
            && next.rect.height() > 0.0
            && (next.rect.width() / next.rect.height() - ratio).abs() > ratio * 0.005
        {
            let w = next.rect.width().min(next.rect.height() * ratio);
            let h = w / ratio;
            let c = next.rect.center();
            next.rect = Rect::new(c.x - w / 2.0, c.y - h / 2.0, w, h);
        }
        if next.is_inside(bounds) { next } else { self.made_to_fit(next, bounds, scale) }
    }

    /// What a drag has made of the box, from where it began.
    fn dragged(&self, grab: Grab, at: Point, map: &ContentMap, modifiers: Modifiers) -> BoxTransform {
        let start = grab.start;
        let (dx, dy) = (at.x - grab.pressed.x, at.y - grab.pressed.y);
        match grab.part {
            Part::None => start,
            Part::Move => self.moved(start, dx, dy, map),
            Part::Handle(handle) => {
                let (sin, cos) = start.angle.to_radians().sin_cos();
                self.resized(handle, start, dx * cos + dy * sin, -dx * sin + dy * cos, map, modifiers)
            }
            Part::Rotate => {
                let p = start.pivot_point();
                let toward = |q: Point| (q.y - p.y).atan2(q.x - p.x).to_degrees();
                let angle = start.angle + toward(at) - toward(grab.pressed);
                let angle = if modifiers.shift { (angle / ROTATE_SNAP).round() * ROTATE_SNAP } else { angle };
                self.turned(start, angle, map)
            }
            Part::Pivot => {
                // The box must stay where it is on screen with its turning point somewhere else,
                // which moves the upright rectangle it is told from.
                let local = start.to_local(at);
                let (w, h) = (start.rect.width().max(f32::EPSILON), start.rect.height().max(f32::EPSILON));
                let pivot = Point::new((local.x - start.rect.x()) / w, (local.y - start.rect.y()) / h);
                let rect = Rect::new(at.x - pivot.x * w, at.y - pivot.y * h, w, h);
                BoxTransform { rect, pivot, ..start }
            }
        }
    }

    fn on_key(
        &self,
        t: BoxTransform,
        key: &Key,
        modifiers: Modifiers,
        map: &ContentMap,
    ) -> Option<BoxTransform> {
        let step = (if modifiers.shift { 10.0 } else { 1.0 }) / map.scale.max(f32::EPSILON);
        let (dx, dy) = match key {
            Key::ArrowLeft => (-step, 0.0),
            Key::ArrowRight => (step, 0.0),
            Key::ArrowUp => (0.0, -step),
            Key::ArrowDown => (0.0, step),
            Key::Character(c)
                if (self.knob || self.outside_rotates) && matches!(c.as_str(), "[" | "]" | "{" | "}") =>
            {
                let turn = if modifiers.shift { ROTATE_SNAP } else { 1.0 };
                let turn = if matches!(c.as_str(), "[" | "{") { -turn } else { turn };
                return Some(self.turned(t, t.angle + turn, map));
            }
            _ => return None,
        };
        if !modifiers.alt {
            return Some(self.moved(t, dx, dy, map));
        }
        let handle = if dx != 0.0 { BoxHandle::Right } else { BoxHandle::Bottom };
        // The keys say how large, always from the side they name: Alt here is not around the pivot.
        Some(self.resized(handle, t, dx, dy, map, Modifiers::default()))
    }
}

/// Eight handles around a box, one that turns it and the point it turns around, drawn over
/// content in the units of a [`ContentSpace`]. It edits a [`BoxTransform`] and draws nothing but
/// the box: the renderer cannot turn a picture, so applying the geometry is up to whoever owns it.
///
/// A corner keeps the proportions and Shift lets go of them, Alt resizes around the pivot, and
/// Shift holds a turn to steps of 15 degrees. Escape cancels a drag, and a double click on the
/// pivot puts it back in the middle.
///
/// Keyboard: the arrows move it, with Alt they resize it, `[` and `]` turn it, and Shift makes
/// each of these go further.
///
/// `CursorIcon` has no diagonal cursors, so the corners show the pointing hand. While dragging
/// it paints without a build, so it costs no rebuilds if nothing else reads the value.
///
/// ```
/// use ravenrs_core::{Rect, Size};
/// use ravenrs_reactive::State;
/// use ravenrs_widgets::{BoxTransform, ContentSpace, TransformHandles};
///
/// let layer = State::new(BoxTransform::new(Rect::new(100.0, 80.0, 300.0, 200.0)).rotated(10.0));
/// let handles = TransformHandles::new(layer.binding())
///     .space(ContentSpace::Fit(Size::new(1200.0, 800.0)))
///     .size(600.0, 400.0);
/// ```
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct TransformHandles {
    value: Binding<BoxTransform>,
    frame: OverlayBox,
    rules: Rules,
    look: Look,
    on_commit: Option<Rc<dyn Fn(BoxTransform)>>,
    disabled: bool,
    label: Option<String>,
    crop_name: bool,
}

impl fmt::Debug for TransformHandles {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TransformHandles").field("frame", &self.frame).finish_non_exhaustive()
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl TransformHandles {
    /// Handles bound to `value`, in the units of its [space](Self::space).
    pub fn new(value: Binding<BoxTransform>) -> Self {
        Self {
            value,
            frame: OverlayBox::default(),
            rules: Rules {
                keep_corners: true,
                aspect: None,
                confined: false,
                min_size: 12.0,
                knob: true,
                pivot: true,
                outside_rotates: false,
                angle_limit: None,
                shrink_to_turn: false,
            },
            look: Look { veil: false, guide: None, turning_guide: None },
            on_commit: None,
            disabled: false,
            label: None,
            crop_name: false,
        }
    }

    /// Whether the corners keep the proportions unless Shift is held (they do by default).
    pub fn keep_proportions(mut self, keep: bool) -> Self {
        self.rules.keep_corners = keep;
        self
    }

    /// Holds this width over height for every handle.
    pub fn aspect(mut self, aspect: Option<f32>) -> Self {
        self.rules.aspect = aspect;
        self
    }

    /// Keeps the whole box inside the content (it may leave by default).
    pub fn confined(mut self, confined: bool) -> Self {
        self.rules.confined = confined;
        self
    }

    /// The smallest side, in logical pixels on screen at the current zoom (12 by default).
    pub fn min_size(mut self, pixels: f32) -> Self {
        self.rules.min_size = pixels.max(1.0);
        self
    }

    /// Whether it can be turned by the handle above it (it can by default).
    pub fn rotatable(mut self, rotatable: bool) -> Self {
        self.rules.knob = rotatable;
        self
    }

    /// Whether a drag outside the box turns it as well (it does not by default).
    pub fn rotate_outside(mut self, rotate: bool) -> Self {
        self.rules.outside_rotates = rotate;
        self
    }

    /// Whether the point it turns around shows and can be moved (it does by default).
    pub fn pivot_handle(mut self, show: bool) -> Self {
        self.rules.pivot = show;
        self
    }

    /// Runs `handler` with the result when a drag ends or a key changes it: one step to undo.
    pub fn on_commit(mut self, handler: impl Fn(BoxTransform) + 'static) -> Self {
        self.on_commit = Some(Rc::new(handler));
        self
    }

    /// Takes no input while disabled, which leaves the pointer to what is underneath.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// The name screen readers announce, instead of the one in [`Labels`](crate::Labels).
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }
}

overlay_builders!(TransformHandles, frame);

fn square(center: Point, side: f32, angle: f32) -> Vec<PathElement> {
    let half = side / 2.0;
    let (sin, cos) = angle.to_radians().sin_cos();
    let corner = |x: f32, y: f32| Point::new(center.x + x * cos - y * sin, center.y + x * sin + y * cos);
    vec![
        PathElement::MoveTo(corner(-half, -half)),
        PathElement::LineTo(corner(half, -half)),
        PathElement::LineTo(corner(half, half)),
        PathElement::LineTo(corner(-half, half)),
        PathElement::Close,
    ]
}

/// What a box looks like, apart from where it is.
#[derive(Clone, Copy, Debug)]
struct BoxStyle {
    rules: Rules,
    look: Look,
    light: Color,
    dark: Color,
    scrim: Color,
}

fn paint_box(
    scene: &mut Scene,
    frame: Rect,
    map: &ContentMap,
    t: &BoxTransform,
    style: BoxStyle,
    turning: bool,
) {
    let BoxStyle { rules, look, light, dark, scrim } = style;
    let corners = t.corners().map(|p| map.screen(p));
    let upright = t.angle.to_radians().sin().abs() < 0.001;
    let shown = map.screen_rect(t.rect);
    scene.push_clip(frame);
    if look.veil {
        let area = map.placed.intersect(&frame);
        let [a, b, c, d] = corners_of(area);
        let mut outline = vec![
            PathElement::MoveTo(a),
            PathElement::LineTo(b),
            PathElement::LineTo(c),
            PathElement::LineTo(d),
            PathElement::Close,
        ];
        outline.push(PathElement::MoveTo(corners[0]));
        outline.extend(corners[1..].iter().map(|p| PathElement::LineTo(*p)));
        outline.push(PathElement::Close);
        scene.push_path(PathFill { rule: FillRule::EvenOdd, ..PathFill::solid(outline, scrim) });
    }
    let guide = if turning { look.turning_guide.or(look.guide) } else { look.guide };
    if let Some(guide) = guide {
        paint_guide(scene, corners, guide, light, dark, if turning { 0.35 } else { 0.45 });
    }
    if upright {
        scene.push_quad(Quad::new(shown.inflate(1.0), Color::TRANSPARENT).border(1.0, dark.with_alpha(0.55)));
        scene.push_quad(Quad::new(shown, Color::TRANSPARENT).border(1.0, light));
    } else {
        let edges = |width: f32| {
            let mut outline = Vec::with_capacity(32);
            for i in 0..4 {
                outline.extend(stroke_outline(&[corners[i], corners[(i + 1) % 4]], width));
            }
            outline
        };
        scene.push_path(PathFill::solid(edges(3.0), dark.with_alpha(0.55)));
        scene.push_path(PathFill::solid(edges(1.0), light));
    }
    if rules.knob {
        let top = map.screen(t.handle(BoxHandle::Top));
        let knob = knob_at(t, map);
        scene.push_path(PathFill::solid(stroke_outline(&[top, knob], 3.0), dark.with_alpha(0.55)));
        scene.push_path(PathFill::solid(stroke_outline(&[top, knob], 1.0), light));
        let dot = Rect::new(knob.x - 5.0, knob.y - 5.0, 10.0, 10.0);
        scene.push_quad(Quad::new(dot, light).radius(5.0).border(1.0, dark.with_alpha(0.6)));
    }
    for handle in BoxHandle::ALL {
        let at = map.screen(t.handle(handle));
        if upright {
            let half = HANDLE_SIDE / 2.0;
            let mark = Rect::new(at.x - half, at.y - half, HANDLE_SIDE, HANDLE_SIDE);
            scene.push_quad(Quad::new(mark, light).radius(2.0).border(1.0, dark.with_alpha(0.6)));
        } else {
            scene.push_path(PathFill::solid(square(at, HANDLE_SIDE + 2.0, t.angle), dark.with_alpha(0.6)));
            scene.push_path(PathFill::solid(square(at, HANDLE_SIDE, t.angle), light));
        }
    }
    if rules.pivot {
        let at = map.screen(t.pivot_point());
        let ring = Rect::new(at.x - 5.0, at.y - 5.0, 10.0, 10.0);
        scene.push_quad(
            Quad::new(ring.inflate(1.0), Color::TRANSPARENT).radius(6.0).border(1.0, dark.with_alpha(0.6)),
        );
        scene.push_quad(Quad::new(ring, Color::TRANSPARENT).radius(5.0).border(1.5, light));
        scene.push_quad(Quad::new(Rect::new(at.x - 1.0, at.y - 1.0, 2.0, 2.0), light).radius(1.0));
    }
    scene.pop_clip();
}

impl View for TransformHandles {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let labels = cx.labels();
        let live = cx.use_state(|| false);
        let is_live = live.get();
        // During a drag it paints from the binding on its own: reading it here too would make
        // every step of the drag a rebuild.
        let current = if is_live {
            Observer::untracked(|| self.value.get())
        } else {
            self.frame.space.watch();
            self.value.get()
        };
        let hover = cx.use_state(|| Part::None);
        let memory = cx.use_state(|| Rc::new(TransformMemory::default())).get();
        // Only a crop answers for what others set, since a box is otherwise free to go anywhere.
        let current = match self.frame.known_size(cx) {
            Some(size) if self.rules.shrink_to_turn && !is_live && size.width > 0.0 => {
                let map = self.frame.space.map(Rect::new(0.0, 0.0, size.width, size.height));
                let settled = self.rules.settled(current, map.bounds(), map.scale);
                if settled != current {
                    self.value.set(settled);
                }
                settled
            }
            _ => current,
        };
        let (light, dark) = picture_inks(&theme);
        let rules = self.rules;
        let style = BoxStyle { rules, look: self.look, light, dark, scrim: theme.color(Semantic::Scrim) };
        let (space, value, remembered) = (self.frame.space.clone(), self.value.clone(), memory.clone());
        let paint = gesture_paint(is_live, move |frame, scene| {
            remembered.frame.set(frame);
            let turning = remembered.grab.get().is_some_and(|g| g.part == Part::Rotate);
            paint_box(scene, frame, &space.map(frame), &value.get(), style, turning);
        });
        let (root, surface) = self.frame.build(cx, paint);
        let name = self
            .label
            .clone()
            .unwrap_or_else(|| if self.crop_name { labels.crop.clone() } else { labels.transform.clone() });
        let decimal = labels.decimal_separator;
        let text = format!(
            "{:.0}, {:.0}, {:.0} × {:.0}, {}",
            current.rect.x(),
            current.rect.y(),
            current.rect.width(),
            current.rect.height(),
            angle_text(current.angle, decimal),
        );
        cx.semantics(surface, Semantics::new(Role::Group).name(name).value(text).disabled(self.disabled));
        if self.disabled {
            return root;
        }
        cx.focusable(surface, 0.0);
        cx.cursor(surface, cursor_of(hover.get(), current.angle));
        let modifiers = cx.modifiers();
        let (value, space, grabbed, commit) =
            (self.value.clone(), self.frame.space.clone(), memory.clone(), self.on_commit.clone());
        let held = modifiers.clone();
        let dragged_live = live.clone();
        cx.on_drag(
            surface,
            Rc::new(move |d| {
                let map = space.map(d.frame);
                let at = map.content(d.position);
                match d.phase {
                    DragPhase::Start => {
                        let start = value.get();
                        let part = part_at(&start, &map, d.position, rules);
                        if part != Part::None {
                            grabbed.grab.set(Some(Grab { part, pressed: at, start }));
                            dragged_live.set_if_changed(true);
                        }
                    }
                    DragPhase::Move => {
                        let Some(grab) = grabbed.grab.get() else { return };
                        let next = rules.dragged(grab, at, &map, held.get());
                        if next != value.get() {
                            value.set(next);
                        }
                    }
                    DragPhase::End => {
                        dragged_live.set_if_changed(false);
                        let Some(grab) = grabbed.grab.take() else { return };
                        if let Some(commit) = &commit
                            && value.get() != grab.start
                        {
                            commit(value.get());
                        }
                    }
                    // The base may add more; they act as nothing here.
                    _ => {}
                }
            }),
        );
        let (value, space, moved, hovered) =
            (self.value.clone(), self.frame.space.clone(), memory.clone(), hover.clone());
        cx.on_pointer_move(
            surface,
            Rc::new(move |at, frame| {
                moved.pointer.set(Some(at));
                if moved.grab.get().is_none() {
                    hovered.set_if_changed(part_at(&value.get(), &space.map(frame), at, rules));
                }
            }),
        );
        let (value, space, clicked) = (self.value.clone(), self.frame.space.clone(), memory.clone());
        cx.on_double_click(
            surface,
            Rc::new(move || {
                let Some(at) = clicked.pointer.get() else { return };
                let map = space.map(clicked.frame.get());
                let t = value.get();
                if part_at(&t, &map, at, rules) != Part::Pivot {
                    return;
                }
                let middle = t.rect.center();
                let grab = Grab { part: Part::Pivot, pressed: middle, start: t };
                value.set(rules.dragged(grab, t.to_world(middle), &map, Modifiers::default()));
            }),
        );
        let (value, space, commit) = (self.value.clone(), self.frame.space.clone(), self.on_commit.clone());
        cx.on_key(
            surface,
            Rc::new(move |key, modifiers| {
                if *key == Key::Escape
                    && let Some(grab) = memory.grab.take()
                {
                    value.set(grab.start);
                    live.set_if_changed(false);
                    return true;
                }
                let map = space.map(memory.frame.get());
                let Some(next) = rules.on_key(value.get(), key, modifiers, &map) else { return false };
                value.set(next);
                if let Some(commit) = &commit {
                    commit(next);
                }
                true
            }),
        );
        root
    }
}

// ---------------------------------------------------------------------------------------------
// Protractor
// ---------------------------------------------------------------------------------------------

/// An angle as people read it: `+3.5°`, `-0.1°`, `0.0°`, never `-0.0°`.
fn angle_text(angle: f32, decimal: char) -> String {
    let shown = (angle.abs() * 10.0).round() / 10.0;
    let sign = if shown == 0.0 {
        ""
    } else if angle > 0.0 {
        "+"
    } else {
        "-"
    };
    format!("{sign}{shown:.1}°").replace('.', &decimal.to_string())
}

/// How far the middle of the arc rises above its ends, whatever the width: the band has one height.
const PROTRACTOR_RISE: f32 = 31.0;
const PROTRACTOR_MARGIN: f32 = 14.0;
const PROTRACTOR_TOP: f32 = 9.0;

/// Where an angle falls on the arc drawn in `frame`.
#[derive(Clone, Copy, Debug)]
struct ProtractorArc {
    frame: Rect,
    least: f32,
    most: f32,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl ProtractorArc {
    /// The radius of the circle through the ends and the top of the arc.
    fn radius(&self) -> f32 {
        let half = (self.frame.width() / 2.0 - PROTRACTOR_MARGIN).max(1.0);
        (half * half + PROTRACTOR_RISE * PROTRACTOR_RISE) / (2.0 * PROTRACTOR_RISE)
    }

    fn point(&self, angle: f32, inset: f32) -> Point {
        let middle = (self.least + self.most) / 2.0;
        let half = ((self.most - self.least) / 2.0).max(f32::EPSILON);
        let r = self.radius();
        let sweep = ((self.frame.width() / 2.0 - PROTRACTOR_MARGIN).max(1.0) / r).clamp(-1.0, 1.0).asin();
        let (sin, cos) = ((angle - middle) / half * sweep).sin_cos();
        let top = self.frame.y() + PROTRACTOR_TOP;
        Point::new(self.frame.center().x + (r - inset) * sin, top + r - (r - inset) * cos)
    }

    fn pixels_per_degree(&self) -> f32 {
        (self.frame.width() - PROTRACTOR_MARGIN * 2.0).max(1.0) / (self.most - self.least).max(f32::EPSILON)
    }
}

/// A band of ticks along an arc that sets an angle, by a drag from anywhere on it. A double
/// click goes back to zero. The arrows change it by a step (with Shift, ten), Home and End go to
/// the ends, and Delete goes back to zero.
///
/// ```
/// use ravenrs_reactive::State;
/// use ravenrs_widgets::Protractor;
///
/// let angle = State::new(0.0_f32);
/// let straighten = Protractor::new(angle.binding()).range(-45.0, 45.0).label("Straighten");
/// ```
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct Protractor {
    value: Binding<f32>,
    least: f32,
    most: f32,
    step: f32,
    width: f32,
    on_commit: Option<Rc<dyn Fn(f32)>>,
    disabled: bool,
    label: Option<String>,
}

impl fmt::Debug for Protractor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Protractor")
            .field("least", &self.least)
            .field("most", &self.most)
            .finish_non_exhaustive()
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl Protractor {
    /// A band bound to `value`, in degrees, from -45 to 45 in steps of 0.1.
    pub fn new(value: Binding<f32>) -> Self {
        Self {
            value,
            least: -45.0,
            most: 45.0,
            step: 0.1,
            width: 260.0,
            on_commit: None,
            disabled: false,
            label: None,
        }
    }

    /// The least and the most it reaches, in degrees.
    pub fn range(mut self, least: f32, most: f32) -> Self {
        self.least = least;
        self.most = most.max(least + f32::EPSILON);
        self
    }

    /// The smallest change, in degrees (0.1 by default).
    pub fn step(mut self, step: f32) -> Self {
        self.step = step.max(0.001);
        self
    }

    /// Its width in logical pixels (260 by default).
    pub fn width(mut self, width: f32) -> Self {
        self.width = width.max(80.0);
        self
    }

    /// Runs `handler` with the angle when a drag ends or a key or double click changes it: one
    /// step to undo.
    pub fn on_commit(mut self, handler: impl Fn(f32) + 'static) -> Self {
        self.on_commit = Some(Rc::new(handler));
        self
    }

    /// Shows the angle without letting it change.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// The name screen readers announce, instead of the one in [`Labels`](crate::Labels).
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }
}

/// On the step and in the range, and never a negative zero.
fn on_step(angle: f32, least: f32, most: f32, step: f32) -> f32 {
    let stepped = ((angle / step).round() * step * 1000.0).round() / 1000.0;
    let held = stepped.clamp(least, most);
    if held == 0.0 { 0.0 } else { held }
}

impl View for Protractor {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let labels = cx.labels();
        let (least, most, step) = (self.least, self.most, self.step);
        let current = self.value.get();
        let grab = cx.use_state(|| Rc::new(Cell::new(None::<(f32, f32)>))).get();
        let disabled = self.disabled;
        let (band, border, major, minor, accent, ink) = (
            theme.color(Semantic::Control),
            theme.color(Semantic::Border),
            theme.color(if disabled { Semantic::InkMuted } else { Semantic::Ink }),
            theme.color(Semantic::InkMuted),
            theme.color(if disabled { Semantic::InkMuted } else { Semantic::Accent }),
            theme.color(if disabled { Semantic::InkMuted } else { Semantic::Ink }),
        );
        let radius = theme.radii.md;
        let span = most - least;
        // Few enough ticks to tell apart, on multiples people recognize.
        let tick = [0.5_f32, 1.0, 2.5, 5.0, 10.0, 15.0, 30.0]
            .into_iter()
            .find(|t| span / *t <= 40.0)
            .unwrap_or(45.0);
        let paint = Paint::Custom(Rc::new(move |frame: Rect, scene: &mut Scene| {
            scene.push_quad(Quad::new(frame, band).radius(radius).border(1.0, border));
            let arc = ProtractorArc { frame, least, most };
            let (mut small, mut large) = (Vec::new(), Vec::new());
            let mut at = (least / tick).ceil() * tick;
            while at <= most + 0.001 {
                let big = (at / (tick * 6.0)).fract().abs() < 0.001 || at == 0.0;
                let line = [arc.point(at, 0.0), arc.point(at, if big { 8.0 } else { 4.0 })];
                if big {
                    large.extend(stroke_outline(&line, 1.5));
                } else {
                    small.extend(stroke_outline(&line, 1.0));
                }
                at += tick;
            }
            scene.push_path(PathFill::solid(small, minor.with_alpha(0.7)));
            scene.push_path(PathFill::solid(large, major));
            let line = [arc.point(current, -2.0), arc.point(current, 13.0)];
            scene.push_path(PathFill::solid(stroke_outline(&line, 2.5), accent));
        }));
        let text = angle_text(current, labels.decimal_separator);
        let number = util::label(cx, &text, TextStyle::Caption, None, ink);
        let node = cx.container(
            LayoutStyle {
                width: Some(self.width),
                height: Some(52.0),
                padding: Insets { bottom: 5.0, ..Insets::ZERO },
                align: Align::Center,
                justify: Justify::End,
                align_self: Some(Align::Start),
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &[number],
            paint,
        );
        let name = self.label.clone().unwrap_or_else(|| labels.protractor.clone());
        cx.semantics(
            node,
            Semantics::new(Role::Slider)
                .name(name)
                .value(text)
                .numeric(f64::from(current), f64::from(least), f64::from(most), Some(f64::from(step)))
                .disabled(disabled),
        );
        if disabled {
            return node;
        }
        cx.focusable(node, radius);
        cx.cursor(node, CursorIcon::ColResize);
        let (value, commit) = (self.value.clone(), self.on_commit.clone());
        let change = Rc::new(move |angle: f32, done: bool| {
            let angle = on_step(angle, least, most, step);
            if angle != value.get() {
                value.set(angle);
            }
            if done && let Some(commit) = &commit {
                commit(value.get());
            }
        });
        let (dragged, value) = (change.clone(), self.value.clone());
        cx.on_drag(
            node,
            Rc::new(move |d| {
                let arc = ProtractorArc { frame: d.frame, least, most };
                match d.phase {
                    DragPhase::Start => grab.set(Some((d.position.x, value.get()))),
                    DragPhase::Move => {
                        if let Some((from, angle)) = grab.get() {
                            dragged(angle + (d.position.x - from) / arc.pixels_per_degree(), false);
                        }
                    }
                    DragPhase::End => {
                        if let Some((from, angle)) = grab.take()
                            && (d.position.x - from).abs() > 0.0
                        {
                            dragged(angle + (d.position.x - from) / arc.pixels_per_degree(), true);
                        }
                    }
                    // The base may add more; they act as nothing here.
                    _ => {}
                }
            }),
        );
        let reset = change.clone();
        cx.on_double_click(node, Rc::new(move || reset(0.0, true)));
        let value = self.value.clone();
        cx.on_key(
            node,
            Rc::new(move |key, modifiers| {
                let by = if modifiers.shift { step * 10.0 } else { step };
                let angle = value.get();
                let next = match key {
                    Key::ArrowLeft | Key::ArrowDown => angle - by,
                    Key::ArrowRight | Key::ArrowUp => angle + by,
                    Key::Home => least,
                    Key::End => most,
                    Key::Delete | Key::Backspace => 0.0,
                    _ => return false,
                };
                change(next, true);
                true
            }),
        );
        node
    }
}

// ---------------------------------------------------------------------------------------------
// CropOverlay
// ---------------------------------------------------------------------------------------------

/// What a [`CropOverlay`] edits: a rectangle of the content and how far it is turned.
///
/// The picture stays still and the frame turns over it, since the renderer cannot turn a
/// picture. A frame turned clockwise by `angle` holds what an upright frame would hold with the
/// picture turned counterclockwise by it, around the middle of the rectangle.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct CropArea {
    /// The rectangle before turning, in the units of the content.
    pub rect: Rect,
    /// How far it is turned clockwise, in degrees.
    pub angle: f32,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl CropArea {
    /// An upright rectangle.
    pub const fn new(rect: Rect) -> Self {
        Self { rect, angle: 0.0 }
    }

    /// The same rectangle turned clockwise by this many degrees.
    pub const fn rotated(mut self, angle: f32) -> Self {
        self.angle = angle;
        self
    }

    /// Its four corners once turned, clockwise from what was the top left, in the units of the
    /// content.
    pub fn corners(&self) -> [Point; 4] {
        BoxTransform::new(self.rect).rotated(self.angle).corners()
    }
}

/// A crop over a picture: what is out of the frame goes dark, with thirds inside the frame and
/// a fine grid while it turns. It is built on [`TransformHandles`] and draws the same lines as
/// [`RuleOfThirdsGrid`](crate::RuleOfThirdsGrid).
///
/// Handles resize it, a drag inside moves it, a drag outside turns it, and it never leaves the
/// content: turning makes it smaller if it must. It hands back the rectangle and the angle and
/// does not turn the picture, since the renderer has no transforms.
///
/// Keyboard: the arrows move it, with Alt they resize it, `[` and `]` turn it, and Shift makes
/// each of these go further. A [`Protractor`] can share the angle through
/// [`Binding::map`](ravenrs_reactive::Binding::map).
///
/// ```
/// use ravenrs_core::{Rect, Size};
/// use ravenrs_reactive::State;
/// use ravenrs_widgets::{ContentSpace, CropArea, CropOverlay, Protractor};
///
/// let crop = State::new(CropArea::new(Rect::new(300.0, 200.0, 1800.0, 1200.0)));
/// let overlay = CropOverlay::new(crop.binding())
///     .aspect(Some(3.0 / 2.0))
///     .space(ContentSpace::Fit(Size::new(2400.0, 1600.0)))
///     .size(600.0, 400.0);
/// let straighten = Protractor::new(crop.binding().map(|c| c.angle, |c, angle| c.angle = angle));
/// ```
#[derive(Debug)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct CropOverlay {
    handles: TransformHandles,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl CropOverlay {
    /// A crop bound to `area`, in the units of its [space](Self::space).
    pub fn new(area: Binding<CropArea>) -> Self {
        let value = area.map(
            |a| BoxTransform::new(a.rect).rotated(a.angle),
            |a, t: BoxTransform| {
                a.rect = t.rect;
                a.angle = t.angle;
            },
        );
        let mut handles = TransformHandles::new(value);
        handles.rules = Rules {
            keep_corners: false,
            aspect: None,
            confined: true,
            min_size: 24.0,
            knob: false,
            pivot: false,
            outside_rotates: true,
            angle_limit: Some(45.0),
            shrink_to_turn: true,
        };
        handles.look = Look {
            veil: true,
            guide: Some(CompositionGuide::Thirds),
            turning_guide: Some(CompositionGuide::Divisions(8)),
        };
        handles.crop_name = true;
        Self { handles }
    }

    /// Holds this width over height, or lets it go free with `None`. A crop of other proportions
    /// is cut down to these as soon as this changes.
    pub fn aspect(mut self, aspect: Option<f32>) -> Self {
        self.handles.rules.aspect = aspect;
        self
    }

    /// How far it can turn either way, in degrees (45 by default). With 0 it does not turn.
    pub fn angle_limit(mut self, degrees: f32) -> Self {
        let degrees = degrees.abs();
        self.handles.rules.angle_limit = Some(degrees);
        self.handles.rules.outside_rotates = degrees > 0.0;
        self
    }

    /// The lines inside the frame, or none (thirds by default).
    pub fn guide(mut self, guide: Option<CompositionGuide>) -> Self {
        self.handles.look.guide = guide;
        self
    }

    /// The lines inside the frame while it turns (eight parts by default).
    pub fn turning_guide(mut self, guide: Option<CompositionGuide>) -> Self {
        self.handles.look.turning_guide = guide;
        self
    }

    /// The smallest side, in logical pixels on screen at the current zoom (24 by default).
    pub fn min_size(mut self, pixels: f32) -> Self {
        self.handles.rules.min_size = pixels.max(1.0);
        self
    }

    /// Runs `handler` with the result when a drag ends or a key changes it: one step to undo.
    pub fn on_commit(mut self, handler: impl Fn(CropArea) + 'static) -> Self {
        self.handles.on_commit = Some(Rc::new(move |t: BoxTransform| {
            handler(CropArea { rect: t.rect, angle: t.angle });
        }));
        self
    }

    /// Takes no input while disabled.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.handles.disabled = disabled;
        self
    }

    /// The name screen readers announce, instead of the one in [`Labels`](crate::Labels).
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.handles.label = Some(label.into());
        self
    }
}

overlay_builders!(CropOverlay, handles.frame);

impl View for CropOverlay {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        self.handles.build(cx)
    }
}
