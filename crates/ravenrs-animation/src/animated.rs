use ravenrs_core::{Color, Effect, Point, Rect};

use crate::oklab::{premultiplied, unpremultiplied};
use crate::{Easing, Spring};

/// A value that can be animated: up to four numbers that add and subtract.
pub trait Animatable: Copy {
    /// Below this distance from the target, the animation counts as finished.
    const REST: f32;
    /// The value as numbers to interpolate.
    fn to_components(self) -> [f32; 4];
    /// The value back from interpolated numbers, which may overshoot.
    fn from_components(c: [f32; 4]) -> Self;
}

impl Animatable for f32 {
    const REST: f32 = 0.001;
    fn to_components(self) -> [f32; 4] {
        [self, 0.0, 0.0, 0.0]
    }
    fn from_components(c: [f32; 4]) -> Self {
        c[0]
    }
}

impl Animatable for Point {
    const REST: f32 = 0.02;
    fn to_components(self) -> [f32; 4] {
        [self.x, self.y, 0.0, 0.0]
    }
    fn from_components(c: [f32; 4]) -> Self {
        Point::new(c[0], c[1])
    }
}

impl Animatable for Rect {
    const REST: f32 = 0.02;
    fn to_components(self) -> [f32; 4] {
        [self.x(), self.y(), self.width(), self.height()]
    }
    fn from_components(c: [f32; 4]) -> Self {
        Rect::new(c[0], c[1], c[2].max(0.0), c[3].max(0.0))
    }
}

impl Animatable for Color {
    const REST: f32 = 0.002;
    fn to_components(self) -> [f32; 4] {
        premultiplied(self)
    }
    fn from_components(c: [f32; 4]) -> Self {
        unpremultiplied(c)
    }
}

impl Animatable for Effect {
    const REST: f32 = 0.002;
    fn to_components(self) -> [f32; 4] {
        [self.opacity, self.offset.x, self.offset.y, self.scale]
    }
    fn from_components(c: [f32; 4]) -> Self {
        Effect { opacity: c[0].clamp(0.0, 1.0), offset: Point::new(c[1], c[2]), scale: c[3].max(0.0) }
    }
}

/// How a value travels to a new target.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub enum Transition {
    /// No animation.
    Instant,
    /// A fixed duration along a curve.
    Tween {
        /// Duration in seconds.
        duration: f32,
        /// The curve.
        easing: Easing,
    },
    /// A spring, which takes as long as it takes and keeps velocity when redirected.
    Spring(Spring),
}

impl Transition {
    /// A timed transition.
    pub const fn tween(duration: f32, easing: Easing) -> Self {
        Self::Tween { duration, easing }
    }

    /// Hover, press and focus color (the `duration-fast` token, ease-out).
    pub const fn fast() -> Self {
        Self::tween(0.12, Easing::EaseOut)
    }

    /// Small layout and opacity changes (the `duration-base` token).
    pub const fn base() -> Self {
        Self::tween(0.2, Easing::EaseInOut)
    }

    /// Panels and sheets entering (the `duration-slow` token).
    pub const fn slow() -> Self {
        Self::tween(0.32, Easing::Emphasized)
    }

    /// Things changing place or size.
    pub const fn snappy() -> Self {
        Self::Spring(Spring::snappy())
    }

    /// Panels and sheets.
    pub const fn gentle() -> Self {
        Self::Spring(Spring::gentle())
    }

    /// The version of this transition for properties that stay animated under
    /// reduced motion (color, opacity): a spring becomes a short fade.
    pub const fn without_bounce(self) -> Self {
        match self {
            Self::Spring(_) => Self::fast(),
            other => other,
        }
    }
}

const ZERO: [f32; 4] = [0.0; 4];

#[derive(Clone, Copy, Debug, PartialEq)]
struct Segment {
    /// How far the target moved when this segment started.
    delta: [f32; 4],
    start: f64,
    duration: f32,
    easing: Easing,
}

