//! Lines at right angles that go around the nodes in their way, and a cache so that a line is
//! worked out again only when something near it changes.
//!
//! A [`EdgeShape::Step`](super::EdgeShape::Step) line first tries the usual three runs; when
//! they cross a node, it searches a coarse grid made of the edges of the nodes near it (each
//! with some room around) for the shortest way at right angles that crosses none of them, each
//! turn costing as much as a stretch of line, so that clean routes win.

use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};

use ravenrs_core::{Point, Rect};

use super::{PORT_RUN, PortSide};

/// The room a line keeps from a node it goes around, in units of the diagram.
pub const ROUTE_MARGIN: f32 = 12.0;
/// How far around the ends of a line a node can be and still be in its way.
const NEAR: f32 = 240.0;
/// What a turn costs, in units of length.
const TURN_COST: f32 = 40.0;

/// The corners of a line at right angles from `from`, leaving a port on `from_side`, to `to`,
/// arriving at a port on `to_side`, going around `obstacles` (with [`ROUTE_MARGIN`] of room).
/// The first and last points are the ports; the runs out of them come next.
pub fn route_around(
    from: Point,
    from_side: PortSide,
    to: Point,
    to_side: PortSide,
    obstacles: &[Rect],
) -> Vec<Point> {
    let plain = super::step_points(from, from_side, to, to_side);
    let walls: Vec<Rect> = obstacles.iter().map(|r| r.inflate(ROUTE_MARGIN)).collect();
    let clear = |points: &[Point]| {
        points.windows(2).skip(1).take(points.len().saturating_sub(3)).all(|w| !blocked(w[0], w[1], &walls))
    };
    if clear(&plain) {
        return plain;
    }
    let (a, b) = (from_side.outward(), to_side.outward());
    let start = Point::new(from.x + a.x * PORT_RUN, from.y + a.y * PORT_RUN);
    let end = Point::new(to.x + b.x * PORT_RUN, to.y + b.y * PORT_RUN);
    match search(start, from_side.is_horizontal(), end, &walls) {
        Some(mut middle) => {
            let mut points = vec![from];
            points.append(&mut middle);
            points.push(to);
            tidy(points)
        }
        // Boxed in on every side: the plain line, through whatever is there.
        None => plain,
    }
}

/// Whether the straight run from `p` to `q` (across or down) goes through the inside of a wall.
fn blocked(p: Point, q: Point, walls: &[Rect]) -> bool {
    walls.iter().any(|r| {
        if (p.y - q.y).abs() < 1e-3 {
            let (x0, x1) = (p.x.min(q.x), p.x.max(q.x));
            p.y > r.y() && p.y < r.max_y() && x1 > r.x() && x0 < r.max_x()
        } else {
            let (y0, y1) = (p.y.min(q.y), p.y.max(q.y));
            p.x > r.x() && p.x < r.max_x() && y1 > r.y() && y0 < r.max_y()
        }
    })
}

