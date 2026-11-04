//! The performance gate of RavenRS.
//!
//! A [`PerfScene`] is a screen and the interactions to try on it. [`Bench::measure`] drives
//! it with a real [`Ui`] and a real [`Renderer`] on an offscreen texture, without a window,
//! and counts the work: builds, layouts, paints, shaping, allocations, draw calls and
//! bytes sent to the GPU. Those counts are exact, so `gate` can compare them with a
//! baseline on every run, where a time measured on a laptop would only be noise.
//!
//! Times are measured apart by `time`, before a release.

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use ravenrs_core::{BuildCause, Event, Point, Rect, Role, Size};
use ravenrs_render::{GpuContext, RenderStats, Renderer, wgpu};
use ravenrs_style::Theme;
use ravenrs_widgets::{AnyView, Ui, UiStats};

pub mod capacity;
pub mod scenes;

/// The window the scenes are measured in, in logical pixels.
pub const VIEWPORT: Size = Size::new(1280.0, 800.0);
/// Physical pixels per logical pixel: the laptop's screen.
pub const SCALE: f32 = 2.0;
/// Time between frames on the 120 Hz screen.
const FRAME: f64 = 1.0 / 120.0;
/// Frames before anything is looked at, for layout feedback to settle.
const SETTLE: u32 = 16;
/// Rounds of the interaction before counting, for caches to fill.
const WARM: u32 = 32;
/// Rounds counted, which ceilings are multiplied by.
pub const COUNTED_ROUNDS: u32 = 16;

const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8Unorm;
const TARGET: (u32, u32) = ((VIEWPORT.width * SCALE) as u32, (VIEWPORT.height * SCALE) as u32);

/// What a component is, which says what may be asked of it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    /// Draws what it has; costs nothing once drawn.
    Static,
    /// Shows a window over much more data: the cost follows the window.
    Virtualized,
    /// Moves by itself.
    Animated,
    /// A picture made on the CPU, whose cost is paid when its data or size changes.
    Picture,
    /// A whole screen of a real application.
    App,
}

/// One round of an interaction: `true` asks for a frame, as a platform does when
/// [`Ui::handle_event`] says so.
pub type Step = Box<dyn FnMut(&mut Ui, u32) -> bool>;

/// A screen ready to be measured under one interaction.
pub struct Setup {
    /// What is on screen.
    pub root: AnyView,
    /// What the interaction does each round.
    pub step: Step,
}

impl std::fmt::Debug for Setup {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Setup").finish_non_exhaustive()
    }
}

/// A limit on a counter, per round. A baseline can follow a slow decay; this cannot.
#[derive(Clone, Copy, Debug)]
pub struct Ceiling {
    /// The interaction it applies to, or `"*"`.
    pub interaction: &'static str,
    /// The counter.
    pub counter: &'static str,
    /// The most it may reach per round counted.
    pub per_round: u64,
}

/// A shorter way to write a [`Ceiling`].
pub const fn ceiling(interaction: &'static str, counter: &'static str, per_round: u64) -> Ceiling {
    Ceiling { interaction, counter, per_round }
}

/// A screen and the interactions to try on it.
#[derive(Clone, Copy, Debug)]
pub struct PerfScene {
    /// Its name in reports and in the baseline.
    pub name: &'static str,
    /// The public components it answers for.
    pub covers: &'static [&'static str],
    /// What kind of thing it is.
    pub class: Class,
    /// The interactions to try.
    pub interactions: &'static [&'static str],
    /// Makes a fresh copy of the screen for one interaction.
    pub make: fn(&str) -> Setup,
    /// The limits that no baseline can move.
    pub ceilings: &'static [Ceiling],
}

/// Counts of the work done during the rounds counted, by name.
pub type Counters = BTreeMap<&'static str, u64>;

/// Counters that vary with the driver, so they are shown but never compared.
pub const REPORTED_ONLY: &[&str] = &["render_allocations"];

