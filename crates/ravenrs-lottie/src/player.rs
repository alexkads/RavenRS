//! Works out what a moment of an animation looks like.

use std::collections::HashMap;
use std::sync::Arc;

use ravenrs_core::{Color, ColorStop, FillRule, PathClip, PathElement, PathFill, PathPaint, Point};

use crate::document::{Document, GradientPaint, Item, Layer, LayerKind, StrokeLook, Transform};
use crate::geometry::{Contour, Matrix, StrokeStyle, dash, stroke, trim};

/// A circle's handles, as a part of its radius.
const KAPPA: f32 = 0.552_284_8;

/// A trim path, worked out for this frame.
#[derive(Clone, Copy, Debug)]
struct TrimNow {
    start: f32,
    end: f32,
    offset: f32,
    together: bool,
}

struct Context<'a> {
    document: &'a Document,
    frame: f32,
    matrix: Matrix,
    opacity: f32,
    tolerance: f32,
    clip: Option<Arc<PathClip>>,
    /// Trims further down the lists around these items, which reach into their groups.
    trims: Vec<TrimNow>,
}

impl Context<'_> {
    fn with(&self, matrix: Matrix, opacity: f32) -> Context<'_> {
        Context {
            document: self.document,
            frame: self.frame,
            matrix,
            opacity,
            tolerance: self.tolerance,
            clip: self.clip.clone(),
            trims: self.trims.clone(),
        }
    }
}

fn matrix_of(transform: &Transform, frame: f32) -> Matrix {
    let position_at = |frame: f32| match &transform.split {
        Some((x, y)) => (x.first_at(frame, 0.0), y.first_at(frame, 0.0)),
        None => transform.position.pair_at(frame, (0.0, 0.0)),
    };
    let position = position_at(frame);
    let mut rotation = transform.rotation.first_at(frame, 0.0);
    if transform.orient {
        let (before, after) = (position_at(frame - 0.5), position_at(frame + 0.5));
        let (dx, dy) = (after.0 - before.0, after.1 - before.1);
        if dx != 0.0 || dy != 0.0 {
            rotation += dy.atan2(dx).to_degrees();
        }
    }
    Matrix::of_transform(
        position,
        transform.anchor.pair_at(frame, (0.0, 0.0)),
        transform.scale.pair_at(frame, (100.0, 100.0)),
        rotation,
        transform.skew.first_at(frame, 0.0),
        transform.skew_axis.first_at(frame, 0.0),
    )
}

fn color_of(value: &[f32], opacity: f32) -> Color {
    // Very old files keep colors from 0 to 255.
    let k = if value.iter().take(3).any(|v| *v > 1.0) { 255.0 } else { 1.0 };
    let at = |i: usize| value.get(i).copied().unwrap_or(0.0) / k;
    Color::rgba(at(0), at(1), at(2), opacity.clamp(0.0, 1.0))
}

fn elements_of(contours: &[Contour]) -> Vec<PathElement> {
    let mut elements = Vec::new();
    for contour in contours {
        contour.append_to(&mut elements);
    }
    elements
}

fn stops_of(paint: &GradientPaint, frame: f32) -> Arc<[ColorStop]> {
    let raw = paint.stops.at(frame);
    let count = if paint.count > 0 { paint.count } else { raw.len() / 4 };
    let colors = raw.get(..count * 4).unwrap_or(&raw);
    let alphas = raw.get(count * 4..).unwrap_or(&[]);
    // Alpha has stops of its own, at offsets of their own.
    let alpha_at = |offset: f32| -> f32 {
        let pairs: Vec<(f32, f32)> = alphas.as_chunks::<2>().0.iter().map(|p| (p[0], p[1])).collect();
        let Some(first) = pairs.first() else { return 1.0 };
        if offset <= first.0 {
            return first.1;
        }
        for pair in pairs.windows(2) {
            if offset <= pair[1].0 {
                let k = (offset - pair[0].0) / (pair[1].0 - pair[0].0).max(1e-6);
                return pair[0].1 + (pair[1].1 - pair[0].1) * k;
            }
        }
        pairs.last().map_or(1.0, |p| p.1)
    };
    colors
        .as_chunks::<4>()
        .0
        .iter()
        .map(|c| ColorStop { offset: c[0], color: Color::rgba(c[1], c[2], c[3], alpha_at(c[0])) })
        .collect()
}