/// The shortest way at right angles from `start` to `end` on the grid of the walls' edges,
/// leaving `start` across when `across`, each turn costing [`TURN_COST`]; its corners, ends
/// included.
fn search(start: Point, across: bool, end: Point, walls: &[Rect]) -> Option<Vec<Point>> {
    let mut xs = vec![start.x, end.x, (start.x + end.x) / 2.0];
    let mut ys = vec![start.y, end.y, (start.y + end.y) / 2.0];
    for r in walls {
        xs.extend([r.x(), r.max_x()]);
        ys.extend([r.y(), r.max_y()]);
    }
    for v in [&mut xs, &mut ys] {
        v.sort_by(f32::total_cmp);
        v.dedup_by(|a, b| (*a - *b).abs() < 1e-3);
    }
    let (w, h) = (xs.len(), ys.len());
    let index = |x: f32, y: f32| {
        let i = xs.iter().position(|v| (v - x).abs() < 1e-3)?;
        let j = ys.iter().position(|v| (v - y).abs() < 1e-3)?;
        Some(j * w + i)
    };
    let (from, to) = (index(start.x, start.y)?, index(end.x, end.y)?);
    let point = |k: usize| Point::new(xs[k % w], ys[k / w]);
    let inside =
        |p: Point| walls.iter().any(|r| p.x > r.x() && p.x < r.max_x() && p.y > r.y() && p.y < r.max_y());
    // States are (grid point, heading across?); a plain scan for the cheapest open state is
    // enough for the few dozen points near one line.
    let states = w * h * 2;
    let mut cost = vec![f32::INFINITY; states];
    let mut back = vec![usize::MAX; states];
    let mut done = vec![false; states];
    let first = from * 2 + usize::from(across);
    cost[first] = 0.0;
    loop {
        let open = (0..states)
            .filter(|&s| !done[s] && cost[s].is_finite())
            .min_by(|&a, &b| cost[a].total_cmp(&cost[b]))?;
        done[open] = true;
        let (k, heading_across) = (open / 2, open % 2 == 1);
        if k == to {
            let mut corners = vec![point(k)];
            let mut s = open;
            while back[s] != usize::MAX {
                s = back[s];
                corners.push(point(s / 2));
            }
            corners.reverse();
            return Some(corners);
        }
        let (i, j) = (k % w, k / w);
        let steps = [
            (i.checked_sub(1), Some(j), true),
            ((i + 1 < w).then_some(i + 1), Some(j), true),
            (Some(i), j.checked_sub(1), false),
            (Some(i), (j + 1 < h).then_some(j + 1), false),
        ];
        for (ni, nj, goes_across) in steps {
            let (Some(ni), Some(nj)) = (ni, nj) else { continue };
            let next = nj * w + ni;
            let (p, q) = (point(k), point(next));
            if (inside(q) && next != to) || blocked(p, q, walls) {
                continue;
            }
            let turn = if goes_across == heading_across { 0.0 } else { TURN_COST };
            let s = next * 2 + usize::from(goes_across);
            let c = cost[open] + (q.x - p.x).abs() + (q.y - p.y).abs() + turn;
            if c < cost[s] {
                cost[s] = c;
                back[s] = open;
            }
        }
    }
}

/// The corners only: points that do not turn the line are dropped.
fn tidy(points: Vec<Point>) -> Vec<Point> {
    let mut kept: Vec<Point> = Vec::with_capacity(points.len());
    for p in points {
        if kept.last().is_some_and(|q: &Point| (q.x - p.x).abs() < 1e-3 && (q.y - p.y).abs() < 1e-3) {
            continue;
        }
        if kept.len() >= 2 {
            let (u, v) = (kept[kept.len() - 2], kept[kept.len() - 1]);
            let cross = (v.x - u.x) * (p.y - v.y) - (v.y - u.y) * (p.x - v.x);
            if cross.abs() < 1e-3 {
                kept.pop();
            }
        }
        kept.push(p);
    }
    kept
}

/// The nodes near a line from `from` to `to`, whose boxes it must go around.
pub(super) fn near(from: Point, to: Point, boxes: impl Iterator<Item = Rect>) -> Vec<Rect> {
    let (x0, y0) = (from.x.min(to.x) - NEAR, from.y.min(to.y) - NEAR);
    let (x1, y1) = (from.x.max(to.x) + NEAR, from.y.max(to.y) + NEAR);
    boxes.filter(|r| r.max_x() > x0 && r.x() < x1 && r.max_y() > y0 && r.y() < y1).collect()
}

/// Lines already worked out, each kept with what it was worked out from, so that drawing a
/// diagram again works out only the lines whose ends or nearby nodes changed.
#[derive(Clone, Debug, Default)]
pub struct RouteCache {
    routes: HashMap<String, (u64, Vec<Point>)>,
    computed: usize,
}

impl RouteCache {
    /// Nothing worked out yet.
    pub fn new() -> Self {
        Self::default()
    }

    /// How many lines were worked out (not found already worked out) since it was made.
    pub fn computed(&self) -> usize {
        self.computed
    }

    /// The line `id` from these ends around these boxes, worked out unless it was already.
    pub(super) fn get(
        &mut self,
        id: &str,
        ends: (Point, PortSide, Point, PortSide),
        boxes: &[Rect],
    ) -> Vec<Point> {
        let mut hasher = DefaultHasher::new();
        for v in [ends.0.x, ends.0.y, ends.2.x, ends.2.y] {
            v.to_bits().hash(&mut hasher);
        }
        (ends.1 as u8, ends.3 as u8).hash(&mut hasher);
        for r in boxes {
            for v in [r.x(), r.y(), r.width(), r.height()] {
                v.to_bits().hash(&mut hasher);
            }
        }
        let key = hasher.finish();
        if let Some((known, points)) = self.routes.get(id)
            && *known == key
        {
            return points.clone();
        }
        self.computed += 1;
        let points = route_around(ends.0, ends.1, ends.2, ends.3, boxes);
        self.routes.insert(id.to_owned(), (key, points.clone()));
        points
    }
}