/// What an interaction did.
#[derive(Clone, Debug)]
pub struct Outcome {
    /// `scene/interaction`.
    pub name: String,
    /// The counts.
    pub counters: Counters,
    /// How long each frame took, when asked for.
    pub frames: Vec<FrameTime>,
}

/// What one frame cost.
#[derive(Clone, Copy, Debug, Default)]
pub struct FrameTime {
    /// Handling the event, building, laying out and painting.
    pub ui: Duration,
    /// Of that, the views running.
    pub build: Duration,
    /// Of that, solving the layout.
    pub layout: Duration,
    /// Of that, placing elements.
    pub place: Duration,
    /// Of that, painting.
    pub paint: Duration,
    /// Recording and submitting the GPU's work.
    pub render: Duration,
    /// The GPU's own time, from a frame or two before.
    pub gpu: Option<Duration>,
}

/// Where the measurements run: the GPU, and what counts allocations.
pub struct Bench {
    gpu: GpuContext,
    target: wgpu::TextureView,
    allocations: fn() -> u64,
}

impl std::fmt::Debug for Bench {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Bench").finish_non_exhaustive()
    }
}

impl Bench {
    /// Opens the GPU. `allocations` tells how many allocations happened so far; only the
    /// binary can install the allocator that knows.
    pub fn new(allocations: fn() -> u64) -> Result<Self, String> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        let gpu = pollster::block_on(GpuContext::new(&instance, None)).map_err(|e| e.to_string())?;
        let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("perf target"),
            size: wgpu::Extent3d { width: TARGET.0, height: TARGET.1, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let target = texture.create_view(&wgpu::TextureViewDescriptor::default());
        Ok(Self { gpu, target, allocations })
    }

    /// The GPU being measured.
    pub fn adapter(&self) -> String {
        let info = self.gpu.adapter.get_info();
        format!("{} ({:?}), {}x{} target", info.name, info.backend, TARGET.0, TARGET.1)
    }

    fn wait(&self) {
        let _ = self.gpu.device.poll(wgpu::PollType::wait_indefinitely());
    }

    fn frame(&self, ui: &mut Ui, renderer: &mut Renderer) {
        renderer.render(ui.update(), &self.target, TARGET, SCALE);
        self.wait();
    }

    /// Drives one interaction of `scene` and counts what it does. With `rounds`, it
    /// counts that many and also keeps their times, with the GPU's clock on.
    pub fn measure(&self, scene: &PerfScene, interaction: &str, rounds: Option<u32>) -> Outcome {
        let Setup { root, mut step } = (scene.make)(interaction);
        let mut renderer = Renderer::new(&self.gpu.device, &self.gpu.queue, FORMAT);
        let mut ui = Ui::new(root, Theme::raven_light());
        ui.set_viewport(VIEWPORT, SCALE);
        let timed = rounds.is_some();
        if timed {
            let started = Instant::now();
            ui.set_phase_clock(Some(std::rc::Rc::new(move || started.elapsed().as_secs_f64())));
            renderer.set_gpu_timing(true);
        }
        let mut now = 0.0;
        for _ in 0..SETTLE {
            now += FRAME;
            ui.set_time(now);
            self.frame(&mut ui, &mut renderer);
        }
        let mut round = 0;
        for _ in 0..WARM {
            now += FRAME;
            ui.set_time(now);
            let asked = step(&mut ui, round);
            round += 1;
            if asked || ui.needs_redraw() {
                self.frame(&mut ui, &mut renderer);
            }
        }

        let before = ui.stats();
        let mut tally = Tally::default();
        let mut frames = Vec::new();
        let mut first_stats: Option<RenderStats> = None;
        for _ in 0..rounds.unwrap_or(COUNTED_ROUNDS) {
            now += FRAME;
            ui.set_time(now);
            let phases = ui.phase_times();
            let (allocations, started) = ((self.allocations)(), Instant::now());
            let asked = step(&mut ui, round);
            round += 1;
            let draw = asked || ui.needs_redraw();
            if draw {
                ui.update();
            }
            let spent = started.elapsed();
            tally.ui_allocations += (self.allocations)() - allocations;
            if !draw {
                continue;
            }
            let (allocations, started) = ((self.allocations)(), Instant::now());
            let stats = renderer.render(ui.scene(), &self.target, TARGET, SCALE);
            let rendered = started.elapsed();
            tally.render_allocations += (self.allocations)() - allocations;
            self.wait();
            tally.add(&stats, first_stats.get_or_insert(stats));
            if timed {
                let after = ui.phase_times();
                frames.push(FrameTime {
                    ui: spent,
                    build: after.build - phases.build,
                    layout: after.layout - phases.layout,
                    place: after.place - phases.place,
                    paint: after.paint - phases.paint,
                    render: rendered,
                    gpu: stats.gpu_time,
                });
            }
        }
        let counters = tally.into_counters(before, ui.stats(), &ui);
        Outcome { name: format!("{}/{interaction}", scene.name), counters, frames }
    }
}

