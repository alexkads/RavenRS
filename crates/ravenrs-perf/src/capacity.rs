//! How much the renderer can draw: each kind of work grows until a frame stops fitting.
//!
//! The screens in `scenes` ask whether real screens are cheap; this asks where the renderer
//! itself becomes the bottleneck for whoever uses it. The time counted runs from handing the
//! scene over to the GPU having finished, because the GPU's own clock is too coarse on short
//! passes, and because this is what a frame really has to fit in.

use std::time::{Duration, Instant};

use ravenrs_core::{Color, ImageData, ImageQuad, PathElement, PathFill, Point, Quad, Rect, Scene, Shadow};
use ravenrs_render::Renderer;
use ravenrs_text::{TextAttrs, TextSystem};

use crate::{Bench, FORMAT, SCALE, TARGET, VIEWPORT};

/// A frame on the 120 Hz screen, in seconds.
pub const BUDGET_120: f64 = 1.0 / 120.0;
/// A frame on a 60 Hz screen, in seconds.
pub const BUDGET_60: f64 = 1.0 / 60.0;
/// Frames before timing, for atlases and caches to fill.
const WARM: usize = 8;
/// Frames timed at each size.
const FRAMES: usize = 24;
/// The growth stops once a frame takes this long, in seconds.
const ENOUGH: f64 = 0.05;
/// Nothing asks for more than this.
const MOST: usize = 4_000_000;
/// Images in the scene with many different images.
const DISTINCT_IMAGES: usize = 512;

/// One kind of work.
#[derive(Clone, Copy, Debug)]
pub struct Load {
    /// Its name in the table.
    pub name: &'static str,
    /// What is being counted.
    pub unit: &'static str,
    /// Where the growth starts.
    pub start: usize,
    /// Makes a scene with this many.
    pub make: fn(usize, &mut Assets) -> Scene,
}

/// What the scenes draw with.
#[derive(Debug)]
pub struct Assets {
    text: TextSystem,
    images: Vec<ImageData>,
    circle: std::sync::Arc<[PathElement]>,
}

impl Default for Assets {
    fn default() -> Self {
        let images = (0..DISTINCT_IMAGES as u32)
            .filter_map(|i| {
                let pixels: Vec<u8> =
                    (0..32u32 * 32).flat_map(|p| [(i % 256) as u8, p as u8, 90, 255]).collect();
                ImageData::from_rgba8(32, 32, pixels)
            })
            .collect();
        Self { text: TextSystem::new(), images, circle: circle().into() }
    }
}

/// A circle of 12 logical pixels of radius, in curves, at the origin.
fn circle() -> Vec<PathElement> {
    const R: f32 = 12.0;
    const K: f32 = R * 0.5523;
    let p = Point::new;
    vec![
        PathElement::MoveTo(p(R, 0.0)),
        PathElement::CubicTo(p(R, K), p(K, R), p(0.0, R)),
        PathElement::CubicTo(p(-K, R), p(-R, K), p(-R, 0.0)),
        PathElement::CubicTo(p(-R, -K), p(-K, -R), p(0.0, -R)),
        PathElement::CubicTo(p(K, -R), p(R, -K), p(R, 0.0)),
        PathElement::Close,
    ]
}

/// Where the `i`-th of `n` things goes, spread over the whole window.
fn place(i: usize, n: usize, width: f32, height: f32) -> Point {
    let columns = ((n as f32 * VIEWPORT.width / VIEWPORT.height).sqrt().ceil() as usize).max(1);
    let rows = n.div_ceil(columns).max(1);
    let (column, row) = (i % columns, i / columns);
    Point::new(
        column as f32 * (VIEWPORT.width - width) / columns.max(2).saturating_sub(1) as f32,
        row as f32 * (VIEWPORT.height - height) / rows.max(2).saturating_sub(1) as f32,
    )
}

fn color(i: usize) -> Color {
    Color::rgba(0.2 + (i % 7) as f32 * 0.1, 0.4, 0.9 - (i % 5) as f32 * 0.1, 1.0)
}

fn card(i: usize, n: usize) -> Rect {
    let at = place(i, n, 40.0, 24.0);
    Rect::new(at.x, at.y, 40.0, 24.0)
}

fn quads(n: usize, _: &mut Assets) -> Scene {
    let mut scene = Scene::new();
    for i in 0..n {
        scene.push_quad(Quad::new(card(i, n), color(i)).radius(6.0));
    }
    scene
}

fn shadows(n: usize, _: &mut Assets) -> Scene {
    let mut scene = Scene::new();
    for i in 0..n {
        let mut quad = Quad::new(card(i, n), color(i)).radius(6.0);
        quad.shadow = Some(Shadow::soft());
        scene.push_quad(quad);
    }
    scene
}

const SENTENCE: &str = "The quick brown fox jumps over the lazy dog";

fn sentences(n: usize, attrs: &TextAttrs, assets: &mut Assets, between: bool) -> Scene {
    let mut layout = assets.text.layout(SENTENCE, attrs);
    layout.measure(None);
    let mut scene = Scene::new();
    for i in 0..n {
        let at = place(i, n, 300.0, 20.0);
        if between {
            scene.push_quad(Quad::new(Rect::new(at.x, at.y, 300.0, 20.0), color(i)).radius(4.0));
        }
        layout.paint(at, Color::WHITE, &mut scene);
    }
    scene
}

fn glyphs(n: usize, assets: &mut Assets) -> Scene {
    sentences(n / SENTENCE.len(), &TextAttrs::default(), assets, false)
}

