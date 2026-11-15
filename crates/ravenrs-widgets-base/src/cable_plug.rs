//! [`CablePlug`]: a cable that hangs, swings and settles between any two places of
//! the window: two widgets, a widget and the pointer, a widget and a point.
//!
//! ```ignore
//! let button = cx.child(&Button::new("Exportar"));
//! let tip = cx.child(&Text::new("Exporte em PDF aqui"));
//! CablePlug::between(button, tip).build(cx);
//! ```
//!
//! The runtime places the ends on the widgets' frames after every layout, runs the
//! physics in the window's [`CableWorld`] (on the GPU when the platform gives one,
//! otherwise on the CPU), paints only while some cable still moves and shows every
//! cable at rest under reduced motion. Components that draw on their own canvas put
//! [`Cable`]s in their scene instead and read the shapes back with [`Cables`].

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use ravenrs_core::cable::{Cable, CableEnd, CableLook, CablePhysics, CableRelease, CableTexture, CableWorld};
use ravenrs_core::{Color, Point, Rect};

use crate::ids::NodeId;

/// Where on a widget an end of a cable goes, and the way the cable leaves it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "variants"))]
pub enum CableSide {
    /// The middle of the left edge, leaving to the left.
    Left,
    /// The middle of the right edge, leaving to the right.
    Right,
    /// The middle of the top edge, leaving upward.
    Top,
    /// The middle of the bottom edge, leaving downward.
    Bottom,
    /// The center, leaving any way: a jack seen from the front.
    #[default]
    Center,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl CableSide {
    pub(crate) fn end(self, frame: Rect) -> CableEnd {
        let c = frame.center();
        match self {
            Self::Left => CableEnd::at(Point::new(frame.x(), c.y)).toward(-1.0, 0.0),
            Self::Right => CableEnd::at(Point::new(frame.max_x(), c.y)).toward(1.0, 0.0),
            Self::Top => CableEnd::at(Point::new(c.x, frame.y())).toward(0.0, -1.0),
            Self::Bottom => CableEnd::at(Point::new(c.x, frame.max_y())).toward(0.0, 1.0),
            Self::Center => CableEnd::at(c),
        }
    }
}

/// What an end of a [`CablePlug`] is held by.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "variants"))]
pub enum CableAnchor {
    /// A widget built in this build, on one of its sides.
    Node(NodeId, CableSide),
    /// A point of the window.
    Point(CableEnd),
    /// The pointer, wherever it goes: a cable being carried.
    Pointer,
}

/// A cable between two places of the window. Build it with `CablePlug::build` in the
/// build of the view that owns it, after building what it ties.
#[derive(Clone)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct CablePlug {
    pub(crate) key: Option<u64>,
    pub(crate) from: CableAnchor,
    pub(crate) to: CableAnchor,
    pub(crate) via: Vec<Point>,
    pub(crate) physics: CablePhysics,
    pub(crate) look: CableLook,
    pub(crate) release: CableRelease,
    pub(crate) grow: bool,
    pub(crate) on_click: Option<Rc<dyn Fn()>>,
    pub(crate) on_drop: Option<Rc<dyn Fn(Point)>>,
}