impl Segment {
    fn remaining(&self, now: f64) -> f32 {
        let p = ((now - self.start) as f32 / self.duration.max(1e-4)).clamp(0.0, 1.0);
        1.0 - self.easing.sample(p)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct SpringRun {
    spring: Spring,
    start: f64,
    /// Displacement from the target at `start`.
    x0: [f32; 4],
    v0: [f32; 4],
}

/// A value on its way to a target. It holds no clock: every call takes the time.
///
/// Redirecting is continuous. A spring restarts from the current position and
/// velocity, which is the exact solution for a moved anchor. Tweens are
/// *additive*: the new one eases in the target's change while earlier ones finish
/// easing out theirs, and the sum has no corner, even when a hover flickers.
#[derive(Clone, Debug)]
pub struct Animated<T> {
    /// The target exactly as given: at rest, conversions must not change it.
    exact: T,
    target: [f32; 4],
    segments: Vec<Segment>,
    spring: Option<SpringRun>,
}

impl<T: Animatable> Animated<T> {
    /// A value at rest.
    pub fn new(value: T) -> Self {
        Self { exact: value, target: value.to_components(), segments: Vec::new(), spring: None }
    }

    /// Where the value is heading.
    pub fn target(&self) -> T {
        self.exact
    }

    /// Sends the value to `target` starting at time `now` (seconds). Asking for
    /// the current target again does nothing, so calling this on every build is fine.
    pub fn set_target(&mut self, target: T, transition: Transition, now: f64) {
        let new = target.to_components();
        if new == self.target {
            return;
        }
        match transition {
            Transition::Instant => {
                self.segments.clear();
                self.spring = None;
            }
            Transition::Tween { duration, easing } if duration > 0.0 => {
                let delta = std::array::from_fn(|i| new[i] - self.target[i]);
                self.segments.push(Segment { delta, start: now, duration, easing });
            }
            Transition::Tween { .. } => {
                self.segments.clear();
                self.spring = None;
            }
            Transition::Spring(spring) => {
                let (x, v) = (self.components(now), self.velocity(now));
                self.segments.clear();
                let x0 = std::array::from_fn(|i| x[i] - new[i]);
                self.spring = Some(SpringRun { spring, start: now, x0, v0: v });
            }
        }
        self.target = new;
        self.exact = target;
    }

    /// The value at time `now`.
    pub fn value(&self, now: f64) -> T {
        if self.is_moving() { T::from_components(self.components(now)) } else { self.exact }
    }

    fn components(&self, now: f64) -> [f32; 4] {
        let mut out = self.target;
        if let Some(run) = &self.spring {
            let t = (now - run.start) as f32;
            for (i, v) in out.iter_mut().enumerate() {
                *v += run.spring.sample(run.x0[i], run.v0[i], t).0;
            }
        }
        for s in &self.segments {
            let remaining = s.remaining(now);
            for (i, v) in out.iter_mut().enumerate() {
                *v -= s.delta[i] * remaining;
            }
        }
        out
    }

    /// Rate of change per second of each component at time `now`.
    pub fn velocity(&self, now: f64) -> [f32; 4] {
        let mut out = ZERO;
        if let Some(run) = &self.spring {
            let t = (now - run.start) as f32;
            for (i, v) in out.iter_mut().enumerate() {
                *v += run.spring.sample(run.x0[i], run.v0[i], t).1;
            }
        }
        const H: f64 = 1.0 / 480.0;
        for s in &self.segments {
            let slope = (s.remaining(now - H) - s.remaining(now + H)) / (2.0 * H as f32);
            for (i, v) in out.iter_mut().enumerate() {
                *v += s.delta[i] * slope;
            }
        }
        out
    }

    /// Forgets what has finished by time `now`. Returns true while still moving.
    pub fn step(&mut self, now: f64) -> bool {
        self.segments.retain(|s| now - s.start < f64::from(s.duration));
        if let Some(run) = &self.spring {
            let t = (now - run.start) as f32;
            let rest = (0..4).all(|i| {
                let (x, v) = run.spring.sample(run.x0[i], run.v0[i], t);
                x.abs() < T::REST && v.abs() < T::REST * 10.0
            });
            if rest {
                self.spring = None;
            }
        }
        !self.segments.is_empty() || self.spring.is_some()
    }

    /// True while the value has not reached its target as of the last [`Self::step`].
    pub fn is_moving(&self) -> bool {
        !self.segments.is_empty() || self.spring.is_some()
    }
}