fn gradient_of(paint: &GradientPaint, cx: &Context<'_>) -> PathPaint {
    let start = cx.matrix.apply(paint.start.pair_at(cx.frame, (0.0, 0.0)));
    let end = cx.matrix.apply(paint.end.pair_at(cx.frame, (0.0, 0.0)));
    let stops = stops_of(paint, cx.frame);
    if paint.radial {
        let radius = ((end.x - start.x).powi(2) + (end.y - start.y).powi(2)).sqrt();
        PathPaint::Radial { center: start, radius, stops }
    } else {
        PathPaint::Linear { start, end, stops }
    }
}

fn emit(
    out: &mut Vec<PathFill>,
    elements: Vec<PathElement>,
    rule: FillRule,
    paint: PathPaint,
    opacity: f32,
    cx: &Context<'_>,
) {
    if elements.is_empty() || opacity <= 0.0 {
        return;
    }
    out.push(PathFill {
        elements: elements.into(),
        rule,
        paint,
        transform: PathFill::IDENTITY,
        opacity: opacity.clamp(0.0, 1.0),
        clip: cx.clip.clone(),
    });
}

fn stroked(contours: &[Contour], look: &StrokeLook, cx: &Context<'_>) -> Vec<PathElement> {
    let scale = cx.matrix.mean_scale();
    let width = look.width.first_at(cx.frame, 0.0) * scale;
    let style = StrokeStyle { width, cap: look.cap, join: look.join, miter_limit: look.miter_limit };
    let (mut pattern, mut offset) = (Vec::new(), 0.0);
    for (name, value) in &look.dashes {
        let v = value.first_at(cx.frame, 0.0) * scale;
        if name == "o" {
            offset = v;
        } else {
            pattern.push(v);
        }
    }
    if pattern.len() % 2 == 1 {
        pattern.extend_from_within(..);
    }
    if pattern.iter().sum::<f32>() > 0.0 {
        stroke(&dash(contours, &pattern, offset, cx.tolerance), style, cx.tolerance)
    } else {
        stroke(contours, style, cx.tolerance)
    }
}

fn rounded_rect(center: (f32, f32), size: (f32, f32), radius: f32, m: Matrix) -> Contour {
    let (hw, hh) = (size.0 / 2.0, size.1 / 2.0);
    let r = radius.min(hw).min(hh).max(0.0);
    let (x0, y0, x1, y1) = (center.0 - hw, center.1 - hh, center.0 + hw, center.1 + hh);
    if r <= 0.0 {
        return Contour::polygon(
            &[m.apply((x1, y0)), m.apply((x1, y1)), m.apply((x0, y1)), m.apply((x0, y0))],
            true,
        );
    }
    let k = r * (1.0 - KAPPA);
    let line = |a: (f32, f32), b: (f32, f32)| {
        let at = |t: f32| m.apply((a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t));
        [at(1.0 / 3.0), at(2.0 / 3.0), m.apply(b)]
    };
    let curves = vec![
        line((x1, y0 + r), (x1, y1 - r)),
        [m.apply((x1, y1 - k)), m.apply((x1 - k, y1)), m.apply((x1 - r, y1))],
        line((x1 - r, y1), (x0 + r, y1)),
        [m.apply((x0 + k, y1)), m.apply((x0, y1 - k)), m.apply((x0, y1 - r))],
        line((x0, y1 - r), (x0, y0 + r)),
        [m.apply((x0, y0 + k)), m.apply((x0 + k, y0)), m.apply((x0 + r, y0))],
        line((x0 + r, y0), (x1 - r, y0)),
        [m.apply((x1 - k, y0)), m.apply((x1, y0 + k)), m.apply((x1, y0 + r))],
    ];
    Contour { start: m.apply((x1, y0 + r)), curves, closed: true }
}

fn ellipse(center: (f32, f32), size: (f32, f32), m: Matrix) -> Contour {
    let (rx, ry) = (size.0 / 2.0, size.1 / 2.0);
    let (kx, ky) = (rx * KAPPA, ry * KAPPA);
    let (x, y) = center;
    let curves = vec![
        [m.apply((x + kx, y - ry)), m.apply((x + rx, y - ky)), m.apply((x + rx, y))],
        [m.apply((x + rx, y + ky)), m.apply((x + kx, y + ry)), m.apply((x, y + ry))],
        [m.apply((x - kx, y + ry)), m.apply((x - rx, y + ky)), m.apply((x - rx, y))],
        [m.apply((x - rx, y - ky)), m.apply((x - kx, y - ry)), m.apply((x, y - ry))],
    ];
    Contour { start: m.apply((x, y - ry)), curves, closed: true }
}

