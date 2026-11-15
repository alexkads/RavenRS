//! Cables that hang, swing and settle: the model behind `CablePlug`.
//!
//! A cable is a rope of a few points joined by distance constraints, moved by
//! Verlet integration under gravity, with its ends (and any points it must pass
//! through) pinned. It knows nothing of nodes, ports or widgets: each end is a
//! point and, optionally, the direction the cable leaves it in, so a flow
//! diagram, a rack of jacks, a switchboard or a tooltip tied to a button all
//! describe their cables the same way.
//!
//! * [`Cable`] describes one cable for a frame: ends, physics, look. Widgets put
//!   it in the scene as [`Primitive::Cable`](crate::Primitive::Cable).
//! * [`CableWorld`] simulates every cable of a window. [`CpuCables`] is the
//!   reference, run on the CPU; the renderer has the same algorithm on the GPU.
//! * [`cable_rest`] is where a cable settles, the same for the same input: what
//!   exports draw and what reduced motion shows.
//! * [`cable_primitives`] draws a cable as plain shapes, for every output that is
//!   not the GPU.
//!
//! The step is fixed ([`CABLE_STEP`]), counted on the world's own clock, so the
//! same events give the same shape at any frame rate.

use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use crate::{Color, ImageData, PathElement, PathFill, PathPaint, Point, Primitive, Quad, Rect};

/// Seconds per simulation step.
pub const CABLE_STEP: f32 = 1.0 / 120.0;
/// Constraint passes per step.
pub const CABLE_ITERATIONS: u32 = 12;
/// The most points a cable has; the GPU runs one thread per point.
pub const CABLE_MAX_POINTS: usize = 32;
/// Movement, in logical pixels per step, under which a cable counts as still.
pub const CABLE_STILL: f32 = 0.03;
/// The most steps one frame runs, so a long pause does not freeze the window.
const MAX_STEPS_PER_FRAME: u64 = 30;
/// Steps a dropped cable takes to fade out.
const FADE_STEPS: u32 = 48;
/// Pieces each segment is drawn in, along a curve through the points: the GPU draws
/// the same.
pub const CABLE_SMOOTHING: usize = 4;
/// Steps the settling of a cable at rest is computed with.
const REST_STEPS: u32 = 360;

/// One end of a cable: where it is and, optionally, the direction the cable
/// leaves it in (out of a port, out of a jack, down from a hole).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CableEnd {
    /// Where the end is, in logical pixels.
    pub at: Point,
    /// Unit vector the cable leaves the end in, if it has one.
    pub toward: Option<Point>,
}

impl CableEnd {
    /// An end at `p` that the cable may leave in any direction.
    pub const fn at(p: Point) -> Self {
        Self { at: p, toward: None }
    }

    /// The same end, leaving in the direction `(dx, dy)` (any length).
    pub fn toward(mut self, dx: f32, dy: f32) -> Self {
        let length = dx.hypot(dy);
        self.toward = (length > f32::EPSILON).then(|| Point::new(dx / length, dy / length));
        self
    }
}

/// What happens to a cable when it is no longer described.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum CableRelease {
    /// It disappears at once.
    #[default]
    Vanish,
    /// Its far end comes loose; it falls, hanging from its start, and fades.
    Fall,
    /// Its far end is pulled back to its start, like a switchboard cord.
    Retract,
}

/// How a cable moves.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CablePhysics {
    /// Length at rest over the length of the path between its ends: the sag.
    pub slack: f32,
    /// Downward pull in logical pixels per second squared.
    pub gravity: f32,
    /// Share of its speed a point keeps from one step to the next, under 1.
    pub keep: f32,
    /// How much of a stretch each pass undoes, `0` to `1`.
    pub stiffness: f32,
    /// Points along the cable, 4 to [`CABLE_MAX_POINTS`].
    pub points: u8,
    /// How far the cable runs straight out of an end that has a direction.
    pub lead: f32,
}

impl Default for CablePhysics {
    fn default() -> Self {
        Self { slack: 1.12, gravity: 2400.0, keep: 0.975, stiffness: 1.0, points: 16, lead: 18.0 }
    }
}

impl CablePhysics {
    fn count(&self) -> usize {
        usize::from(self.points).clamp(4, CABLE_MAX_POINTS)
    }
}

/// The surface of a cable.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum CableTexture {
    /// Plain, with a highlight along it.
    #[default]
    Smooth,
    /// Woven cloth, like an old switchboard cord.
    Braided,
    /// A coil, like a telephone cord.
    Coiled,
    /// A stripe of this color along the middle.
    Striped(Color),
    /// A picture repeated along the cable every `repeat` logical pixels, its width
    /// across the cable.
    Image {
        /// The picture.
        image: ImageData,
        /// Logical pixels along the cable one copy covers.
        repeat: f32,
    },
}

/// How a cable looks.
#[derive(Clone, Debug, PartialEq)]
pub struct CableLook {
    /// Body color.
    pub color: Color,
    /// Width in logical pixels.
    pub thickness: f32,
    /// Strength of the highlight, `0` to `1`.
    pub shine: f32,
    /// Its surface.
    pub texture: CableTexture,
    /// Draws a plug at each attached end.
    pub plugs: bool,
    /// Color of the plugs.
    pub plug_color: Color,
    /// Dash and gap lengths, for a dashed line.
    pub dash: Option<[f32; 2]>,
    /// Opacity, `0` to `1`.
    pub opacity: f32,
}

impl Default for CableLook {
    fn default() -> Self {
        Self {
            color: Color::hex(0x4C66EE),
            thickness: 4.0,
            shine: 0.6,
            texture: CableTexture::Smooth,
            plugs: false,
            plug_color: Color::hex(0x5B6478),
            dash: None,
            opacity: 1.0,
        }
    }
}

/// Where a cable's own coordinates land in the scene: scaled, then moved. A canvas
/// that pans and zooms simulates its cables in its own units, so panning does not
/// shake them, and draws them through its view.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CableView {
    /// Scene pixels per unit.
    pub scale: f32,
    /// Where the origin lands.
    pub offset: Point,
}

