/// A damped harmonic oscillator with unit mass, solved in closed form.
///
/// Numeric integration drifts with the frame rate and can blow up after a long
/// frame. The analytic solution gives the same position at a given time however
/// many frames were drawn on the way there.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spring {
    /// Restoring force per unit of displacement.
    pub stiffness: f32,
    /// Braking force per unit of velocity.
    pub damping: f32,
}

impl Spring {
    /// A spring with this stiffness and damping.
    pub const fn new(stiffness: f32, damping: f32) -> Self {
        Self { stiffness, damping }
    }

    /// For things that change place or size (the `spring-snappy` token).
    pub const fn snappy() -> Self {
        Self::new(380.0, 30.0)
    }

    /// For panels and sheets (the `spring-gentle` token).
    pub const fn gentle() -> Self {
        Self::new(170.0, 26.0)
    }

    /// A spring described the way designers think about it: `response` is the
    /// period of the undamped oscillation in seconds, and `damping_ratio` is
    /// 1.0 for no overshoot, less for some bounce.
    pub fn from_response(response: f32, damping_ratio: f32) -> Self {
        let omega = std::f32::consts::TAU / response.max(1e-3);
        Self::new(omega * omega, 2.0 * damping_ratio.max(0.0) * omega)
    }

    /// Displacement and velocity `t` seconds after starting from displacement
    /// `x0` with velocity `v0`. The spring pulls toward zero.
    pub fn sample(&self, x0: f32, v0: f32, t: f32) -> (f32, f32) {
        let k = self.stiffness.max(1e-3);
        let omega = k.sqrt();
        let zeta = self.damping.max(0.0) / (2.0 * omega);
        let t = t.max(0.0);
        if (zeta - 1.0).abs() < 1e-3 {
            let c = v0 + omega * x0;
            let decay = (-omega * t).exp();
            ((x0 + c * t) * decay, (c - omega * (x0 + c * t)) * decay)
        } else if zeta < 1.0 {
            let wd = omega * (1.0 - zeta * zeta).sqrt();
            let a = zeta * omega;
            let c = (v0 + a * x0) / wd;
            let decay = (-a * t).exp();
            let (sin, cos) = (wd * t).sin_cos();
            let x = decay * (x0 * cos + c * sin);
            let v = decay * ((c * wd - a * x0) * cos - (x0 * wd + a * c) * sin);
            (x, v)
        } else {
            let root = (zeta * zeta - 1.0).sqrt();
            let (r1, r2) = (-omega * (zeta - root), -omega * (zeta + root));
            let a = (v0 - r2 * x0) / (r1 - r2);
            let b = x0 - a;
            let (e1, e2) = ((r1 * t).exp(), (r2 * t).exp());
            (a * e1 + b * e2, a * r1 * e1 + b * r2 * e2)
        }
    }
}