/// A star or polygon, with `rounds` the part of a stretch of its circle each kind of
/// corner bends along: the outer ones first, then the inner ones.
fn star(
    center: (f32, f32),
    count: usize,
    turn: f32,
    radii: (f32, f32),
    rounds: (f32, f32),
    polygon: bool,
    m: Matrix,
) -> Contour {
    let corners = if polygon { count } else { count * 2 };
    let stretches = if polygon { corners * 4 } else { corners * 2 } as f32;
    // Each corner, with the points its curves leave from and arrive by.
    let shape: Vec<[Point; 3]> = (0..corners)
        .map(|i| {
            let outer = polygon || i % 2 == 0;
            let (radius, round) = if outer { (radii.0, rounds.0) } else { (radii.1, rounds.1) };
            let angle =
                turn - std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::TAU / corners as f32;
            let (sin, cos) = angle.sin_cos();
            let (x, y) = (center.0 + cos * radius, center.1 + sin * radius);
            let reach = std::f32::consts::TAU * radius / stretches * round;
            [
                m.apply((x + sin * reach, y - cos * reach)),
                m.apply((x, y)),
                m.apply((x - sin * reach, y + cos * reach)),
            ]
        })
        .collect();
    let curves = (0..corners)
        .map(|i| {
            let next = &shape[(i + 1) % corners];
            [shape[i][2], next[0], next[1]]
        })
        .collect();
    Contour { start: shape[0][1], curves, closed: true }
}

/// Rounds the corners between straight sides. Curved sides stay as they are.
fn rounded_corners(contour: &Contour, radius: f32) -> Contour {
    let mut vertices = vec![contour.start];
    vertices.extend(contour.curves.iter().map(|c| c[2]));
    if contour.closed && vertices.len() > 2 && vertices.first() == vertices.last() {
        vertices.pop();
    }
    let straight = contour.curves.iter().scan(contour.start, |from, c| {
        let to = c[2];
        let expected = |t: f32| Point::new(from.x + (to.x - from.x) * t, from.y + (to.y - from.y) * t);
        let near = |p: Point, q: Point| (p.x - q.x).abs() + (p.y - q.y).abs() < 0.5;
        let is_line = (near(c[0], expected(1.0 / 3.0)) && near(c[1], expected(2.0 / 3.0)))
            || (near(c[0], *from) && near(c[1], to));
        *from = to;
        Some(is_line)
    });
    if radius <= 0.0 || vertices.len() < 3 || !straight.into_iter().all(|s| s) {
        return contour.clone();
    }
    let count = vertices.len();
    let toward = |from: Point, to: Point, by: f32| {
        let length = ((to.x - from.x).powi(2) + (to.y - from.y).powi(2)).sqrt().max(1e-6);
        let k = (by / length).min(0.5);
        Point::new(from.x + (to.x - from.x) * k, from.y + (to.y - from.y) * k)
    };
    let corner_range: Vec<usize> =
        if contour.closed { (0..count).collect() } else { (1..count - 1).collect() };
    let mut out = Contour { closed: contour.closed, ..Contour::default() };
    let mut pen: Option<Point> = None;
    if !contour.closed {
        out.start = vertices[0];
        pen = Some(vertices[0]);
    }
    let line_to = |from: Point, to: Point| {
        let at = |t: f32| Point::new(from.x + (to.x - from.x) * t, from.y + (to.y - from.y) * t);
        [at(1.0 / 3.0), at(2.0 / 3.0), to]
    };
    for i in corner_range {
        let at = vertices[i];
        let (before, after) = (vertices[(i + count - 1) % count], vertices[(i + 1) % count]);
        let (a, b) = (toward(at, before, radius), toward(at, after, radius));
        match pen {
            Some(p) => out.curves.push(line_to(p, a)),
            None => out.start = a,
        }
        let mix = |p: Point, q: Point, k: f32| Point::new(p.x + (q.x - p.x) * k, p.y + (q.y - p.y) * k);
        out.curves.push([mix(a, at, KAPPA), mix(b, at, KAPPA), b]);
        pen = Some(b);
    }
    if let Some(p) = pen {
        let end = if contour.closed { out.start } else { vertices[count - 1] };
        out.curves.push(line_to(p, end));
    }
    out
}

fn trim_of(item: &Item, frame: f32) -> Option<TrimNow> {
    let Item::Trim { start, end, offset, together } = item else { return None };
    Some(TrimNow {
        start: start.first_at(frame, 0.0) / 100.0,
        end: end.first_at(frame, 100.0) / 100.0,
        offset: offset.first_at(frame, 0.0) / 360.0,
        together: *together,
    })
}