impl Default for CableView {
    fn default() -> Self {
        Self { scale: 1.0, offset: Point::ZERO }
    }
}

impl CableView {
    /// Where `p` lands in the scene.
    pub fn apply(&self, p: Point) -> Point {
        Point::new(p.x * self.scale + self.offset.x, p.y * self.scale + self.offset.y)
    }

    /// Where a scene point is in the cable's own units.
    pub fn back(&self, p: Point) -> Point {
        let k = self.scale.max(f32::EPSILON);
        Point::new((p.x - self.offset.x) / k, (p.y - self.offset.y) / k)
    }
}

/// One cable as a widget describes it for a frame.
#[derive(Clone, Debug, PartialEq)]
pub struct Cable {
    /// Stays the same from frame to frame for the same cable.
    pub id: u64,
    /// Who describes it: when nothing of a group is described any more, its
    /// cables go without falling. Group `0` belongs to the window and never goes.
    pub group: u64,
    /// Where it starts.
    pub from: CableEnd,
    /// Where it ends.
    pub to: CableEnd,
    /// Points it is held at on the way, in order.
    pub via: Arc<[Point]>,
    /// How it moves.
    pub physics: CablePhysics,
    /// How it looks.
    pub look: CableLook,
    /// What happens when it is no longer described.
    pub release: CableRelease,
    /// A new cable starts taut and drops into its sag; otherwise it shows up at rest.
    pub grow: bool,
    /// Where its points are, once a world has placed them, in its own units.
    pub points: Option<Arc<[Point]>>,
    /// Where its own units land in the scene.
    pub view: CableView,
}

impl Cable {
    /// A cable from `from` to `to`, with the default physics and look.
    pub fn new(id: u64, from: CableEnd, to: CableEnd) -> Self {
        Self {
            id,
            group: 0,
            from,
            to,
            via: Arc::from([]),
            physics: CablePhysics::default(),
            look: CableLook::default(),
            release: CableRelease::Vanish,
            grow: false,
            points: None,
            view: CableView::default(),
        }
    }

    /// The pinned points: index along the cable and where it is held.
    fn pins(&self, scale: f32, tail: Option<Point>) -> Vec<(usize, Point)> {
        let n = self.physics.count();
        let to = tail.unwrap_or(self.to.at);
        let gap = |a: Point, b: Point| (b.x - a.x).hypot(b.y - a.y);
        let lead = self.physics.lead.min(gap(self.from.at, to) * 0.25).max(0.0) * scale.min(1.0);
        let mut pins = vec![(0, self.from.at)];
        if let Some(d) = self.from.toward {
            pins.push((1, Point::new(self.from.at.x + d.x * lead, self.from.at.y + d.y * lead)));
        }
        if !self.via.is_empty() {
            // Held points take places along the cable in proportion to how far they are.
            let mut stops = vec![self.from.at];
            stops.extend(self.via.iter().copied());
            stops.push(to);
            let total: f32 = stops.windows(2).map(|w| gap(w[0], w[1])).sum::<f32>().max(f32::EPSILON);
            let mut along = 0.0;
            for (k, p) in self.via.iter().enumerate() {
                along += gap(stops[k], stops[k + 1]);
                let index = ((along / total) * (n - 1) as f32).round() as usize;
                let index = index.clamp(2, n.saturating_sub(3).max(2));
                if !pins.iter().any(|(i, _)| *i == index) {
                    pins.push((index, *p));
                }
            }
        }
        if tail.is_none()
            && let Some(d) = self.to.toward
        {
            pins.push((n - 2, Point::new(to.x + d.x * lead, to.y + d.y * lead)));
        }
        pins.push((n - 1, to));
        pins.sort_by_key(|(i, _)| *i);
        pins.dedup_by_key(|(i, _)| *i);
        pins
    }

    /// Length of the path through its pins.
    fn span(pins: &[(usize, Point)]) -> f32 {
        pins.windows(2).map(|w| (w[1].1.x - w[0].1.x).hypot(w[1].1.y - w[0].1.y)).sum()
    }

    /// The same cable placed at `points`.
    pub fn placed(&self, points: Arc<[Point]>) -> Self {
        Self { points: Some(points), ..self.clone() }
    }
}

/// The parameters of one step, the same on the CPU and the GPU.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CableStep {
    /// Distance between neighbouring points at rest.
    pub rest: f32,
    /// Movement from gravity in one step: `gravity * step²`.
    pub fall: f32,
    /// Share of speed kept.
    pub keep: f32,
    /// Share of a stretch undone per pass.
    pub stiffness: f32,
    /// Speed, in logical pixels per step, under which friction stops a point.
    pub friction: f32,
}

/// One fixed step of a rope of `pos`, last positions `prev`, with `pinned` held.
/// Returns how far the farthest point moved. The GPU shader is this function.
pub fn cable_step(pos: &mut [Point], prev: &mut [Point], pinned: &[Option<Point>], s: CableStep) -> f32 {
    let n = pos.len();
    let mut moved: f32 = 0.0;
    for i in 0..n {
        let p = pos[i];
        if let Some(pin) = pinned[i] {
            moved = moved.max((pin.x - p.x).hypot(pin.y - p.y));
            prev[i] = pin;
            pos[i] = pin;
            continue;
        }
        let (vx, vy) = (p.x - prev[i].x, p.y - prev[i].y);
        // Friction: a point that barely moves loses its speed quickly, so a cable comes
        // to a stop instead of creeping.
        let keep = if vx.hypot(vy) < s.friction { s.keep.min(0.85) } else { s.keep };
        let v = Point::new(vx * keep, vy * keep);
        prev[i] = p;
        pos[i] = Point::new(p.x + v.x, p.y + v.y + s.fall);
    }
    for _ in 0..CABLE_ITERATIONS {
        // Red then black: no two segments of one pass share a point, as on the GPU.
        for parity in 0..2 {
            let mut k = parity;
            while k + 1 < n {
                let (wa, wb) =
                    (f32::from(u8::from(pinned[k].is_none())), f32::from(u8::from(pinned[k + 1].is_none())));
                let total = wa + wb;
                if total > 0.0 {
                    let (a, b) = (pos[k], pos[k + 1]);
                    let (dx, dy) = (b.x - a.x, b.y - a.y);
                    let length = dx.hypot(dy);
                    // A rope only resists being stretched.
                    if length > s.rest && length > f32::EPSILON {
                        let push = (length - s.rest) / length * s.stiffness / total;
                        pos[k] = Point::new(a.x + dx * push * wa, a.y + dy * push * wa);
                        pos[k + 1] = Point::new(b.x - dx * push * wb, b.y - dy * push * wb);
                    }
                }
                k += 2;
            }
        }
    }
    for i in 0..n {
        moved = moved.max((pos[i].x - prev[i].x).hypot(pos[i].y - prev[i].y));
    }
    moved
}