/// One thing opened for the first or a later time: its worst frame, which is the one a
/// person sees stutter. Warm frames say nothing about this.
#[derive(Clone, Debug)]
pub struct Visit {
    /// What was opened.
    pub name: String,
    /// The worst frame's time on the CPU, from building to submitting.
    pub worst: Duration,
    /// Frames it took to settle.
    pub frames: u32,
    /// Glyphs rasterized on the way.
    pub rasterized: u64,
}

impl Bench {
    /// Puts `root` on screen for the first time, then runs each visit in turn, each
    /// until the screen stops asking for frames.
    pub fn tour(
        &self,
        root: AnyView,
        prepare: impl FnOnce(&mut Ui),
        visits: Vec<(String, Box<dyn FnOnce()>)>,
    ) -> Vec<Visit> {
        let mut renderer = Renderer::new(&self.gpu.device, &self.gpu.queue, FORMAT);
        let mut ui = Ui::new(root, Theme::raven_light());
        ui.set_viewport(VIEWPORT, SCALE);
        let mut now = 0.0;
        let mut out = vec![self.settle("first frame", &mut ui, &mut renderer, &mut now)];
        prepare(&mut ui);
        out.push(self.settle("signed in", &mut ui, &mut renderer, &mut now));
        for (name, visit) in visits {
            visit();
            out.push(self.settle(&name, &mut ui, &mut renderer, &mut now));
        }
        // Each visit saw the running total; what it added is what it cost.
        let mut seen = 0;
        for visit in &mut out {
            let total = visit.rasterized.max(seen);
            visit.rasterized = total - seen;
            seen = total;
        }
        out
    }

    fn settle(&self, name: &str, ui: &mut Ui, renderer: &mut Renderer, now: &mut f64) -> Visit {
        let mut visit = Visit { name: name.to_owned(), worst: Duration::ZERO, frames: 0, rasterized: 0 };
        // Long enough for transitions; what keeps moving forever stops being counted here.
        for _ in 0..60 {
            *now += FRAME;
            ui.set_time(*now);
            if !ui.needs_redraw() && visit.frames > 0 {
                break;
            }
            let started = Instant::now();
            let stats = renderer.render(ui.update(), &self.target, TARGET, SCALE);
            visit.worst = visit.worst.max(started.elapsed());
            visit.frames += 1;
            self.wait();
            visit.rasterized = stats.glyphs_rasterized;
        }
        visit
    }
}

#[derive(Default)]
struct Tally {
    frames: u64,
    ui_allocations: u64,
    render_allocations: u64,
    draw_calls: u64,
    breaks_kind: u64,
    breaks_image: u64,
    breaks_clip: u64,
    instances: u64,
    uploaded_bytes: u64,
    rasterized: u64,
    resets: u64,
}

