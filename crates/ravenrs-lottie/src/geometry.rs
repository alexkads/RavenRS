//! Contours and what a Lottie does to them: transforms, cuts along their length and strokes.

use ravenrs_core::{PathElement, Point};

use crate::property::Bezier;

/// `[a, b, c, d, e, f]`, taking `(x, y)` to `(a x + c y + e, b x + d y + f)`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Matrix(pub [f32; 6]);

impl Matrix {
    pub(crate) const IDENTITY: Self = Self([1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);

    /// `self` after `inner`: `inner` happens first.
    pub(crate) fn then_inner(self, inner: Self) -> Self {
        let [a, b, c, d, e, f] = self.0;
        let [g, h, i, j, k, l] = inner.0;
        Self([
            a * g + c * h,
            b * g + d * h,
            a * i + c * j,
            b * i + d * j,
            a * k + c * l + e,
            b * k + d * l + f,
        ])
    }

    pub(crate) fn apply(self, p: (f32, f32)) -> Point {
        let [a, b, c, d, e, f] = self.0;
        Point::new(a * p.0 + c * p.1 + e, b * p.0 + d * p.1 + f)
    }

    /// How much it scales lengths, on average.
    pub(crate) fn mean_scale(self) -> f32 {
        let [a, b, c, d, ..] = self.0;
        (a * d - b * c).abs().sqrt()
    }

    /// Position, anchor, scale in percent, rotation and skew in degrees: a Lottie transform.
    pub(crate) fn of_transform(
        position: (f32, f32),
        anchor: (f32, f32),
        scale: (f32, f32),
        rotation: f32,
        skew: f32,
        skew_axis: f32,
    ) -> Self {
        let translate = |x: f32, y: f32| Self([1.0, 0.0, 0.0, 1.0, x, y]);
        let rotate = |deg: f32| {
            let (s, c) = deg.to_radians().sin_cos();
            Self([c, s, -s, c, 0.0, 0.0])
        };
        let mut m = translate(position.0, position.1).then_inner(rotate(rotation));
        if skew != 0.0 {
            let t = (-skew).to_radians().tan();
            m = m
                .then_inner(rotate(skew_axis))
                .then_inner(Self([1.0, 0.0, t, 1.0, 0.0, 0.0]))
                .then_inner(rotate(-skew_axis));
        }
        m.then_inner(Self([scale.0 / 100.0, 0.0, 0.0, scale.1 / 100.0, 0.0, 0.0]))
            .then_inner(translate(-anchor.0, -anchor.1))
    }
}

/// A run of cubic curves.
#[derive(Clone, Debug, Default)]
pub(crate) struct Contour {
    pub start: Point,
    pub curves: Vec<[Point; 3]>,
    pub closed: bool,
}

impl Contour {
    pub(crate) fn of_bezier(path: &Bezier, m: Matrix) -> Option<Self> {
        let first = *path.vertices.first()?;
        let mut curves = Vec::with_capacity(path.vertices.len());
        let count = path.vertices.len();
        let tangent = |list: &[(f32, f32)], i: usize| list.get(i).copied().unwrap_or((0.0, 0.0));
        let end = if path.closed { count } else { count - 1 };
        for i in 0..end {
            let (from, to) = (path.vertices[i], path.vertices[(i + 1) % count]);
            let (o, n) = (tangent(&path.tangents_out, i), tangent(&path.tangents_in, (i + 1) % count));
            curves.push([
                m.apply((from.0 + o.0, from.1 + o.1)),
                m.apply((to.0 + n.0, to.1 + n.1)),
                m.apply(to),
            ]);
        }
        Some(Self { start: m.apply(first), curves, closed: path.closed })
    }

    fn line_curve(from: Point, to: Point) -> [Point; 3] {
        let at = |t: f32| Point::new(from.x + (to.x - from.x) * t, from.y + (to.y - from.y) * t);
        [at(1.0 / 3.0), at(2.0 / 3.0), to]
    }

    pub(crate) fn polygon(points: &[Point], closed: bool) -> Self {
        let Some(first) = points.first().copied() else { return Self::default() };
        let mut curves: Vec<[Point; 3]> =
            points.windows(2).map(|pair| Self::line_curve(pair[0], pair[1])).collect();
        if closed && let Some(last) = points.last() {
            curves.push(Self::line_curve(*last, first));
        }
        Self { start: first, curves, closed }
    }