fn step_of(physics: &CablePhysics, span: f32, scale: f32) -> CableStep {
    let n = physics.count();
    CableStep {
        rest: span * physics.slack.max(1.0) * scale / (n - 1) as f32,
        fall: physics.gravity * CABLE_STEP * CABLE_STEP,
        keep: physics.keep.clamp(0.0, 0.999),
        stiffness: physics.stiffness.clamp(0.0, 1.0),
        friction: 0.4,
    }
}

fn pinned_of(pins: &[(usize, Point)], n: usize) -> Vec<Option<Point>> {
    let mut pinned = vec![None; n];
    for &(i, p) in pins {
        if i < n {
            pinned[i] = Some(p);
        }
    }
    pinned
}

/// Points along the path through the pins, a starting shape.
fn along_pins(pins: &[(usize, Point)], n: usize) -> Vec<Point> {
    (0..n)
        .map(|i| {
            let after = pins.iter().position(|(k, _)| *k >= i).unwrap_or(pins.len() - 1);
            let (k1, p1) = pins[after];
            if after == 0 || k1 == i {
                return p1;
            }
            let (k0, p0) = pins[after - 1];
            let t = (i - k0) as f32 / (k1 - k0) as f32;
            Point::new(p0.x + (p1.x - p0.x) * t, p0.y + (p1.y - p0.y) * t)
        })
        .collect()
}

/// Where `cable` settles: the same for the same ends, physics and held points.
pub fn cable_rest(cable: &Cable) -> Vec<Point> {
    let n = cable.physics.count();
    let pins = cable.pins(1.0, None);
    let mut pos = along_pins(&pins, n);
    let mut prev = pos.clone();
    let pinned = pinned_of(&pins, n);
    // Settling heavily damped reaches the same rest as swinging there, in fewer steps.
    let mut step = step_of(&cable.physics, Cable::span(&pins), 1.0);
    step.keep = step.keep.min(0.9);
    for _ in 0..REST_STEPS {
        if cable_step(&mut pos, &mut prev, &pinned, step) < CABLE_STILL * 0.1 {
            break;
        }
    }
    pos
}

/// Simulates every cable of a window.
pub trait CableWorld {
    /// Takes the cables described this frame. Under `reduced` motion each one is at
    /// rest. Cables wholly outside `view` are not simulated.
    fn sync(&mut self, cables: &[Cable], view: Rect, reduced: bool);
    /// Moves the clock on by `dt` seconds and runs the steps due.
    fn step(&mut self, dt: f32);
    /// Whether any cable still moves: the window keeps painting while it does.
    fn moving(&self) -> bool;
    /// The latest known points of a cable. On the GPU they arrive a frame late.
    fn shape(&self, id: u64) -> Option<Arc<[Point]>>;
    /// The cables held now, by id.
    fn ids(&self) -> Vec<u64>;
    /// Cables let go of and still on their way out, placed, to draw on top.
    fn leaving(&self) -> Vec<Cable>;
    /// True when the renderer draws the cables from the world's own buffers; false
    /// when the scene carries their points.
    fn on_gpu(&self) -> bool {
        false
    }
}

/// What a live cable is doing.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Phase {
    Held,
    /// Let go: steps since, and for a retract where its far end is.
    Leaving {
        steps: u32,
        tail: Point,
    },
}

#[derive(Clone, Debug)]
struct Live {
    cable: Cable,
    pos: Vec<Point>,
    prev: Vec<Point>,
    phase: Phase,
    /// Rest length factor, which a retract shrinks.
    scale: f32,
    moved: f32,
    resting: bool,
}

impl Live {
    fn pins(&self) -> Vec<(usize, Point)> {
        match self.phase {
            Phase::Held => self.cable.pins(self.scale, None),
            Phase::Leaving { tail, .. } => match self.cable.release {
                CableRelease::Retract => self.cable.pins(self.scale, Some(tail)),
                // A fallen cable hangs from its start only.
                _ => self.cable.pins(1.0, None).into_iter().filter(|(i, _)| *i <= 1).collect(),
            },
        }
    }

    /// The pins and the step parameters for the steps to come.
    fn plan(&self) -> (Vec<(usize, Point)>, CableStep) {
        let pins = self.pins();
        let span = match self.phase {
            // A fallen or retracting cable keeps the length it had.
            Phase::Leaving { .. } => Cable::span(&self.cable.pins(1.0, None)),
            Phase::Held => Cable::span(&pins),
        };
        (pins, step_of(&self.cable.physics, span, self.scale))
    }

    /// Moves a let-go cable one step further on its way out.
    fn advance(&mut self) {
        if let Phase::Leaving { steps, tail } = &mut self.phase {
            *steps += 1;
            if self.cable.release == CableRelease::Retract {
                let home = self.cable.from.at;
                *tail = Point::new(tail.x + (home.x - tail.x) * 0.12, tail.y + (home.y - tail.y) * 0.12);
                self.scale = (self.scale * 0.95).max(0.02);
            }
        }
    }

