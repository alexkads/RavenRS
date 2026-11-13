//! [`TimeLink`]: the clock and a frame, turned into an effect.

use std::fmt;
use std::rc::Rc;

use ravenrs_core::{Effect, Rect};

/// Turns the clock (seconds) and an element's frame into an [`Effect`], worked out
/// on every painted frame while the element shows. See [`BuildCx::time_linked`](crate::BuildCx::time_linked).
#[derive(Clone)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct TimeLink(Rc<dyn Fn(f64, Rect) -> Effect>);

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl TimeLink {
    /// The effect is `f(seconds, frame)`.
    pub fn new(f: impl Fn(f64, Rect) -> Effect + 'static) -> Self {
        Self(Rc::new(f))
    }

    /// Floats up and down by `distance` pixels every `period` seconds.
    pub fn float(distance: f32, period: f32) -> Self {
        Self::new(move |t, _| {
            let phase = (t as f32 / period.max(0.01)) * std::f32::consts::TAU;
            Effect::moved(0.0, phase.sin() * distance)
        })
    }

    /// The effect at `seconds` for an element at `frame`.
    pub fn effect_at(&self, seconds: f64, frame: Rect) -> Effect {
        (self.0)(seconds, frame)
    }
}

impl fmt::Debug for TimeLink {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("TimeLink")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_float_returns_to_its_start_each_period() {
        let link = TimeLink::float(6.0, 2.0);
        let frame = Rect::new(0.0, 0.0, 10.0, 10.0);
        assert!(link.effect_at(0.0, frame).offset.y.abs() < 1e-5);
        assert!((link.effect_at(0.5, frame).offset.y - 6.0).abs() < 1e-4);
        assert!(link.effect_at(2.0, frame).offset.y.abs() < 1e-4);
    }
}