impl Tally {
    fn add(&mut self, stats: &RenderStats, first: &RenderStats) {
        self.frames += 1;
        self.draw_calls += stats.draw_calls as u64;
        self.breaks_kind += stats.breaks_kind as u64;
        self.breaks_image += stats.breaks_image as u64;
        self.breaks_clip += stats.breaks_clip as u64;
        self.instances += (stats.quads + stats.glyphs + stats.images + stats.paths) as u64;
        self.uploaded_bytes += stats.uploaded_bytes as u64;
        self.rasterized = stats.glyphs_rasterized - first.glyphs_rasterized;
        self.resets = stats.atlas_resets - first.atlas_resets;
    }

    fn into_counters(self, before: UiStats, after: UiStats, ui: &Ui) -> Counters {
        Counters::from([
            ("frames", self.frames),
            ("builds", after.builds - before.builds),
            ("layouts", after.layouts - before.layouts),
            ("placements", after.placements - before.placements),
            ("paints", after.paints - before.paints),
            ("shapes", after.shapes - before.shapes),
            ("ui_allocations", self.ui_allocations),
            ("render_allocations", self.render_allocations),
            ("draw_calls", self.draw_calls),
            ("breaks_kind", self.breaks_kind),
            ("breaks_image", self.breaks_image),
            ("breaks_clip", self.breaks_clip),
            ("instances", self.instances),
            ("uploaded_bytes", self.uploaded_bytes),
            ("glyphs_rasterized", self.rasterized),
            ("atlas_resets", self.resets),
            ("elements", ui.element_count() as u64),
            ("primitives", ui.scene().len() as u64),
            ("wakes_scheduled", u64::from(ui.next_wake().is_some())),
            ("cause_data", after.causes.count(BuildCause::Data) - before.causes.count(BuildCause::Data)),
            ("cause_hover", after.causes.count(BuildCause::Hover) - before.causes.count(BuildCause::Hover)),
            ("cause_clock", after.causes.count(BuildCause::Clock) - before.causes.count(BuildCause::Clock)),
            (
                "cause_scroll",
                after.causes.count(BuildCause::Scroll) - before.causes.count(BuildCause::Scroll),
            ),
        ])
    }
}

/// Moves the pointer back and forth between two places, as if crossing two controls.
pub fn hover_between(a: Point, b: Point) -> Step {
    Box::new(move |ui, round| {
        let position = if round % 2 == 0 { a } else { b };
        ui.handle_event(&Event::PointerMove { position })
    })
}

/// Moves the pointer back and forth between the first two buttons on screen, so
/// every round really changes what is hovered, wherever the layout put them.
pub fn hover_buttons() -> Step {
    let mut places: Option<(Point, Point)> = None;
    Box::new(move |ui, round| {
        let (a, b) = *places.get_or_insert_with(|| {
            let viewport = Rect::from_origin_size(Point::ZERO, VIEWPORT);
            let found: Vec<Point> = ui
                .access_tree()
                .nodes
                .iter()
                .filter(|node| node.role == Role::Button && viewport.contains(node.bounds.center()))
                .map(|node| node.bounds.center())
                .take(2)
                .collect();
            match found.as_slice() {
                [a, b] => (*a, *b),
                _ => panic!("a hover scene needs two buttons on screen"),
            }
        });
        let position = if round % 2 == 0 { a } else { b };
        ui.handle_event(&Event::PointerMove { position })
    })
}

/// Scrolls down and back up at `position`.
pub fn scroll_at(position: Point) -> Step {
    Box::new(move |ui, round| {
        let delta = Point::new(0.0, if round % 2 == 0 { -40.0 } else { 40.0 });
        ui.handle_event(&Event::Scroll { position, delta })
    })
}

/// Nothing happens: what is left is what the screen does by itself.
pub fn nothing() -> Step {
    Box::new(|_, _| false)
}

/// The window changes size by a pixel, which redoes everything from scratch.
pub fn resize() -> Step {
    Box::new(|ui, round| {
        let width = VIEWPORT.width - (round % 2) as f32;
        ui.set_viewport(Size::new(width, VIEWPORT.height), SCALE);
        true
    })
}