    /// As straight pieces, no further from the curve than `tolerance`.
    pub(crate) fn flatten(&self, tolerance: f32) -> Vec<Point> {
        let mut points = vec![self.start];
        let mut pen = self.start;
        for [c1, c2, end] in &self.curves {
            let ddx = (pen.x - 2.0 * c1.x + c2.x).abs().max((c1.x - 2.0 * c2.x + end.x).abs());
            let ddy = (pen.y - 2.0 * c1.y + c2.y).abs().max((c1.y - 2.0 * c2.y + end.y).abs());
            let bend = (ddx * ddx + ddy * ddy).sqrt();
            let pieces = ((0.75 * bend / tolerance).sqrt().ceil() as usize).clamp(1, 96);
            for i in 1..=pieces {
                let t = i as f32 / pieces as f32;
                let u = 1.0 - t;
                let (a, b, c, d) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
                points.push(Point::new(
                    a * pen.x + b * c1.x + c * c2.x + d * end.x,
                    a * pen.y + b * c1.y + c * c2.y + d * end.y,
                ));
            }
            pen = *end;
        }
        points
    }

    pub(crate) fn append_to(&self, out: &mut Vec<PathElement>) {
        out.push(PathElement::MoveTo(self.start));
        for [c1, c2, end] in &self.curves {
            out.push(PathElement::CubicTo(*c1, *c2, *end));
        }
        if self.closed {
            out.push(PathElement::Close);
        }
    }
}

fn distance(a: Point, b: Point) -> f32 {
    ((a.x - b.x).powi(2) + (a.y - b.y).powi(2)).sqrt()
}

fn lerp(a: Point, b: Point, t: f32) -> Point {
    Point::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t)
}

/// The stretch of a polyline between two lengths along it.
fn stretch(points: &[Point], from: f32, to: f32) -> Vec<Point> {
    let mut out = Vec::new();
    let mut walked = 0.0;
    for pair in points.windows(2) {
        let length = distance(pair[0], pair[1]);
        let (start, end) = (walked, walked + length);
        walked = end;
        if end < from || start > to || length <= 0.0 {
            continue;
        }
        if out.is_empty() {
            out.push(lerp(pair[0], pair[1], ((from - start) / length).clamp(0.0, 1.0)));
        }
        out.push(lerp(pair[0], pair[1], ((to - start) / length).clamp(0.0, 1.0)));
    }
    out
}

/// Keeps the part of the contours between `start` and `end`, from 0 to 1 of their
/// length, shifted by `offset`, a trim path. `together` treats them as one long line.
pub(crate) fn trim(
    contours: &[Contour],
    start: f32,
    end: f32,
    offset: f32,
    together: bool,
    tolerance: f32,
) -> Vec<Contour> {
    let (mut from, mut to) = (start + offset, end + offset);
    if (to - from).abs() >= 1.0 {
        return contours.to_vec();
    }
    if from == to {
        return Vec::new();
    }
    if from > to {
        std::mem::swap(&mut from, &mut to);
    }
    let shift = from.floor();
    from -= shift;
    to -= shift;
    let lines: Vec<Vec<Point>> = contours
        .iter()
        .map(|contour| {
            let mut points = contour.flatten(tolerance);
            if contour.closed
                && let Some(first) = points.first().copied()
            {
                points.push(first);
            }
            points
        })
        .collect();
    let lengths: Vec<f32> =
        lines.iter().map(|line| line.windows(2).map(|p| distance(p[0], p[1])).sum()).collect();
    let cut = |line: &[Point], length: f32, a: f32, b: f32, out: &mut Vec<Contour>| {
        // Past the end of the line it comes around to the start.
        if b > 1.0 {
            let mut first = stretch(line, a * length, length);
            let second = stretch(line, 0.0, (b - 1.0) * length);
            first.extend(second.into_iter().skip(1));
            if first.len() >= 2 {
                out.push(Contour::polygon(&first, false));
            }
        } else {
            let piece = stretch(line, a * length, b * length);
            if piece.len() >= 2 {
                out.push(Contour::polygon(&piece, false));
            }
        }
    };
    let mut out = Vec::new();
    if together {
        let total: f32 = lengths.iter().sum();
        let (at_from, at_to) = (from * total, to.min(1.0) * total);
        let mut walked = 0.0;
        for (line, length) in lines.iter().zip(&lengths) {
            let (s, e) = (walked, walked + length);
            walked = e;
            if e <= at_from || s >= at_to || *length <= 0.0 {
                continue;
            }
            cut(line, *length, ((at_from - s) / length).max(0.0), ((at_to - s) / length).min(1.0), &mut out);
        }
    } else {
        for (line, length) in lines.iter().zip(&lengths) {
            if *length > 0.0 {
                cut(line, *length, from, to, &mut out);
            }
        }
    }
    out
}