    fn gone(&self) -> bool {
        match self.phase {
            Phase::Held => false,
            Phase::Leaving { steps, tail } => match self.cable.release {
                CableRelease::Retract => {
                    let home = self.cable.from.at;
                    (tail.x - home.x).hypot(tail.y - home.y) < 0.5
                }
                _ => steps >= FADE_STEPS,
            },
        }
    }

    fn opacity(&self) -> f32 {
        match self.phase {
            Phase::Leaving { steps, .. } if self.cable.release == CableRelease::Fall => {
                1.0 - steps as f32 / FADE_STEPS as f32
            }
            _ => 1.0,
        }
    }

    fn placed(&self) -> Cable {
        let mut cable = self.cable.placed(Arc::from(self.pos.as_slice()));
        cable.look.opacity *= self.opacity();
        if matches!(self.phase, Phase::Leaving { .. }) && self.cable.release == CableRelease::Fall {
            // Its far end is loose: no plug there to draw as attached.
            cable.to.toward = None;
        }
        cable
    }
}

fn rest_key(cable: &Cable) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    let mut put = |p: Point| {
        p.x.to_bits().hash(&mut h);
        p.y.to_bits().hash(&mut h);
    };
    put(cable.from.at);
    put(cable.to.at);
    put(cable.from.toward.unwrap_or(Point::ZERO));
    put(cable.to.toward.unwrap_or(Point::ZERO));
    for p in cable.via.iter() {
        put(*p);
    }
    let p = cable.physics;
    for v in [p.slack, p.gravity, p.keep, p.stiffness, p.lead] {
        v.to_bits().hash(&mut h);
    }
    p.points.hash(&mut h);
    h.finish()
}

fn outside(cable: &Cable, view: Rect) -> bool {
    let margin = 200.0;
    let mut points = vec![cable.view.apply(cable.from.at), cable.view.apply(cable.to.at)];
    points.extend(cable.via.iter().map(|p| cable.view.apply(*p)));
    let (mut x0, mut y0, mut x1, mut y1) =
        (f32::INFINITY, f32::INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY);
    for p in points {
        (x0, y0, x1, y1) = (x0.min(p.x), y0.min(p.y), x1.max(p.x), y1.max(p.y));
    }
    // It hangs below its ends.
    y1 += (x1 - x0).max(y1 - y0) * 0.6;
    x1 < view.x() - margin
        || x0 > view.max_x() + margin
        || y1 < view.y() - margin
        || y0 > view.max_y() + margin
}

/// The work one cable has for the steps of a frame: what the GPU runs for it.
#[derive(Clone, Debug, PartialEq)]
pub struct CablePlan {
    /// The cable.
    pub id: u64,
    /// Its points.
    pub points: usize,
    /// Held points: index and where.
    pub pins: Vec<(usize, Point)>,
    /// The parameters of each step.
    pub step: CableStep,
}

/// Everything about the cables of a window except running their steps: which are
/// held, which are on their way out, where they rest, how many steps are due. The
/// CPU world and the GPU world keep the same book, so they follow the same rules.
#[derive(Debug, Default)]
pub struct CableBook {
    live: HashMap<u64, Live>,
    rests: HashMap<u64, Arc<[Point]>>,
    clock: f64,
    done: u64,
    changed: bool,
    reduced: bool,
    /// Cables whose points the book set itself (new, at rest), for the GPU to take.
    placed: HashSet<u64>,
}

impl CableBook {
    /// An empty book.
    pub fn new() -> Self {
        Self::default()
    }

    fn rest(&mut self, cable: &Cable) -> Arc<[Point]> {
        let key = rest_key(cable);
        if self.rests.len() > 1024 {
            self.rests.clear();
        }
        self.rests.entry(key).or_insert_with(|| Arc::from(cable_rest(cable))).clone()
    }

    /// Steps run since the book began.
    pub fn steps(&self) -> u64 {
        self.done
    }

    /// Takes the cables described this frame; see [`CableWorld::sync`].
    pub fn sync(&mut self, cables: &[Cable], view: Rect, reduced: bool) {
        self.reduced = reduced;
        let seen: HashSet<u64> = cables.iter().map(|c| c.id).collect();
        let groups: HashSet<u64> = cables.iter().map(|c| c.group).collect();
        // Let go of what is no longer described.
        let mut gone = Vec::new();
        for (id, live) in &mut self.live {
            if seen.contains(id) {
                continue;
            }
            let keep = !reduced
                && (live.cable.group == 0 || groups.contains(&live.cable.group))
                && live.cable.release != CableRelease::Vanish;
            match (keep, live.phase) {
                (true, Phase::Held) => {
                    let tail = live.pos.last().copied().unwrap_or(live.cable.to.at);
                    live.phase = Phase::Leaving { steps: 0, tail };
                    live.resting = false;
                    self.changed = true;
                }
                (true, Phase::Leaving { .. }) => {}
                (false, _) => gone.push(*id),
            }
        }
        for id in gone {
            self.live.remove(&id);
        }
        for cable in cables {
            let n = cable.physics.count();
            let resting = reduced || outside(cable, view);
            match self.live.get_mut(&cable.id) {
                Some(live) if live.pos.len() == n => {
                    let moved = live.cable.from != cable.from
                        || live.cable.to != cable.to
                        || live.cable.via != cable.via
                        || live.cable.physics != cable.physics;
                    if live.phase != Phase::Held {
                        live.phase = Phase::Held;
                        live.scale = 1.0;
                    }
                    live.cable = Cable { points: None, ..cable.clone() };
                    if moved {
                        self.changed = true;
                    }
                    let place = resting && (moved || !live.resting);
                    live.resting = resting;
                    if place {
                        let rest = self.rest(cable);
                        if let Some(live) = self.live.get_mut(&cable.id) {
                            live.pos = rest.to_vec();
                            live.prev = rest.to_vec();
                            live.moved = 0.0;
                        }
                        self.placed.insert(cable.id);
                    }
                }
                _ => {
                    // A new cable starts taut along its path, as the line drawn while
                    // connecting was, and drops from there.
                    let drops = cable.grow && !resting;
                    let start =
                        if drops { along_pins(&cable.pins(1.0, None), n) } else { self.rest(cable).to_vec() };
                    self.changed |= drops;
                    self.placed.insert(cable.id);
                    self.live.insert(
                        cable.id,
                        Live {
                            cable: Cable { points: None, ..cable.clone() },
                            prev: start.clone(),
                            pos: start,
                            phase: Phase::Held,
                            scale: 1.0,
                            moved: if drops { 1.0 } else { 0.0 },
                            resting,
                        },
                    );
                }
            }
        }
    }