/// What a style sees of the outlines above it: trimmed by what reaches in from outside.
fn styled(outlines: &[Contour], cx: &Context<'_>) -> Vec<Contour> {
    let mut seen = outlines.to_vec();
    for t in &cx.trims {
        seen = trim(&seen, t.start, t.end, t.offset, t.together, cx.tolerance);
    }
    seen
}

/// Draws these items, top first, and hands back the outlines for the styles of the
/// groups around them.
fn items(list: &[Item], cx: &Context<'_>, out: &mut Vec<PathFill>) -> Vec<Contour> {
    let mut outlines: Vec<Contour> = Vec::new();
    let frame = cx.frame;
    // What is above a repeater is drawn by it, once per copy, and not on its own.
    let skip = list.iter().rposition(|item| matches!(item, Item::Repeater { .. })).unwrap_or(0);
    for (at, item) in list.iter().enumerate().skip(skip) {
        match item {
            Item::Group(inner, transform) => {
                let (matrix, opacity) = match transform {
                    Some(t) => (
                        cx.matrix.then_inner(matrix_of(t, frame)),
                        cx.opacity * t.opacity.first_at(frame, 100.0) / 100.0,
                    ),
                    None => (cx.matrix, cx.opacity),
                };
                let mut inner_cx = cx.with(matrix, opacity);
                inner_cx.trims.extend(list[at + 1..].iter().filter_map(|later| trim_of(later, frame)));
                outlines.extend(items(inner, &inner_cx, out));
            }
            Item::Path(shape) => outlines.extend(Contour::of_bezier(&shape.at(frame), cx.matrix)),
            Item::Rect { position, size, radius } => outlines.push(rounded_rect(
                position.pair_at(frame, (0.0, 0.0)),
                size.pair_at(frame, (0.0, 0.0)),
                radius.first_at(frame, 0.0),
                cx.matrix,
            )),
            Item::Ellipse { position, size } => outlines.push(ellipse(
                position.pair_at(frame, (0.0, 0.0)),
                size.pair_at(frame, (0.0, 0.0)),
                cx.matrix,
            )),
            Item::Star { polygon, position, rotation, points, inner, outer, inner_round, outer_round } => {
                let center = position.pair_at(frame, (0.0, 0.0));
                let count = points.first_at(frame, 5.0).round().max(3.0) as usize;
                let turn = rotation.first_at(frame, 0.0).to_radians();
                let radii = (outer.first_at(frame, 0.0), inner.first_at(frame, 0.0));
                let rounds =
                    (outer_round.first_at(frame, 0.0) / 100.0, inner_round.first_at(frame, 0.0) / 100.0);
                outlines.push(star(center, count, turn, radii, rounds, *polygon, cx.matrix));
            }
            Item::Fill { color, opacity, even_odd } => {
                let alpha = opacity.first_at(frame, 100.0) / 100.0 * cx.opacity;
                let rule = if *even_odd { FillRule::EvenOdd } else { FillRule::NonZero };
                emit(
                    out,
                    elements_of(&styled(&outlines, cx)),
                    rule,
                    PathPaint::Solid(color_of(&color.at(frame), 1.0)),
                    alpha,
                    cx,
                );
            }
            Item::GradientFill { paint, opacity, even_odd } => {
                let alpha = opacity.first_at(frame, 100.0) / 100.0 * cx.opacity;
                let rule = if *even_odd { FillRule::EvenOdd } else { FillRule::NonZero };
                emit(out, elements_of(&styled(&outlines, cx)), rule, gradient_of(paint, cx), alpha, cx);
            }
            Item::Stroke { color, opacity, look } => {
                let alpha = opacity.first_at(frame, 100.0) / 100.0 * cx.opacity;
                let elements = stroked(&styled(&outlines, cx), look, cx);
                emit(
                    out,
                    elements,
                    FillRule::NonZero,
                    PathPaint::Solid(color_of(&color.at(frame), 1.0)),
                    alpha,
                    cx,
                );
            }
            Item::GradientStroke { paint, opacity, look } => {
                let alpha = opacity.first_at(frame, 100.0) / 100.0 * cx.opacity;
                let elements = stroked(&styled(&outlines, cx), look, cx);
                emit(out, elements, FillRule::NonZero, gradient_of(paint, cx), alpha, cx);
            }
            Item::Trim { .. } => {
                if let Some(t) = trim_of(item, frame) {
                    outlines = trim(&outlines, t.start, t.end, t.offset, t.together, cx.tolerance);
                }
            }
            Item::RoundedCorners(radius) => {
                let r = radius.first_at(frame, 0.0) * cx.matrix.mean_scale();
                outlines = outlines.iter().map(|c| rounded_corners(c, r)).collect();
            }
            Item::Repeater { copies, offset, transform, start_opacity, end_opacity } => {
                let count = copies.first_at(frame, 1.0).max(0.0).ceil() as usize;
                // Here the anchor is only the center of the turn: position moves from it.
                let (px, py) = transform.position.pair_at(frame, (0.0, 0.0));
                let anchor = transform.anchor.pair_at(frame, (0.0, 0.0));
                let step = Matrix::of_transform(
                    (px + anchor.0, py + anchor.1),
                    anchor,
                    transform.scale.pair_at(frame, (100.0, 100.0)),
                    transform.rotation.first_at(frame, 0.0),
                    0.0,
                    0.0,
                );
                let shift = offset.first_at(frame, 0.0);
                let (first, last) = (
                    start_opacity.first_at(frame, 100.0) / 100.0,
                    end_opacity.first_at(frame, 100.0) / 100.0,
                );
                outlines.clear();
                // What is above it in the list is drawn again, once per copy.
                let earlier = &list[..at];
                for i in (0..count).rev() {
                    let mut m = cx.matrix;
                    for _ in 0..(i as f32 + shift).max(0.0).round() as usize {
                        m = m.then_inner(step);
                    }
                    let k = if count > 1 { i as f32 / (count - 1) as f32 } else { 0.0 };
                    let copy = cx.with(m, cx.opacity * (first + (last - first) * k));
                    outlines.extend(items(earlier, &copy, out));
                }
            }
        }
    }
    outlines
}