/// Cuts contours into dashes, `pattern` being on and off lengths in turn.
pub(crate) fn dash(contours: &[Contour], pattern: &[f32], offset: f32, tolerance: f32) -> Vec<Contour> {
    let total: f32 = pattern.iter().sum();
    if total <= 0.0 {
        return contours.to_vec();
    }
    let mut out = Vec::new();
    for contour in contours {
        let mut line = contour.flatten(tolerance);
        if contour.closed
            && let Some(first) = line.first().copied()
        {
            line.push(first);
        }
        let length: f32 = line.windows(2).map(|p| distance(p[0], p[1])).sum();
        let mut at = -offset.rem_euclid(total);
        let mut i = 0;
        while at < length {
            let step = pattern[i % pattern.len()];
            if i % 2 == 0 && at + step > 0.0 {
                let piece = stretch(&line, at.max(0.0), (at + step).min(length));
                if piece.len() >= 2 {
                    out.push(Contour::polygon(&piece, false));
                }
            }
            at += step;
            i += 1;
            if step <= 0.0 && i > pattern.len() * 2 {
                break;
            }
        }
    }
    out
}

/// How a stroke looks.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct StrokeStyle {
    pub width: f32,
    /// 1 butt, 2 round, 3 square.
    pub cap: i64,
    /// 1 miter, 2 round, 3 bevel.
    pub join: i64,
    pub miter_limit: f32,
}

/// Each piece goes the same way around, so that the winding of their overlaps adds up
/// and never cancels. Filled by the non-zero rule, they are the stroke.
fn push_piece(points: &[Point], out: &mut Vec<PathElement>) {
    let area: f32 = (0..points.len())
        .map(|i| {
            let (p, q) = (points[i], points[(i + 1) % points.len()]);
            p.x * q.y - q.x * p.y
        })
        .sum();
    let mut add = |p: Point, first: bool| {
        out.push(if first { PathElement::MoveTo(p) } else { PathElement::LineTo(p) });
    };
    if area >= 0.0 {
        for (i, p) in points.iter().enumerate() {
            add(*p, i == 0);
        }
    } else {
        for (i, p) in points.iter().rev().enumerate() {
            add(*p, i == 0);
        }
    }
    out.push(PathElement::Close);
}

fn round_piece(center: Point, radius: f32, tolerance: f32, out: &mut Vec<PathElement>) {
    let steps = ((std::f32::consts::PI
        / (1.0 - (tolerance / radius.max(tolerance)).min(0.5)).acos().max(0.05))
    .ceil() as usize)
        .clamp(8, 64);
    let points: Vec<Point> = (0..steps)
        .map(|i| {
            let (s, c) = (i as f32 / steps as f32 * std::f32::consts::TAU).sin_cos();
            Point::new(center.x + c * radius, center.y + s * radius)
        })
        .collect();
    push_piece(&points, out);
}