    /// Moves the clock on by `dt` seconds; returns the steps due.
    pub fn due(&mut self, dt: f32) -> u64 {
        self.clock += f64::from(dt.clamp(0.0, 0.25));
        let due = (self.clock / f64::from(CABLE_STEP) + 1e-6).floor() as u64;
        let steps = due.saturating_sub(self.done).min(MAX_STEPS_PER_FRAME);
        self.done = due;
        if steps > 0 {
            self.changed = false;
        }
        steps
    }

    /// The work of every cable that moves, for the steps to come.
    pub fn plans(&self) -> Vec<CablePlan> {
        let mut plans: Vec<_> = self
            .live
            .iter()
            .filter(|(_, l)| !l.resting)
            .map(|(id, l)| {
                let (pins, step) = l.plan();
                CablePlan { id: *id, points: l.pos.len(), pins, step }
            })
            .collect();
        plans.sort_by_key(|p| p.id);
        plans
    }

    /// After `steps` steps ran elsewhere: cables on their way out move on, and the
    /// ones that arrived go.
    pub fn advance(&mut self, steps: u64) {
        for live in self.live.values_mut() {
            if !live.resting {
                for _ in 0..steps {
                    live.advance();
                }
            }
        }
        self.live.retain(|_, live| !live.gone());
    }

    /// Runs `steps` steps of every cable here, on the CPU.
    pub fn step_here(&mut self, steps: u64) {
        for live in self.live.values_mut() {
            if live.resting {
                continue;
            }
            for _ in 0..steps {
                let (pins, step) = live.plan();
                let pinned = pinned_of(&pins, live.pos.len());
                live.moved = cable_step(&mut live.pos, &mut live.prev, &pinned, step);
                live.advance();
            }
        }
        self.live.retain(|_, live| !live.gone());
    }

    /// The cables whose points the book set since the last call, with those points.
    pub fn take_placed(&mut self) -> Vec<(u64, Vec<Point>)> {
        let mut out: Vec<_> =
            self.placed.drain().filter_map(|id| self.live.get(&id).map(|l| (id, l.pos.clone()))).collect();
        out.sort_by_key(|(id, _)| *id);
        out
    }

    /// Where a cable's points are, as read back from elsewhere.
    pub fn set_points(&mut self, id: u64, points: &[Point]) {
        if let Some(live) = self.live.get_mut(&id)
            && live.pos.len() == points.len()
        {
            live.pos.copy_from_slice(points);
        }
    }

    /// How far the farthest point moved in the latest step, as measured elsewhere.
    pub fn set_moved(&mut self, moved: f32) {
        for live in self.live.values_mut() {
            if !live.resting {
                live.moved = moved;
            }
        }
    }

    /// Every cable the book has, held or leaving, by id.
    pub fn all(&self) -> Vec<u64> {
        let mut ids: Vec<_> = self.live.keys().copied().collect();
        ids.sort_unstable();
        ids
    }

    /// See [`CableWorld::moving`].
    pub fn moving(&self) -> bool {
        !self.reduced
            && (self.changed
                || self
                    .live
                    .values()
                    .any(|l| !l.resting && (l.moved > CABLE_STILL || l.phase != Phase::Held)))
    }

    /// See [`CableWorld::shape`].
    pub fn shape(&self, id: u64) -> Option<Arc<[Point]>> {
        let live = self.live.get(&id)?;
        (live.phase == Phase::Held).then(|| Arc::from(live.pos.as_slice()))
    }

    /// See [`CableWorld::ids`].
    pub fn ids(&self) -> Vec<u64> {
        let mut ids: Vec<_> =
            self.live.iter().filter(|(_, l)| l.phase == Phase::Held).map(|(id, _)| *id).collect();
        ids.sort_unstable();
        ids
    }

    /// See [`CableWorld::leaving`].
    pub fn leaving(&self) -> Vec<Cable> {
        let mut out: Vec<_> =
            self.live.values().filter(|l| l.phase != Phase::Held).map(Live::placed).collect();
        out.sort_by_key(|c| c.id);
        out
    }
}

/// The reference world, on the CPU: what runs where the GPU cannot (WebGL2, tests,
/// exports) and what the GPU world is checked against.
#[derive(Debug, Default)]
pub struct CpuCables {
    book: CableBook,
}

impl CpuCables {
    /// An empty world.
    pub fn new() -> Self {
        Self::default()
    }

    /// Steps run since the world began.
    pub fn steps(&self) -> u64 {
        self.book.steps()
    }
}

impl CableWorld for CpuCables {
    fn sync(&mut self, cables: &[Cable], view: Rect, reduced: bool) {
        self.book.sync(cables, view, reduced);
        self.book.placed.clear();
    }

    fn step(&mut self, dt: f32) {
        let steps = self.book.due(dt);
        if steps > 0 {
            self.book.step_here(steps);
        }
    }

    fn moving(&self) -> bool {
        self.book.moving()
    }

    fn shape(&self, id: u64) -> Option<Arc<[Point]>> {
        self.book.shape(id)
    }

    fn ids(&self) -> Vec<u64> {
        self.book.ids()
    }

    fn leaving(&self) -> Vec<Cable> {
        self.book.leaving()
    }
}

// ---------------------------------------------------------------------------------------------
// Drawing as plain shapes