fn layer_matrix(layer: &Layer, by_index: &HashMap<i64, &Layer>, frame: f32) -> Matrix {
    let mut matrix = matrix_of(&layer.transform, frame);
    let mut parent = layer.parent;
    let mut guard = 0;
    while let Some(index) = parent {
        let Some(found) = by_index.get(&index) else { break };
        matrix = matrix_of(&found.transform, frame).then_inner(matrix);
        parent = found.parent;
        guard += 1;
        if guard > 64 {
            break;
        }
    }
    matrix
}

fn union_clip(paths: &[PathFill], inverted: bool) -> PathClip {
    let mut elements = Vec::new();
    for path in paths {
        elements.extend_from_slice(&path.elements);
    }
    PathClip { elements: elements.into(), rule: FillRule::NonZero, inverted }
}

fn masks_clip(layer: &Layer, frame: f32, matrix: Matrix) -> Option<PathClip> {
    if layer.masks.is_empty() {
        return None;
    }
    let contours_of = |subtract: bool| -> Vec<PathElement> {
        let contours: Vec<Contour> = layer
            .masks
            .iter()
            .filter(|mask| mask.subtract == subtract)
            .filter_map(|mask| Contour::of_bezier(&mask.path.at(frame), matrix))
            .collect();
        elements_of(&contours)
    };
    let added = contours_of(false);
    if !added.is_empty() {
        let inverted = layer.masks.iter().find(|m| !m.subtract).is_some_and(|m| m.inverted);
        return Some(PathClip { elements: added.into(), rule: FillRule::NonZero, inverted });
    }
    Some(PathClip { elements: contours_of(true).into(), rule: FillRule::NonZero, inverted: true })
}