/// The outline of a stroke along the contours.
pub(crate) fn stroke(contours: &[Contour], style: StrokeStyle, tolerance: f32) -> Vec<PathElement> {
    let half = style.width / 2.0;
    let mut out = Vec::new();
    if half <= 0.0 {
        return out;
    }
    for contour in contours {
        let mut line = contour.flatten(tolerance);
        line.dedup_by(|a, b| distance(*a, *b) < 1e-4);
        if contour.closed && line.len() > 2 && distance(line[0], line[line.len() - 1]) < 1e-4 {
            line.pop();
        }
        if line.len() < 2 {
            if style.cap == 2
                && let Some(p) = line.first()
            {
                round_piece(*p, half, tolerance, &mut out);
            }
            continue;
        }
        let count = line.len();
        let segments = if contour.closed { count } else { count - 1 };
        let normal = |i: usize| {
            let (a, b) = (line[i % count], line[(i + 1) % count]);
            let length = distance(a, b).max(1e-6);
            Point::new(-(b.y - a.y) / length * half, (b.x - a.x) / length * half)
        };
        for i in 0..segments {
            let (a, b) = (line[i], line[(i + 1) % count]);
            let n = normal(i);
            push_piece(
                &[
                    Point::new(a.x + n.x, a.y + n.y),
                    Point::new(b.x + n.x, b.y + n.y),
                    Point::new(b.x - n.x, b.y - n.y),
                    Point::new(a.x - n.x, a.y - n.y),
                ],
                &mut out,
            );
        }
        // The joins between pieces. Straight pieces of a curve meet at small angles, and
        // only a wedge is missing there.
        let joins: Vec<(usize, usize)> = if contour.closed {
            (0..count).map(|i| (i, (i + 1) % count)).collect()
        } else {
            (0..count.saturating_sub(2)).map(|i| (i, i + 1)).collect()
        };
        for (before, after) in joins {
            let at = line[(before + 1) % count];
            let (n0, n1) = (normal(before), normal(after));
            let cross = n0.x * n1.y - n0.y * n1.x;
            if cross.abs() < 1e-6 {
                continue;
            }
            // The outside of the turn is on the side the normals point away from.
            let side = if cross > 0.0 { -1.0 } else { 1.0 };
            let (p0, p1) = (
                Point::new(at.x + n0.x * side, at.y + n0.y * side),
                Point::new(at.x + n1.x * side, at.y + n1.y * side),
            );
            let cos = ((n0.x * n1.x + n0.y * n1.y) / (half * half)).clamp(-1.0, 1.0);
            // A small angle on a flattened curve needs no more than the wedge.
            if cos > 0.995 {
                push_piece(&[at, p0, p1], &mut out);
                continue;
            }
            match style.join {
                2 => round_piece(at, half, tolerance, &mut out),
                1 => {
                    let half_cos = ((1.0 + cos) / 2.0).sqrt().max(1e-3);
                    if 1.0 / half_cos <= style.miter_limit {
                        let mid = Point::new((n0.x + n1.x) * side, (n0.y + n1.y) * side);
                        let length = (mid.x * mid.x + mid.y * mid.y).sqrt().max(1e-6);
                        let reach = half / half_cos;
                        let tip = Point::new(at.x + mid.x / length * reach, at.y + mid.y / length * reach);
                        push_piece(&[at, p0, tip, p1], &mut out);
                    } else {
                        push_piece(&[at, p0, p1], &mut out);
                    }
                }
                _ => push_piece(&[at, p0, p1], &mut out),
            }
        }
        if !contour.closed {
            let ends = [(line[0], line[1]), (line[count - 1], line[count - 2])];
            for (end, inner) in ends {
                match style.cap {
                    2 => round_piece(end, half, tolerance, &mut out),
                    3 => {
                        let length = distance(end, inner).max(1e-6);
                        let along =
                            Point::new((end.x - inner.x) / length * half, (end.y - inner.y) / length * half);
                        let across = Point::new(-along.y, along.x);
                        push_piece(
                            &[
                                Point::new(end.x + across.x, end.y + across.y),
                                Point::new(end.x + across.x + along.x, end.y + across.y + along.y),
                                Point::new(end.x - across.x + along.x, end.y - across.y + along.y),
                                Point::new(end.x - across.x, end.y - across.y),
                            ],
                            &mut out,
                        );
                    }
                    _ => {}
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: f32, y: f32) -> Point {
        Point::new(x, y)
    }

    fn line(a: Point, b: Point) -> Contour {
        Contour::polygon(&[a, b], false)
    }

    #[test]
    fn a_transform_moves_rotates_and_scales_around_its_anchor() {
        let m = Matrix::of_transform((10.0, 20.0), (5.0, 5.0), (200.0, 100.0), 0.0, 0.0, 0.0);
        let q = m.apply((5.0, 5.0));
        assert_eq!((q.x, q.y), (10.0, 20.0), "the anchor lands on the position");
        let q = m.apply((6.0, 5.0));
        assert_eq!((q.x, q.y), (12.0, 20.0));
        let r =
            Matrix::of_transform((0.0, 0.0), (0.0, 0.0), (100.0, 100.0), 90.0, 0.0, 0.0).apply((1.0, 0.0));
        assert!(r.x.abs() < 1e-5 && (r.y - 1.0).abs() < 1e-5, "{r:?}");
    }

    #[test]
    fn a_trim_keeps_the_stretch_asked_for_and_comes_around_on_an_offset() {
        let whole = [line(p(0.0, 0.0), p(100.0, 0.0))];
        let half = trim(&whole, 0.25, 0.75, 0.0, false, 0.1);
        let points = half[0].flatten(0.1);
        assert_eq!(points.first().map(|q| q.x), Some(25.0));
        assert_eq!(points.last().map(|q| q.x), Some(75.0));
        assert!(trim(&whole, 0.5, 0.5, 0.0, false, 0.1).is_empty(), "nothing left");
        assert_eq!(trim(&whole, 0.0, 1.0, 0.0, false, 0.1)[0].flatten(0.1).last().map(|q| q.x), Some(100.0));
        let square = Contour::polygon(&[p(0.0, 0.0), p(10.0, 0.0), p(10.0, 10.0), p(0.0, 10.0)], true);
        let wrapped = trim(&[square], 0.9, 1.1, 0.0, false, 0.1);
        let flat = wrapped[0].flatten(0.1);
        assert_eq!(flat.first().map(|q| (q.x, q.y)), Some((0.0, 4.0)));
        assert!(flat.last().is_some_and(|q| (q.x - 4.0).abs() < 1e-3 && q.y.abs() < 1e-3), "{flat:?}");
    }

    #[test]
    fn a_stroke_covers_its_width_and_a_cap_adds_to_its_length() {
        let bounds = |elements: &[PathElement]| {
            let mut b = [f32::MAX, f32::MAX, f32::MIN, f32::MIN];
            for e in elements {
                if let PathElement::MoveTo(q) | PathElement::LineTo(q) = e {
                    b = [b[0].min(q.x), b[1].min(q.y), b[2].max(q.x), b[3].max(q.y)];
                }
            }
            b
        };
        let style = StrokeStyle { width: 10.0, cap: 1, join: 2, miter_limit: 4.0 };
        let butt = stroke(&[line(p(0.0, 0.0), p(100.0, 0.0))], style, 0.1);
        assert_eq!(bounds(&butt), [0.0, -5.0, 100.0, 5.0]);
        let square = stroke(&[line(p(0.0, 0.0), p(100.0, 0.0))], StrokeStyle { cap: 3, ..style }, 0.1);
        assert_eq!(bounds(&square), [-5.0, -5.0, 105.0, 5.0]);
    }

    #[test]
    fn every_piece_of_a_stroke_winds_the_same_way() {
        let area = |points: &[Point]| -> f32 {
            (0..points.len())
                .map(|i| {
                    let (a, b) = (points[i], points[(i + 1) % points.len()]);
                    a.x * b.y - b.x * a.y
                })
                .sum()
        };
        let corner = Contour::polygon(&[p(0.0, 0.0), p(50.0, 0.0), p(50.0, 50.0)], false);
        let style = StrokeStyle { width: 8.0, cap: 2, join: 1, miter_limit: 4.0 };
        let elements = stroke(&[corner], style, 0.1);
        let mut piece = Vec::new();
        let mut pieces = 0;
        for e in &elements {
            match e {
                PathElement::MoveTo(q) | PathElement::LineTo(q) => piece.push(*q),
                PathElement::Close => {
                    assert!(area(&piece) >= 0.0, "{piece:?}");
                    piece.clear();
                    pieces += 1;
                }
                PathElement::CubicTo(..) => unreachable!("a stroke is made of lines"),
            }
        }
        assert!(pieces >= 5, "two sides, a join and two caps: {pieces}");
    }

    #[test]
    fn dashes_alternate_along_the_line() {
        let dashes = dash(&[line(p(0.0, 0.0), p(100.0, 0.0))], &[10.0, 10.0], 0.0, 0.1);
        assert_eq!(dashes.len(), 5);
        assert_eq!(dashes[1].flatten(0.1).first().map(|q| q.x), Some(20.0));
    }
}