/// The two sides of a line through `points`, `half` away: a ribbon outline.
fn ribbon(points: &[Point], half: f32, offset: f32) -> Vec<PathElement> {
    if points.len() < 2 {
        return Vec::new();
    }
    let normal = |i: usize| {
        let (a, b) = (points[i.saturating_sub(1)], points[(i + 1).min(points.len() - 1)]);
        let (dx, dy) = (b.x - a.x, b.y - a.y);
        let length = dx.hypot(dy).max(f32::EPSILON);
        (-dy / length, dx / length)
    };
    let mut outline = Vec::with_capacity(points.len() * 2 + 2);
    for (i, p) in points.iter().enumerate() {
        let (nx, ny) = normal(i);
        let at = Point::new(p.x + nx * (offset + half), p.y + ny * (offset + half));
        outline.push(if i == 0 { PathElement::MoveTo(at) } else { PathElement::LineTo(at) });
    }
    for (i, p) in points.iter().enumerate().rev() {
        let (nx, ny) = normal(i);
        outline.push(PathElement::LineTo(Point::new(p.x + nx * (offset - half), p.y + ny * (offset - half))));
    }
    outline.push(PathElement::Close);
    outline
}

/// Cuts a line into the pieces a dash pattern keeps.
fn dashed(points: &[Point], dash: f32, gap: f32) -> Vec<Vec<Point>> {
    let mut out = Vec::new();
    let (mut on, mut left) = (true, dash.max(0.5));
    let mut piece = vec![points[0]];
    for w in points.windows(2) {
        let (mut a, b) = (w[0], w[1]);
        let mut length = (b.x - a.x).hypot(b.y - a.y);
        while length > left {
            let t = left / length;
            let cut = Point::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t);
            if on {
                piece.push(cut);
                out.push(std::mem::take(&mut piece));
            } else {
                piece = vec![cut];
            }
            length -= left;
            a = cut;
            on = !on;
            left = if on { dash.max(0.5) } else { gap.max(0.5) };
        }
        left -= length;
        if on {
            piece.push(b);
        }
    }
    if on && piece.len() > 1 {
        out.push(piece);
    }
    out
}

/// Direction from `b` to `a`, unit length.
fn unit(a: Point, b: Point) -> Point {
    let (dx, dy) = (a.x - b.x, a.y - b.y);
    let length = dx.hypot(dy).max(f32::EPSILON);
    Point::new(dx / length, dy / length)
}

/// A plug at `at`, its body running along `along` (unit) away from the jack.
fn plug(at: Point, along: Point, look: &CableLook) -> [Primitive; 2] {
    let w = look.thickness;
    let (length, width) = (w * 2.6 + 6.0, w * 1.5 + 4.0);
    let center = Point::new(at.x + along.x * length / 2.0, at.y + along.y * length / 2.0);
    let angle = along.y.atan2(along.x);
    let color = look.plug_color.with_alpha(look.plug_color.a * look.opacity);
    let body = Quad::new(Rect::new(center.x - length / 2.0, center.y - width / 2.0, length, width), color)
        .radius(width * 0.3)
        .rotated(angle);
    let tip_size = width * 0.5;
    let tip = Quad::new(
        Rect::new(at.x - tip_size / 2.0, at.y - tip_size / 2.0, tip_size, tip_size),
        color.lerp(Color::WHITE, 0.35),
    )
    .radius(tip_size * 0.2)
    .rotated(angle);
    [Primitive::Quad(body), Primitive::Quad(tip)]
}

/// Draws `cable` at `points` as shapes, quads and pictures: what every output that
/// is not the GPU shows. Its body, then its plugs.
pub fn cable_primitives(cable: &Cable, points: &[Point]) -> Vec<Primitive> {
    let mut out = cable_body(cable, points);
    out.extend(cable_plugs(cable, points));
    out
}

/// The body of `cable` at `points` as shapes, without its plugs: to draw the bodies of a
/// run of cables together and their plugs after, in two draws instead of two a cable.
pub fn cable_body(cable: &Cable, points: &[Point]) -> Vec<Primitive> {
    let (cable, points) = &in_scene(cable, points);
    let look = &cable.look;
    let mut out = Vec::new();
    if points.len() < 2 || look.opacity <= 0.0 {
        return out;
    }
    let half = look.thickness / 2.0;
    let pieces = match look.dash {
        Some([dash, gap]) => dashed(points, dash, gap),
        None => vec![points.to_vec()],
    };
    let fade = |c: Color| c.with_alpha(c.a * look.opacity);
    for piece in &pieces {
        if piece.len() < 2 {
            continue;
        }
        let body = ribbon(piece, half, 0.0);
        match &look.texture {
            CableTexture::Image { image, repeat } => {
                out.extend(textured(piece, half, image, repeat.max(1.0), look.opacity));
            }
            texture => {
                out.push(Primitive::Path(PathFill::solid(body, fade(look.color))));
                match texture {
                    CableTexture::Striped(stripe) => {
                        out.push(Primitive::Path(PathFill::solid(
                            ribbon(piece, half * 0.3, 0.0),
                            fade(*stripe),
                        )));
                    }
                    CableTexture::Braided => out.extend(marks(piece, look, 0.9, 0.55)),
                    CableTexture::Coiled => out.extend(marks(piece, look, 0.6, 0.0)),
                    _ => {}
                }
            }
        }
        if look.shine > 0.0 {
            let highlight = look.color.lerp(Color::WHITE, 0.55);
            let alpha = look.shine.clamp(0.0, 1.0) * 0.7;
            out.push(Primitive::Path(PathFill::solid(
                ribbon(piece, half * 0.22, -half * 0.38),
                fade(highlight.with_alpha(alpha)),
            )));
        }
    }
    out
}