impl std::fmt::Debug for CablePlug {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CablePlug").field("from", &self.from).field("to", &self.to).finish_non_exhaustive()
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl CablePlug {
    fn new(from: CableAnchor, to: CableAnchor) -> Self {
        Self {
            key: None,
            from,
            to,
            via: Vec::new(),
            physics: CablePhysics::default(),
            look: CableLook::default(),
            release: CableRelease::Fall,
            grow: true,
            on_click: None,
            on_drop: None,
        }
    }

    /// A cable from the center of `a` to the center of `b`.
    pub fn between(a: NodeId, b: NodeId) -> Self {
        Self::new(CableAnchor::Node(a, CableSide::Center), CableAnchor::Node(b, CableSide::Center))
    }

    /// A cable out of `a` whose other end is in the hand: it follows the pointer
    /// until it is tied with [`Self::to`] or the pointer lets go ([`Self::on_drop`]).
    pub fn from(a: NodeId) -> Self {
        Self::new(CableAnchor::Node(a, CableSide::Center), CableAnchor::Pointer)
    }

    /// A cable between any two anchors.
    pub fn anchored(from: CableAnchor, to: CableAnchor) -> Self {
        Self::new(from, to)
    }

    /// Ties the far end to `b`.
    pub fn to(mut self, b: NodeId) -> Self {
        self.to = CableAnchor::Node(b, CableSide::Center);
        self
    }

    /// Ties the far end to a point of the window.
    pub fn to_point(mut self, p: Point) -> Self {
        self.to = CableAnchor::Point(CableEnd::at(p));
        self
    }

    /// Lets the far end follow the pointer.
    pub fn loose(mut self) -> Self {
        self.to = CableAnchor::Pointer;
        self
    }

    /// The sides of the two widgets the ends go on.
    pub fn sides(mut self, from: CableSide, to: CableSide) -> Self {
        if let CableAnchor::Node(n, _) = self.from {
            self.from = CableAnchor::Node(n, from);
        }
        if let CableAnchor::Node(n, _) = self.to {
            self.to = CableAnchor::Node(n, to);
        }
        self
    }

    /// Points the cable is held at on the way.
    pub fn via(mut self, points: impl IntoIterator<Item = Point>) -> Self {
        self.via = points.into_iter().collect();
        self
    }

    /// Keeps the same cable across builds even when what it ties changes, so moving
    /// its end makes it swing instead of a new cable showing up.
    pub fn key(mut self, key: u64) -> Self {
        self.key = Some(key);
        self
    }

    /// Body color.
    pub fn color(mut self, color: Color) -> Self {
        self.look.color = color;
        self
    }

    /// Width in logical pixels.
    pub fn thickness(mut self, width: f32) -> Self {
        self.look.thickness = width.max(0.5);
        self
    }

    /// Strength of the highlight along it, `0` to `1`.
    pub fn shine(mut self, shine: f32) -> Self {
        self.look.shine = shine.clamp(0.0, 1.0);
        self
    }

    /// Its surface: smooth, braided, coiled, striped or a picture of yours.
    pub fn texture(mut self, texture: CableTexture) -> Self {
        self.look.texture = texture;
        self
    }

    /// Draws a plug at each attached end.
    pub fn plugs(mut self, plugs: bool) -> Self {
        self.look.plugs = plugs;
        self
    }

    /// Color of the plugs.
    pub fn plug_color(mut self, color: Color) -> Self {
        self.look.plug_color = color;
        self
    }

    /// A dashed cable: dash and gap lengths.
    pub fn dash(mut self, dash: f32, gap: f32) -> Self {
        self.look.dash = Some([dash, gap]);
        self
    }

    /// How much longer than the path it is: the sag. `1.0` is taut.
    pub fn slack(mut self, slack: f32) -> Self {
        self.physics.slack = slack.max(1.0);
        self
    }

    /// The whole physics at once: weight, damping, stiffness, points.
    pub fn physics(mut self, physics: CablePhysics) -> Self {
        self.physics = physics;
        self
    }

    /// Downward pull, in logical pixels per second squared.
    pub fn gravity(mut self, gravity: f32) -> Self {
        self.physics.gravity = gravity;
        self
    }

    /// What happens when it is no longer built: it falls (the default), it is pulled
    /// back to its start, or it vanishes.
    pub fn release(mut self, release: CableRelease) -> Self {
        self.release = release;
        self
    }

    /// Pulled back to its start when no longer built, like a switchboard cord.
    pub fn retract(self, retract: bool) -> Self {
        let release = if retract { CableRelease::Retract } else { CableRelease::Fall };
        self.release(release)
    }

    /// A new cable starts taut and drops into its sag (the default); without it the
    /// cable shows up at rest.
    pub fn grow(mut self, grow: bool) -> Self {
        self.grow = grow;
        self
    }

    /// Called on a click on the cable.
    pub fn on_click(mut self, handler: impl Fn() + 'static) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }

    /// Called with where the pointer is when it lets go of a cable whose end it holds.
    pub fn on_drop(mut self, handler: impl Fn(Point) + 'static) -> Self {
        self.on_drop = Some(Rc::new(handler));
        self
    }
}

/// The cables of the window, to read their shapes back: where a cable is now, which
/// one is under the pointer. On the GPU the shapes arrive a frame late.
#[derive(Clone)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct Cables(pub(crate) Rc<RefCell<Box<dyn CableWorld>>>);

impl std::fmt::Debug for Cables {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Cables")
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl Cables {
    /// The latest points of the cable `id`.
    pub fn shape(&self, id: u64) -> Option<Arc<[Point]>> {
        self.0.borrow().shape(id)
    }

    /// The ids of the cables held now.
    pub fn ids(&self) -> Vec<u64> {
        self.0.borrow().ids()
    }

    /// Whether any cable still moves.
    pub fn moving(&self) -> bool {
        self.0.borrow().moving()
    }

    /// Whether the physics runs on the GPU.
    pub fn on_gpu(&self) -> bool {
        self.0.borrow().on_gpu()
    }
}

/// How far a point is from a line through `points`.
pub fn distance_to_cable(points: &[Point], p: Point) -> f32 {
    let mut best = f32::INFINITY;
    for w in points.windows(2) {
        let (a, b) = (w[0], w[1]);
        let (dx, dy) = (b.x - a.x, b.y - a.y);
        let length = dx * dx + dy * dy;
        let t =
            if length > 0.0 { (((p.x - a.x) * dx + (p.y - a.y) * dy) / length).clamp(0.0, 1.0) } else { 0.0 };
        let (x, y) = (a.x + dx * t - p.x, a.y + dy * t - p.y);
        best = best.min(x.hypot(y));
    }
    best
}

/// A cable built this build, waiting for the layout to place its ends.
#[derive(Clone)]
pub(crate) struct CableRequest {
    pub plug: CablePlug,
    /// The widget that built it, for an identity that lasts across builds.
    pub owner: u64,
    pub index: u64,
}

/// The id of a cable built by `owner`, the `index`th of its build, unless it has a key.
pub(crate) fn cable_id(request: &CableRequest, from: Option<u64>, to: Option<u64>) -> u64 {
    let mut h = request.owner ^ 0x6361_626c_6500_0000;
    let mix = |h: u64, v: u64| {
        let mut x = h ^ v.wrapping_mul(0x9e37_79b9_7f4a_7c15);
        x = (x ^ (x >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        x ^ (x >> 31)
    };
    match request.plug.key {
        Some(key) => mix(h, key),
        None => {
            h = mix(h, from.unwrap_or(request.index));
            mix(h, to.unwrap_or(0x0070_6f69_6e74_6572))
        }
    }
}

/// The group every cable built with [`CablePlug`] belongs to: the window's.
pub(crate) const PLUG_GROUP: u64 = 0;

/// The cable as the world takes it, once its ends are placed.
pub(crate) fn place(request: &CableRequest, id: u64, from: CableEnd, to: CableEnd) -> Cable {
    let plug = &request.plug;
    Cable {
        id,
        group: PLUG_GROUP,
        from,
        to,
        via: Arc::from(plug.via.as_slice()),
        physics: plug.physics,
        look: plug.look.clone(),
        release: plug.release,
        grow: plug.grow,
        points: None,
        view: ravenrs_core::cable::CableView::default(),
    }
}