/// What a real screen looks like: a shape, then text on it, and so on, which splits the draws.
fn ui_mix(n: usize, assets: &mut Assets) -> Scene {
    sentences(n, &TextAttrs::default(), assets, true)
}

fn clips(n: usize, _: &mut Assets) -> Scene {
    let mut scene = Scene::new();
    for i in 0..n {
        let rect = card(i, n);
        scene.push_clip(rect);
        scene.push_quad(Quad::new(rect, color(i)).radius(6.0));
        scene.pop_clip();
    }
    scene
}

fn paths(n: usize, assets: &mut Assets) -> Scene {
    let mut scene = Scene::new();
    for i in 0..n {
        let at = place(i, n, 24.0, 24.0);
        scene.push_path(PathFill {
            transform: [1.0, 0.0, 0.0, 1.0, at.x + 12.0, at.y + 12.0],
            ..PathFill::solid(assets.circle.clone(), color(i))
        });
    }
    scene
}

/// Shapes as big as the window, as a large animation has.
fn big_paths(n: usize, assets: &mut Assets) -> Scene {
    let mut scene = Scene::new();
    let scale = VIEWPORT.height / 24.0;
    for i in 0..n {
        scene.push_path(PathFill {
            transform: [scale, 0.0, 0.0, scale, VIEWPORT.width / 2.0, VIEWPORT.height / 2.0],
            opacity: 0.05,
            ..PathFill::solid(assets.circle.clone(), color(i))
        });
    }
    scene
}

fn images(n: usize, assets: &mut Assets) -> Scene {
    let mut scene = Scene::new();
    for i in 0..n {
        let image = assets.images[i % assets.images.len()].clone();
        scene.push_image(ImageQuad::new(card(i, n), image));
    }
    scene
}

/// Layers covering the whole window, each seen through the last: what the GPU's fill costs.
fn overdraw(n: usize, _: &mut Assets) -> Scene {
    let mut scene = Scene::new();
    let window = Rect::from_origin_size(Point::ZERO, VIEWPORT);
    for i in 0..n {
        let tint = Color::rgba(0.3, 0.5, (i % 3) as f32 * 0.3, 0.05);
        scene.push_quad(Quad::new(window, tint).radius(16.0));
    }
    scene
}

/// Every kind of work measured, in the order reported.
pub const LOADS: [Load; 9] = [
    Load { name: "quads", unit: "quads", start: 1_000, make: quads },
    Load { name: "soft shadows", unit: "quads", start: 500, make: shadows },
    Load { name: "text", unit: "glyphs", start: 2_000, make: glyphs },
    Load { name: "ui mix", unit: "quad+text", start: 100, make: ui_mix },
    Load { name: "clips", unit: "clipped quads", start: 500, make: clips },
    Load { name: "curves", unit: "shapes", start: 100, make: paths },
    Load { name: "big curves", unit: "shapes", start: 2, make: big_paths },
    Load { name: "images", unit: "images", start: 100, make: images },
    Load { name: "overdraw", unit: "full layers", start: 2, make: overdraw },
];

/// What a frame took with this much work.
#[derive(Clone, Copy, Debug)]
pub struct Point2 {
    /// How much was drawn.
    pub amount: usize,
    /// Preparing and submitting on the CPU, in seconds.
    pub cpu: f64,
    /// Until the GPU finished, in seconds.
    pub total: f64,
    /// Draw calls.
    pub draw_calls: usize,
    /// Shapes that really got drawn, for telling when some are being left out.
    pub paths: usize,
}

fn median(values: &mut [Duration]) -> f64 {
    values.sort();
    values.get(values.len() / 2).map_or(0.0, Duration::as_secs_f64)
}

impl Bench {
    /// Grows `load` until a frame takes too long, and says what each size cost.
    pub fn capacity(&self, load: &Load) -> Vec<Point2> {
        let mut assets = Assets::default();
        let mut renderer = Renderer::new(&self.gpu.device, &self.gpu.queue, FORMAT);
        let mut curve = Vec::new();
        let mut amount = load.start;
        while amount <= MOST {
            let scene = (load.make)(amount, &mut assets);
            let (mut draw_calls, mut paths) = (0, 0);
            for _ in 0..WARM {
                renderer.render(&scene, &self.target, TARGET, SCALE);
                self.wait();
            }
            let (mut cpu, mut total) = (Vec::new(), Vec::new());
            for _ in 0..FRAMES {
                let started = Instant::now();
                let stats = renderer.render(&scene, &self.target, TARGET, SCALE);
                cpu.push(started.elapsed());
                self.wait();
                total.push(started.elapsed());
                draw_calls = stats.draw_calls;
                paths = stats.paths;
            }
            let point =
                Point2 { amount, cpu: median(&mut cpu), total: median(&mut total), draw_calls, paths };
            curve.push(point);
            if point.total > ENOUGH {
                break;
            }
            amount *= 2;
        }
        curve
    }
}

/// How much fits in `budget` seconds, between the sizes measured; `None` when all of it did.
pub fn fits(curve: &[Point2], budget: f64) -> Option<usize> {
    let after = curve.iter().position(|point| point.total > budget)?;
    let Some(before) = after.checked_sub(1).map(|i| curve[i]) else { return Some(curve[0].amount) };
    let next = curve[after];
    let part = (budget - before.total) / (next.total - before.total);
    Some(before.amount + (part * (next.amount - before.amount) as f64) as usize)
}

/// The size at which shapes started being left out, for a load made of shapes.
pub fn drops_from(curve: &[Point2]) -> Option<usize> {
    curve.iter().find(|point| point.paths > 0 && point.paths < point.amount).map(|point| point.paths)
}