/// The points of a smooth curve through `points` (Catmull-Rom), [`CABLE_SMOOTHING`]
/// pieces to a segment.
pub fn cable_curve(points: &[Point]) -> Vec<Point> {
    let n = points.len();
    if n < 3 {
        return points.to_vec();
    }
    let at = |k: isize| points[k.clamp(0, n as isize - 1) as usize];
    let mut out = Vec::with_capacity((n - 1) * CABLE_SMOOTHING + 1);
    for k in 0..n - 1 {
        let (p0, p1, p2, p3) = (at(k as isize - 1), at(k as isize), at(k as isize + 1), at(k as isize + 2));
        for sub in 0..CABLE_SMOOTHING {
            let t = sub as f32 / CABLE_SMOOTHING as f32;
            let (t2, t3) = (t * t, t * t * t);
            let f = |a: f32, b: f32, c: f32, d: f32| {
                0.5 * (2.0 * b
                    + (-a + c) * t
                    + (2.0 * a - 5.0 * b + 4.0 * c - d) * t2
                    + (-a + 3.0 * b - 3.0 * c + d) * t3)
            };
            out.push(Point::new(f(p0.x, p1.x, p2.x, p3.x), f(p0.y, p1.y, p2.y, p3.y)));
        }
    }
    out.push(points[n - 1]);
    out
}

/// The cable and its points in scene coordinates: through its view, thickness too.
fn in_scene(cable: &Cable, points: &[Point]) -> (Cable, Vec<Point>) {
    let mut c = cable.clone();
    c.look.thickness *= cable.view.scale;
    c.view = CableView::default();
    let placed: Vec<Point> = points.iter().map(|p| cable.view.apply(*p)).collect();
    (c, cable_curve(&placed))
}

/// The plugs of `cable` at `points`, when its look has them: what the GPU leaves to
/// the quads.
pub fn cable_plugs(cable: &Cable, points: &[Point]) -> Vec<Primitive> {
    let (cable, points) = in_scene(cable, points);
    plugs_of(&cable, &points)
}

fn plugs_of(cable: &Cable, points: &[Point]) -> Vec<Primitive> {
    let look = &cable.look;
    let mut out = Vec::new();
    if !look.plugs || points.len() < 2 || look.opacity <= 0.0 {
        return out;
    }
    out.extend(plug(points[0], unit(points[1], points[0]), look));
    if cable.to.toward.is_some() || cable.points.is_some() {
        let n = points.len();
        out.extend(plug(points[n - 1], unit(points[n - 2], points[n - 1]), look));
    }
    out
}

/// Short darker strokes across the cable, every `spacing` widths, slanted by `slant`:
/// the weave of a braid, the turns of a coil.
fn marks(points: &[Point], look: &CableLook, spacing: f32, slant: f32) -> Vec<Primitive> {
    let w = look.thickness;
    let every = (w * spacing).max(2.0);
    let color = look.color.lerp(Color::BLACK, 0.35);
    let color = color.with_alpha(color.a * look.opacity);
    let mut out = Vec::new();
    let mut left = every / 2.0;
    for seg in points.windows(2) {
        let (a, b) = (seg[0], seg[1]);
        let length = (b.x - a.x).hypot(b.y - a.y);
        if length <= f32::EPSILON {
            continue;
        }
        let d = Point::new((b.x - a.x) / length, (b.y - a.y) / length);
        let n = Point::new(-d.y, d.x);
        let mut t = left;
        while t < length {
            let c = Point::new(a.x + d.x * t, a.y + d.y * t);
            let h = w / 2.0;
            let p0 = Point::new(c.x + n.x * h - d.x * slant * h, c.y + n.y * h - d.y * slant * h);
            let p1 = Point::new(c.x - n.x * h + d.x * slant * h, c.y - n.y * h + d.y * slant * h);
            out.push(Primitive::Path(PathFill::solid(ribbon(&[p0, p1], (w * 0.09).max(0.6), 0.0), color)));
            t += every;
        }
        left = t - length;
    }
    out
}

