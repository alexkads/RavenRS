/// A timing curve from `(0, 0)` to `(1, 1)` with two control points, the same
/// as CSS `cubic-bezier()`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CubicBezier {
    /// First control point, x then y.
    pub p1: (f32, f32),
    /// Second control point, x then y.
    pub p2: (f32, f32),
}

impl CubicBezier {
    /// A curve with these control points.
    pub const fn new(x1: f32, y1: f32, x2: f32, y2: f32) -> Self {
        Self { p1: (x1, y1), p2: (x2, y2) }
    }

    /// The curve's height at horizontal position `x`. Newton's method converges
    /// in a few steps almost everywhere; bisection covers the flat spots.
    pub fn sample(&self, x: f32) -> f32 {
        if x <= 0.0 {
            return 0.0;
        }
        if x >= 1.0 {
            return 1.0;
        }
        let curve = |a: f32, b: f32, t: f32| {
            let u = 1.0 - t;
            3.0 * u * u * t * a + 3.0 * u * t * t * b + t * t * t
        };
        let slope = |a: f32, b: f32, t: f32| {
            let u = 1.0 - t;
            3.0 * u * u * a + 6.0 * u * t * (b - a) + 3.0 * t * t * (1.0 - b)
        };
        let (x1, x2) = (self.p1.0.clamp(0.0, 1.0), self.p2.0.clamp(0.0, 1.0));
        let mut t = x;
        let mut solved = false;
        for _ in 0..6 {
            let error = curve(x1, x2, t) - x;
            if error.abs() < 1e-4 {
                solved = true;
                break;
            }
            let d = slope(x1, x2, t);
            if d.abs() < 1e-5 {
                break;
            }
            t = (t - error / d).clamp(0.0, 1.0);
        }
        if !solved {
            let (mut lo, mut hi) = (0.0_f32, 1.0_f32);
            for _ in 0..20 {
                t = (lo + hi) / 2.0;
                if curve(x1, x2, t) < x {
                    lo = t;
                } else {
                    hi = t;
                }
            }
        }
        curve(self.p1.1, self.p2.1, t)
    }
}

/// How a timed transition moves from start to end.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[non_exhaustive]
pub enum Easing {
    /// Constant speed; for loops and scrubbing.
    Linear,
    /// Starts slowly; for things that leave for good.
    EaseIn,
    /// Starts fast and settles; acknowledges input at once.
    #[default]
    EaseOut,
    /// Slow at both ends; for things moving from one resting place to another.
    EaseInOut,
    /// A long, soft landing; for large surfaces entering.
    Emphasized,
    /// Any other curve.
    Bezier(CubicBezier),
}

impl Easing {
    /// Progress at `t`, where both run from 0 to 1 (a [`CubicBezier`] may overshoot).
    pub fn sample(&self, t: f32) -> f32 {
        match self {
            Self::Linear => t.clamp(0.0, 1.0),
            Self::EaseIn => CubicBezier::new(0.42, 0.0, 1.0, 1.0).sample(t),
            Self::EaseOut => CubicBezier::new(0.0, 0.0, 0.2, 1.0).sample(t),
            Self::EaseInOut => CubicBezier::new(0.4, 0.0, 0.2, 1.0).sample(t),
            Self::Emphasized => CubicBezier::new(0.05, 0.7, 0.1, 1.0).sample(t),
            Self::Bezier(curve) => curve.sample(t),
        }
    }
}