fn layer_paths(
    layer: &Layer,
    layers: &[Layer],
    by_index: &HashMap<i64, &Layer>,
    cx: &Context<'_>,
    out: &mut Vec<PathFill>,
) {
    let frame = cx.frame;
    if layer.hidden || frame < layer.in_frame || frame >= layer.out_frame {
        return;
    }
    let matrix = cx.matrix.then_inner(layer_matrix(layer, by_index, frame));
    // The times in a file already count the layer's start. Only what is inside a
    // nested composition runs on a clock of its own.
    let opacity = cx.opacity * layer.transform.opacity.first_at(frame, 100.0) / 100.0;
    if opacity <= 0.0 {
        return;
    }
    let mut own = cx.with(matrix, opacity);
    if let Some(clip) = masks_clip(layer, frame, matrix) {
        own.clip = Some(Arc::new(clip));
    }
    if layer.matte_mode > 0 {
        let inverted = matches!(layer.matte_mode, 2 | 4);
        let source = match layer.matte_index {
            Some(index) => layers.iter().find(|l| l.index == Some(index)),
            None => layers
                .iter()
                .position(|l| std::ptr::eq(l, layer))
                .and_then(|at| at.checked_sub(1))
                .and_then(|at| layers.get(at)),
        };
        if let Some(source) = source {
            let mut matte = Vec::new();
            let plain = Context { clip: None, ..cx.with(cx.matrix, 1.0) };
            let visible = Layer { is_matte: false, ..source.clone() };
            layer_paths(&visible, layers, by_index, &plain, &mut matte);
            own.clip = Some(Arc::new(union_clip(&matte, inverted)));
        }
    }
    let first = out.len();
    match &layer.kind {
        LayerKind::Empty => {}
        LayerKind::Shape(list) => {
            items(list, &own, out);
        }
        LayerKind::Solid { width, height, color } => {
            let contour = rounded_rect((width / 2.0, height / 2.0), (*width, *height), 0.0, matrix);
            emit(
                out,
                elements_of(&[contour]),
                FillRule::NonZero,
                PathPaint::Solid(Color::rgba(color[0], color[1], color[2], 1.0)),
                opacity,
                &own,
            );
        }
        LayerKind::Image { image, width, height } => {
            let contour = rounded_rect((width / 2.0, height / 2.0), (*width, *height), 0.0, matrix);
            let origin = matrix.apply((0.0, 0.0));
            let (across, down) = (matrix.apply((*width, 0.0)), matrix.apply((0.0, *height)));
            let placement = [
                across.x - origin.x,
                across.y - origin.y,
                down.x - origin.x,
                down.y - origin.y,
                origin.x,
                origin.y,
            ];
            let paint = PathPaint::Image { image: image.clone(), placement };
            emit(out, elements_of(&[contour]), FillRule::NonZero, paint, opacity, &own);
        }
        LayerKind::Precomp { reference, width, height, remap } => {
            let Some(inner) = cx.document.precomps.get(reference) else { return };
            let inner_frame = match remap {
                Some(remap) => remap.first_at(frame, 0.0) * cx.document.frame_rate,
                None => (frame - layer.start) / layer.stretch,
            };
            // A composition shows nothing past its edges, unless something already clips it.
            if own.clip.is_none() && *width > 0.0 && *height > 0.0 {
                let contour = rounded_rect((width / 2.0, height / 2.0), (*width, *height), 0.0, matrix);
                own.clip = Some(Arc::new(PathClip {
                    elements: elements_of(&[contour]).into(),
                    rule: FillRule::NonZero,
                    inverted: false,
                }));
            }
            let inner_cx = Context { frame: inner_frame, ..own.with(matrix, opacity) };
            let inner_cx = Context { clip: own.clip.clone(), ..inner_cx };
            draw_layers(inner, &inner_cx, out);
        }
    }
    if let Some((color, effect_opacity)) = &layer.fill_effect {
        let paint = PathPaint::Solid(color_of(&color.at(frame), 1.0));
        let kept = effect_opacity.first_at(frame, 1.0).clamp(0.0, 1.0);
        for path in &mut out[first..] {
            path.paint = paint.clone();
            path.opacity *= kept;
        }
    }
}

fn draw_layers(layers: &[Layer], cx: &Context<'_>, out: &mut Vec<PathFill>) {
    let by_index: HashMap<i64, &Layer> =
        layers.iter().filter_map(|layer| layer.index.map(|index| (index, layer))).collect();
    for layer in layers {
        if layer.is_matte {
            continue;
        }
        layer_paths(layer, layers, &by_index, cx, out);
    }
}

pub(crate) fn paths_at(document: &Document, seconds: f64, scale: f32) -> Vec<PathFill> {
    let frame = document.in_frame + (seconds as f32 * document.frame_rate);
    // A composition's last frame is the one before where it ends, when layers end too.
    let last = (document.out_frame - 1.0).max(document.in_frame);
    let frame = if frame >= document.out_frame { last } else { frame.max(document.in_frame) };
    let cx = Context {
        document,
        frame,
        matrix: Matrix::IDENTITY,
        opacity: 1.0,
        tolerance: 0.1 / scale.max(0.01),
        clip: None,
        trims: Vec::new(),
    };
    let mut out = Vec::new();
    draw_layers(&document.layers, &cx, &mut out);
    // The list is written top first, and painting goes bottom first.
    out.reverse();
    out
}