/// The cable covered with `image`, repeated every `repeat` pixels: each piece of a
/// segment gets its own slice of the picture.
fn textured(points: &[Point], half: f32, image: &ImageData, repeat: f32, opacity: f32) -> Vec<Primitive> {
    let mut out = Vec::new();
    let mut along = 0.0f32;
    for seg in points.windows(2) {
        let (a, b) = (seg[0], seg[1]);
        let length = (b.x - a.x).hypot(b.y - a.y);
        if length <= f32::EPSILON {
            continue;
        }
        let d = Point::new((b.x - a.x) / length, (b.y - a.y) / length);
        let n = Point::new(-d.y, d.x);
        let mut t = 0.0;
        while t < length {
            let into = (along + t) % repeat;
            let piece = (repeat - into).min(length - t);
            let start = Point::new(a.x + d.x * t, a.y + d.y * t);
            let end = Point::new(start.x + d.x * piece, start.y + d.y * piece);
            // A hair longer, so pieces leave no seam.
            let over = Point::new(d.x * 0.3, d.y * 0.3);
            let outline = vec![
                PathElement::MoveTo(Point::new(start.x - over.x + n.x * half, start.y - over.y + n.y * half)),
                PathElement::LineTo(Point::new(end.x + over.x + n.x * half, end.y + over.y + n.y * half)),
                PathElement::LineTo(Point::new(end.x + over.x - n.x * half, end.y + over.y - n.y * half)),
                PathElement::LineTo(Point::new(start.x - over.x - n.x * half, start.y - over.y - n.y * half)),
                PathElement::Close,
            ];
            // The picture's square goes to one whole copy: its left edge `into` behind
            // the piece's start, its x along the cable and its y across it.
            let origin = Point::new(start.x - d.x * into - n.x * half, start.y - d.y * into - n.y * half);
            let placement =
                [d.x * repeat, d.y * repeat, n.x * half * 2.0, n.y * half * 2.0, origin.x, origin.y];
            let mut fill = PathFill::solid(outline, Color::WHITE);
            fill.paint = PathPaint::Image { image: image.clone(), placement };
            fill.opacity = opacity;
            out.push(Primitive::Path(fill));
            t += piece;
        }
        along += length;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line() -> Cable {
        Cable::new(
            1,
            CableEnd::at(Point::new(0.0, 0.0)).toward(1.0, 0.0),
            CableEnd::at(Point::new(300.0, 0.0)).toward(-1.0, 0.0),
        )
    }

    fn view() -> Rect {
        Rect::new(-500.0, -500.0, 2000.0, 2000.0)
    }

    fn run(world: &mut CpuCables, cables: &[Cable], seconds: f32, fps: f32) {
        let frames = (seconds * fps).round() as usize;
        for _ in 0..frames {
            world.sync(cables, view(), false);
            world.step(1.0 / fps);
        }
    }

    #[test]
    fn a_new_cable_drops_and_settles_with_a_belly_in_under_a_second() {
        let mut world = CpuCables::new();
        let cable = Cable { grow: true, ..line() };
        world.sync(std::slice::from_ref(&cable), view(), false);
        assert!(world.moving());
        let start = world.shape(1).unwrap();
        assert!(start.iter().all(|p| p.y.abs() < 0.01), "starts taut");
        run(&mut world, std::slice::from_ref(&cable), 1.0, 60.0);
        assert!(!world.moving(), "still moving after a second");
        let shape = world.shape(1).unwrap();
        let lowest = shape.iter().map(|p| p.y).fold(f32::MIN, f32::max);
        assert!(lowest > 30.0, "no belly: {lowest}");
    }

    #[test]
    fn no_segment_stretches_past_its_limit() {
        let mut world = CpuCables::new();
        let cable = Cable { grow: true, ..line() };
        run(&mut world, std::slice::from_ref(&cable), 0.5, 60.0);
        let shape = world.shape(1).unwrap();
        let pins = cable.pins(1.0, None);
        let rest = step_of(&cable.physics, Cable::span(&pins), 1.0).rest;
        for w in shape.windows(2) {
            let length = (w[1].x - w[0].x).hypot(w[1].y - w[0].y);
            assert!(length <= rest * 1.05 + 0.01, "{length} > {rest}");
        }
    }

    #[test]
    fn the_same_events_give_the_same_shape_at_30_and_at_120_frames_per_second() {
        let cable = Cable { grow: true, ..line() };
        let moved = Cable { to: CableEnd::at(Point::new(200.0, 120.0)), ..cable.clone() };
        let mut slow = CpuCables::new();
        run(&mut slow, std::slice::from_ref(&cable), 0.5, 30.0);
        run(&mut slow, std::slice::from_ref(&moved), 0.5, 30.0);
        let mut fast = CpuCables::new();
        run(&mut fast, std::slice::from_ref(&cable), 0.5, 120.0);
        run(&mut fast, std::slice::from_ref(&moved), 0.5, 120.0);
        assert_eq!(slow.steps(), fast.steps());
        assert_eq!(slow.shape(1), fast.shape(1));
    }

    #[test]
    fn a_settled_cable_matches_its_rest() {
        let mut world = CpuCables::new();
        let cable = Cable { grow: true, ..line() };
        run(&mut world, std::slice::from_ref(&cable), 3.0, 60.0);
        let shape = world.shape(1).unwrap();
        let rest = cable_rest(&cable);
        for (a, b) in shape.iter().zip(&rest) {
            assert!((a.x - b.x).abs() < 1.5 && (a.y - b.y).abs() < 1.5, "{a:?} vs {b:?}");
        }
    }

    #[test]
    fn reduced_motion_shows_the_rest_and_never_moves() {
        let mut world = CpuCables::new();
        let cable = Cable { grow: true, ..line() };
        world.sync(std::slice::from_ref(&cable), view(), true);
        assert!(!world.moving());
        assert_eq!(world.shape(1).unwrap().to_vec(), cable_rest(&cable));
    }

    #[test]
    fn a_cable_let_go_falls_and_is_gone_and_a_retracted_one_goes_home() {
        for release in [CableRelease::Fall, CableRelease::Retract] {
            let mut world = CpuCables::new();
            let cable = Cable { release, group: 7, ..line() };
            run(&mut world, std::slice::from_ref(&cable), 0.2, 60.0);
            // Another cable of the same group stays, so the group is still there.
            let other = Cable { id: 2, group: 7, ..line() };
            run(&mut world, std::slice::from_ref(&other), 0.1, 60.0);
            assert_eq!(world.leaving().len(), 1, "{release:?}");
            assert!(world.moving());
            run(&mut world, std::slice::from_ref(&other), 2.0, 60.0);
            assert!(world.leaving().is_empty(), "{release:?} still leaving");
        }
    }

    #[test]
    fn a_group_that_is_gone_takes_its_cables_without_a_fall() {
        let mut world = CpuCables::new();
        let cable = Cable { release: CableRelease::Fall, group: 3, ..line() };
        run(&mut world, std::slice::from_ref(&cable), 0.1, 60.0);
        run(&mut world, &[], 0.1, 60.0);
        assert!(world.leaving().is_empty());
        assert!(world.shape(1).is_none());
    }

    #[test]
    fn plugs_textures_and_dashes_draw_shapes() {
        let cable = line();
        let points = cable_rest(&cable);
        let plain = cable_primitives(&cable, &points);
        assert!(plain.iter().all(|p| matches!(p, Primitive::Path(_))));
        let mut fancy = cable.clone();
        fancy.look.plugs = true;
        fancy.look.texture = CableTexture::Braided;
        fancy.look.dash = Some([8.0, 4.0]);
        let drawn = cable_primitives(&fancy, &points);
        assert!(drawn.iter().filter(|p| matches!(p, Primitive::Quad(_))).count() == 4);
        assert!(drawn.len() > plain.len());
        let image = ImageData::from_fn(4, 4, |x, _| if x < 2 { Color::BLACK } else { Color::WHITE }).unwrap();
        fancy.look.texture = CableTexture::Image { image, repeat: 20.0 };
        let pictures = cable_primitives(&fancy, &points);
        assert!(
            pictures
                .iter()
                .any(|p| matches!(p, Primitive::Path(f) if matches!(f.paint, PathPaint::Image { .. })))
        );
    }
}
